//! Orchestrate project components and external providers.

use std::process::{Command, Stdio, Child};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use std::net::TcpStream;
use anyhow::{Context, Result, bail};
use crate::detect;
use crate::model::{ApplicationGraph, Component, Provider, Ownership, Readiness};
use crate::util;

pub struct RunningApp {
    pub children: Vec<ManagedProcess>,
    pub graph: ApplicationGraph,
}

pub struct ManagedProcess {
    pub id: String,
    pub child: Child,
    pub cmd: String,
    pub ready: bool,
}

impl RunningApp {
    pub fn shutdown(&mut self) {
        for mp in self.children.iter_mut().rev() {
            let _ = mp.child.kill();
            let _ = mp.child.wait();
        }
        self.children.clear();
    }
}

impl Drop for RunningApp {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn orchestrate(graph: &ApplicationGraph, verbose: bool) -> Result<RunningApp> {
    println!("Runtime plan:");
    println!();
    let order = graph.start_order_ids();
    let mut step = 1;
    for id in &order {
        if let Some(c) = graph.component(id) {
            if c.start.is_empty() && c.prepare.is_empty() {
                continue;
            }
            println!("  {}. [component] {} ({:?})", step, c.id, c.role);
            for p in &c.prepare {
                if p.starts_with("__axiom_") {
                    continue;
                }
                println!("       prepare: {}", p);
            }
            for s in &c.start {
                println!("       start:   {}", s);
            }
            if !c.ports.is_empty() {
                println!("       ports:   {:?}", c.ports);
            }
            step += 1;
        } else if let Some(p) = graph.provider(id) {
            if p.id.starts_with("system-") {
                continue; // don't list toolchain as startup steps
            }
            println!("  {}. [provider]  {} ({})", step, p.name, if p.reachable { "reachable" } else { "not reachable" });
            if let Some(ref cmd) = p.start_command {
                if p.can_start && !p.reachable {
                    println!("       start:   {}", cmd);
                }
            }
            step += 1;
        }
    }
    println!();

    // Handle user-external providers that need starting
    for id in &order {
        if let Some(p) = graph.provider(id) {
            if p.id.starts_with("system-") {
                continue;
            }
            if p.ownership == Ownership::UserExternal && p.can_start && !p.reachable {
                println!("{} is installed but not reachable.", p.name);
                if let Some(ref cmd) = p.start_command {
                    let ok = util::prompt_yes_no(&format!("Start {} ({})?", p.name, cmd), true);
                    if ok {
                        println!("✓ Starting provider {} — {}", p.name, cmd);
                        let _child = spawn_background(Path::new("."), cmd, verbose)?;
                        // brief wait + re-probe
                        thread::sleep(Duration::from_secs(2));
                        let ready = p.ports.iter().any(|port| tcp_open(*port))
                            || !p.endpoints.is_empty();
                        if ready {
                            println!("  ✓ {} ready", p.name);
                        } else {
                            println!("  · {} started; readiness could not be verified", p.name);
                        }
                    } else {
                        println!("  · skipping {}", p.name);
                    }
                }
            } else if !p.installed && p.ownership == Ownership::UserExternal {
                println!("✗ Required provider not installed: {}", p.name);
                println!("  Install it separately, then re-run axiom.");
            } else if p.reachable {
                println!("✓ Provider {} ready", p.name);
            }
        }
    }

    // Prepare + start project components
    let mut children: Vec<ManagedProcess> = Vec::new();
    let mut all_ready = true;

    for id in &order {
        let Some(c) = graph.component(id) else { continue };
        // Reconstruct PreparePlan from encoded prepare list
        if !c.prepare.is_empty() {
            let axiom_owned = crate::deps::is_axiom_owned_workspace(&c.info.path);
            let mut reasons = Vec::new();
            let mut commands = Vec::new();
            let mut remove_first = None;
            for prep in &c.prepare {
                if let Some(rest) = prep.strip_prefix("__axiom_reason__") {
                    reasons.push(rest.to_string());
                } else if let Some(rest) = prep.strip_prefix("__axiom_remove__") {
                    remove_first = Some(std::path::PathBuf::from(rest));
                } else {
                    commands.push(prep.clone());
                }
            }
            if !commands.is_empty() || remove_first.is_some() {
                println!("✓ Preparing {}...", c.id);
                if c.info.has_package_json {
                    println!("  ✓ package.json found");
                }
                if c.info.path.join("package-lock.json").is_file() {
                    println!("  ✓ package-lock.json found");
                }
                let plan = crate::deps::PreparePlan {
                    commands,
                    remove_first,
                    reasons,
                };
                crate::deps::execute_prepare(&c.info.path, &plan, axiom_owned, verbose)?;
            } else if c.info.has_package_json {
                // Fast path: healthy deps, nothing to do
                println!("✓ {} dependencies ready (reusing)", c.id);
            }
        }
        if c.start.is_empty() {
            continue;
        }
        for start_cmd in &c.start {
            println!("✓ Starting {} — {}", c.id, start_cmd);
            let child = spawn_background(&c.info.path, start_cmd, verbose)?;
            let mut mp = ManagedProcess {
                id: c.id.clone(),
                child,
                cmd: start_cmd.clone(),
                ready: false,
            };
            match wait_ready(c, &mut mp, Duration::from_secs(45), verbose) {
                ReadyState::Ready => {
                    println!("  ✓ {} ready", c.id);
                    mp.ready = true;
                }
                ReadyState::StartedUnknown => {
                    println!("  · {} started; readiness could not be verified", c.id);
                    all_ready = false;
                }
                ReadyState::Failed(msg) => {
                    let _ = mp.child.kill();
                    let _ = mp.child.wait();
                    for prev in children.iter_mut().rev() {
                        let _ = prev.child.kill();
                        let _ = prev.child.wait();
                    }
                    bail!("{} failed: {}", c.id, msg);
                }
            }
            children.push(mp);
        }
    }

    print_final_status(graph, &children, all_ready);

    Ok(RunningApp {
        children,
        graph: graph.clone(),
    })
}

enum ReadyState {
    Ready,
    StartedUnknown,
    Failed(String),
}

fn wait_ready(c: &Component, mp: &mut ManagedProcess, timeout: Duration, verbose: bool) -> ReadyState {
    let deadline = Instant::now() + timeout;
    thread::sleep(Duration::from_millis(400));
    match mp.child.try_wait() {
        Ok(Some(status)) => return ReadyState::Failed(format!("exited early with {}", status)),
        Ok(None) => {}
        Err(e) => return ReadyState::Failed(e.to_string()),
    }

    // Prefer HTTP/TCP checks on discovered ports
    if !c.ports.is_empty() {
        while Instant::now() < deadline {
            // Process must still be alive
            if let Ok(Some(status)) = mp.child.try_wait() {
                return ReadyState::Failed(format!("exited with {}", status));
            }
            for port in &c.ports {
                match check_port_ready(*port, verbose) {
                    PortCheck::Ready => {
                        if let Ok(Some(status)) = mp.child.try_wait() {
                            return ReadyState::Failed(format!("exited with {}", status));
                        }
                        return ReadyState::Ready;
                    }
                    PortCheck::OpenTcpOnly => {
                        // TCP open is enough for non-HTTP or quirky HTTP servers
                        if let Ok(Some(status)) = mp.child.try_wait() {
                            return ReadyState::Failed(format!("exited with {}", status));
                        }
                        return ReadyState::Ready;
                    }
                    PortCheck::Closed => {}
                }
            }
            thread::sleep(Duration::from_millis(300));
        }
        return ReadyState::StartedUnknown;
    }

    // No ports known — process still alive after brief wait
    thread::sleep(Duration::from_millis(800));
    match mp.child.try_wait() {
        Ok(Some(status)) => ReadyState::Failed(format!("exited with {}", status)),
        Ok(None) => ReadyState::StartedUnknown,
        Err(e) => ReadyState::Failed(e.to_string()),
    }
}

#[derive(Debug)]
enum PortCheck {
    Ready,       // HTTP reachable (any meaningful response)
    OpenTcpOnly, // TCP accepts connections
    Closed,
}

/// Protocol-aware port readiness. Never treats HEAD-unsupported as failure.
fn check_port_ready(port: u16, verbose: bool) -> PortCheck {
    if !tcp_open(port) {
        return PortCheck::Closed;
    }
    // Try HTTP on loopback — many local services are HTTP
    let base = format!("http://127.0.0.1:{}", port);
    match http_probe(&base) {
        HttpProbe::Reachable(code) => {
            if verbose {
                println!("  · HTTP {} on :{} → reachable", code, port);
            }
            PortCheck::Ready
        }
        HttpProbe::Unreachable => {
            // TCP is open; may be non-HTTP (DB, raw TCP, etc.)
            if verbose {
                println!("  · TCP open on :{} (non-HTTP or probe failed)", port);
            }
            PortCheck::OpenTcpOnly
        }
    }
}

#[derive(Debug)]
enum HttpProbe {
    Reachable(u16), // status code if known, or 0
    Unreachable,
}

/// HEAD first; on 405/501/other "method not allowed" style failures, fall back to GET.
/// Any HTTP response (including 4xx/5xx from an actual server) counts as reachable.
fn http_probe(base_url: &str) -> HttpProbe {
    match http_status("HEAD", base_url) {
        Some(code) if code == 405 || code == 501 || code == 0 => http_get_probe(base_url),
        Some(code) if code > 0 => HttpProbe::Reachable(code),
        _ => http_get_probe(base_url),
    }
}

fn http_get_probe(base_url: &str) -> HttpProbe {
    match http_status("GET", base_url) {
        Some(code) if code > 0 => HttpProbe::Reachable(code),
        _ => HttpProbe::Unreachable,
    }
}

fn http_status(method: &str, url: &str) -> Option<u16> {
    crate::platform::http_request(method, url, 2000).map(|(code, _, _)| code)
}

fn tcp_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(250),
    )
    .is_ok()
}

/// Evidence that an HTTP endpoint is a user-facing application UI.
#[derive(Debug, Clone)]
struct EndpointEval {
    component_id: String,
    port: u16,
    url: String,
    /// Higher = more likely the app the user should open
    ui_score: i32,
    api_score: i32,
    content_type: String,
    title: Option<String>,
    notes: Vec<String>,
}

fn evaluate_http_endpoint(component_id: &str, port: u16) -> Option<EndpointEval> {
    if !tcp_open(port) {
        return None;
    }
    let url = format!("http://127.0.0.1:{}", port);
    // Fetch body with GET (not HEAD) — we need content
    let (status, content_type_raw, body) = match crate::platform::http_request("GET", &url, 3000) {
        Some(v) => v,
        None => {
            return Some(EndpointEval {
                component_id: component_id.to_string(),
                port,
                url,
                ui_score: 5,
                api_score: 5,
                content_type: "unknown".into(),
                title: None,
                notes: vec!["TCP reachable; HTTP body unavailable".into()],
            });
        }
    };
    let content_type = content_type_raw.to_lowercase();
    let size = body.len();
    let body_l = body.to_lowercase();

    let mut ui_score: i32 = 0;
    let mut api_score: i32 = 0;
    let mut notes = Vec::new();

    // Content-Type signals
    if content_type.contains("text/html") {
        ui_score += 40;
        notes.push("Content-Type: text/html".into());
    }
    if content_type.contains("application/json") {
        api_score += 40;
        notes.push("Content-Type: application/json".into());
    }
    if content_type.contains("text/plain") {
        api_score += 10;
    }

    // HTML structure signals
    if body_l.contains("<html") || body_l.contains("<!doctype") {
        ui_score += 30;
        notes.push("HTML document".into());
    }
    if body_l.contains("<div") || body_l.contains("<main") || body_l.contains("<body") {
        ui_score += 10;
    }
    if body_l.contains("<form") || body_l.contains("<input") || body_l.contains("<textarea") {
        ui_score += 15;
        notes.push("interactive form controls".into());
    }
    if body_l.contains("<script") {
        ui_score += 10;
        notes.push("contains scripts".into());
    }
    // SPA root markers
    if body_l.contains("id=\"root\"")
        || body_l.contains("id='root'")
        || body_l.contains("id=\"app\"")
        || body_l.contains("__next")
        || body_l.contains("data-reactroot")
    {
        ui_score += 20;
        notes.push("SPA application shell".into());
    }

    // Title
    let title = extract_html_title(&body);
    if let Some(ref t) = title {
        if t.len() > 1 {
            ui_score += 10;
            notes.push(format!("title: {}", t));
        }
    }

    // Vite / webpack dev server fingerprint → development layer, not primary product UI
    if body_l.contains("@vite/client")
        || body_l.contains("/@vite/")
        || body_l.contains("vite/dist/client")
        || body_l.contains("__vite_plugin")
    {
        ui_score -= 35;
        notes.push("Vite dev client fingerprint (dev server)".into());
    }
    if body_l.contains("webpack-dev-server") || body_l.contains("__webpack_dev_server") {
        ui_score -= 25;
        notes.push("webpack-dev-server fingerprint".into());
    }

    // Pure JSON API shape
    let trimmed = body.trim();
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && !body_l.contains("<html") {
        api_score += 25;
        notes.push("JSON body".into());
        // tiny health payloads are strongly API
        if size < 200 {
            api_score += 15;
            ui_score -= 10;
            notes.push("small JSON payload".into());
        }
    }

    // Size: real UIs are usually larger than health checks
    if content_type.contains("text/html") {
        if size > 1500 {
            ui_score += 15;
        } else if size < 400 {
            ui_score -= 10;
            notes.push("very small HTML".into());
        }
    }

    // OpenAPI / swagger hints → API docs (user-facing-ish but secondary)
    if body_l.contains("swagger") || body_l.contains("openapi") {
        ui_score += 5;
        api_score += 10;
        notes.push("API documentation UI".into());
    }

    Some(EndpointEval {
        component_id: component_id.to_string(),
        port,
        url,
        ui_score,
        api_score,
        content_type,
        title,
        notes,
    })
}

fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let start = lower.find("<title>")? + 7;
    let end = lower[start..].find("</title>")? + start;
    let t = body[start..end].trim();
    if t.is_empty() {
        None
    } else {
        Some(t.chars().take(80).collect())
    }
}

fn print_final_status(graph: &ApplicationGraph, children: &[ManagedProcess], all_ready: bool) {
    let missing_ports: Vec<u16> = graph
        .required_local_ports()
        .into_iter()
        .filter(|p| !tcp_open(*p))
        .collect();

    // Evaluate every reachable component port
    let mut evals: Vec<EndpointEval> = Vec::new();
    for c in &graph.components {
        for port in &c.ports {
            if let Some(ev) = evaluate_http_endpoint(&c.id, *port) {
                evals.push(ev);
            }
        }
    }
    // Also probe required_local_ports not tied to a component
    for p in graph.required_local_ports() {
        if evals.iter().any(|e| e.port == p) {
            continue;
        }
        if let Some(ev) = evaluate_http_endpoint("(discovered)", p) {
            evals.push(ev);
        }
    }

    println!();
    if all_ready && missing_ports.is_empty() && !children.is_empty() {
        println!("APPLICATION READY");
        println!();

        // Primary = highest ui_score among endpoints that look like UI more than API
        // If all are API-like, still pick highest ui_score and label accordingly
        evals.sort_by(|a, b| b.ui_score.cmp(&a.ui_score));

        if let Some(primary) = evals.first() {
            let is_ui = primary.ui_score >= primary.api_score && primary.ui_score >= 20;
            println!("Open:");
            println!("  {}", primary.url);
            if let Some(ref t) = primary.title {
                println!("  title: {}", t);
            }
            if is_ui {
                println!("  role: user-facing application");
            } else if primary.api_score > primary.ui_score {
                println!("  role: primary reachable service (API-like response)");
            } else {
                println!("  role: primary reachable service");
            }
            println!("  component: {}", primary.component_id);
            if !primary.notes.is_empty() {
                println!("  evidence: {}", primary.notes.join("; "));
            }
        }

        if evals.len() > 1 {
            println!();
            println!("Other services:");
            for ev in evals.iter().skip(1) {
                let kind = if ev.ui_score >= ev.api_score && ev.ui_score >= 20 {
                    "UI"
                } else if ev.api_score > ev.ui_score {
                    "API/internal"
                } else {
                    "service"
                };
                println!(
                    "  {}  ({}, component: {}, ui_score={}, api_score={})",
                    ev.url, kind, ev.component_id, ev.ui_score, ev.api_score
                );
            }
        }
    } else if !children.is_empty() {
        println!("PARTIALLY READY");
        for mp in children {
            println!("  {} {}", if mp.ready { "✓" } else { "·" }, mp.id);
        }
        for p in &missing_ports {
            println!("  ✗ port {} — not accepting connections", p);
        }
        if !evals.is_empty() {
            println!();
            println!("Reachable endpoints (partial):");
            evals.sort_by(|a, b| b.ui_score.cmp(&a.ui_score));
            for ev in &evals {
                println!(
                    "  {}  (ui_score={}, api_score={}, component: {})",
                    ev.url, ev.ui_score, ev.api_score, ev.component_id
                );
            }
        }
    } else {
        println!("No project processes started.");
    }
    println!();
    println!("Press Ctrl+C to stop all components.");
}

fn run_foreground(cwd: &Path, cmd_str: &str, _verbose: bool) -> Result<()> {
    let (prog, args) = crate::platform::parse_command_line(cmd_str);
    if prog.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new(&prog);
    for a in &args {
        cmd.arg(a);
    }
    // On Windows, .cmd must often be run via cmd.exe /C for correct PATH resolution
    #[cfg(windows)]
    {
        if prog.ends_with(".cmd") || prog.ends_with(".bat") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&prog);
            for a in &args {
                c.arg(a);
            }
            c.current_dir(cwd).stdout(Stdio::inherit()).stderr(Stdio::inherit());
            let status = c.status().with_context(|| format!("failed: {}", cmd_str))?;
            if !status.success() {
                bail!("{} failed with {}", cmd_str, status);
            }
            return Ok(());
        }
    }
    cmd.current_dir(cwd).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let status = cmd.status().with_context(|| format!("failed: {}", cmd_str))?;
    if !status.success() {
        bail!("{} failed with {}", cmd_str, status);
    }
    Ok(())
}

fn spawn_background(cwd: &Path, cmd_str: &str, _verbose: bool) -> Result<Child> {
    let (prog, args) = crate::platform::parse_command_line(cmd_str);
    if prog.is_empty() {
        bail!("empty command");
    }
    #[cfg(windows)]
    {
        if prog.ends_with(".cmd") || prog.ends_with(".bat") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&prog);
            for a in &args {
                c.arg(a);
            }
            c.current_dir(cwd)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::null());
            return c.spawn().with_context(|| format!("spawn failed: {}", cmd_str));
        }
    }
    let mut cmd = Command::new(&prog);
    for a in &args {
        cmd.arg(a);
    }
    cmd.current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null());
    cmd.spawn().with_context(|| format!("spawn failed: {}", cmd_str))
}

pub fn supervise(app: &mut RunningApp) -> Result<()> {
    loop {
        thread::sleep(Duration::from_millis(500));
        let mut failed = None;
        for mp in &mut app.children {
            match mp.child.try_wait() {
                Ok(Some(status)) => {
                    failed = Some(format!("{} exited with {}", mp.id, status));
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    failed = Some(format!("{}: {}", mp.id, e));
                    break;
                }
            }
        }
        if let Some(msg) = failed {
            println!();
            println!("✗ {}", msg);
            for mp in &mut app.children {
                let alive = matches!(mp.child.try_wait(), Ok(None));
                println!("  {} — {}", mp.id, if alive { "still running" } else { "stopped" });
            }
            app.shutdown();
            bail!("component failed");
        }
    }
}
