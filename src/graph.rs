//! Build ApplicationGraph from a project root.

use std::path::{Path, PathBuf};
use std::fs;
use serde_json::Value;
use crate::detect::{self, ProjectKind, ProjectInfo};
use crate::model::{
    self, ApplicationGraph, Component, ComponentRole, Ownership, Readiness, ServiceRef,
};
use crate::provider;
use crate::providers;

pub fn build_application(root: &Path) -> ApplicationGraph {
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("application")
        .to_string();

    // 1. Project components — one directory may expand into MULTIPLE runtimes
    let projects = detect::find_projects_under(root, 12);
    let mut components: Vec<Component> = Vec::new();
    for info in projects {
        components.extend(expand_runtime_components(info));
    }
    dedupe_components(&mut components);

    for c in &mut components {
        c.role = infer_role(c);
        refine_start_commands(c);
        // Ports are scoped to THIS runtime only (see expand + discover_ports_for)
        if c.ports.is_empty() {
            c.ports = discover_ports_for(c);
        }
        if let Some(p) = c.ports.first() {
            c.health_url = Some(format!("http://127.0.0.1:{}/", p));
        }
    }

    // 2. Service references (dynamic ports from project)
    let mut service_refs = discover_service_refs(root);
    service_refs.sort_by_key(|r| r.weight);

    // 3. Resolve unresolved local ports → new project-owned components (do NOT
    //    dump ports onto unrelated siblings that share a directory)
    let owned: Vec<u16> = components.iter().flat_map(|c| c.ports.clone()).collect();
    let unresolved: Vec<_> = service_refs
        .iter()
        .filter(|r| r.port.map(|p| !owned.contains(&p)).unwrap_or(false))
        .cloned()
        .collect();
    if !unresolved.is_empty() {
        let resolved = provider::resolve_providers(root, &unresolved, &components);
        for c in resolved {
            // Avoid duplicate if we already expanded a matching Python runtime
            let dup = components.iter().any(|e| {
                e.info.path == c.info.path
                    && e.start.iter().any(|s| c.start.iter().any(|s2| s2 == s))
            });
            if !dup {
                components.push(c);
            }
        }
    }

    // Unique ids
    uniquify_ids(&mut components);

    // 4. External providers (Ollama, Docker, system toolchains)
    let mentions = collect_mentions(root);
    let providers = providers::detect_providers(&service_refs, &mentions);

    // 5. Edges
    let edges = infer_edges(&components, &providers, &service_refs);

    ApplicationGraph {
        root: root.to_path_buf(),
        name,
        components,
        providers,
        service_refs,
        edges,
    }
}


/// Split a project directory into independent runtime components when evidence
/// shows multiple stacks (e.g. Node frontend + Python server in the same folder).
fn expand_runtime_components(info: ProjectInfo) -> Vec<Component> {
    let mut out = Vec::new();

    let has_node = info.has_package_json
        || info.kinds.contains(&ProjectKind::Node)
        || info.kinds.contains(&ProjectKind::Vite)
        || info.kinds.contains(&ProjectKind::React)
        || info.kinds.contains(&ProjectKind::Electron);

    let py_entry = [
        "server.py", "app.py", "main.py", "run.py", "wsgi.py", "asgi.py", "src/main.py",
    ]
    .iter()
    .find(|f| info.path.join(f).is_file())
    .map(|s| s.to_string());

    let has_python = info.kinds.contains(&ProjectKind::Python)
        || info.has_requirements
        || info.has_pyproject
        || py_entry.is_some();

    let has_rust = info.has_cargo_toml || info.kinds.contains(&ProjectKind::Rust);
    let has_go = info.has_go_mod || info.kinds.contains(&ProjectKind::Go);

    let multi = [has_node, has_python, has_rust, has_go]
        .iter()
        .filter(|&&x| x)
        .count()
        >= 2;

    if !multi {
        // Single-stack directory → one component
        out.push(component_from_info(info));
        return out;
    }

    // --- Node / JS runtime ---
    if has_node {
        let mut node_info = info.clone();
        node_info.kinds.retain(|k| {
            matches!(
                k,
                ProjectKind::Node
                    | ProjectKind::Vite
                    | ProjectKind::React
                    | ProjectKind::Electron
                    | ProjectKind::Tauri
            )
        });
        if node_info.kinds.is_empty() {
            node_info.kinds.push(ProjectKind::Node);
        }
        node_info.markers.retain(|m| {
            m.contains("package") || m.contains("vite") || m.contains("react") || m.contains("electron")
        });
        node_info.has_requirements = false;
        node_info.has_pyproject = false;
        node_info.has_cargo_toml = false;
        node_info.has_go_mod = false;
        let mut c = component_from_info(node_info);
        c.id = format!("{}-web", info.name);
        c.role = ComponentRole::Frontend;
        // Only JS-related ports
        c.ports = discover_ports_scoped(&info.path, RuntimeScope::Node);
        out.push(c);
    }

    // --- Python runtime ---
    if has_python {
        let mut py_info = info.clone();
        py_info.kinds = vec![ProjectKind::Python];
        py_info.markers = vec!["python".into()];
        if let Some(ref e) = py_entry {
            py_info.markers.push(e.clone());
        }
        py_info.has_package_json = false;
        py_info.package_scripts = None;
        py_info.has_cargo_toml = false;
        py_info.has_go_mod = false;
        py_info.has_vite = false;
        let mut c = component_from_info(py_info);
        c.id = format!("{}-py", info.name);
        c.role = ComponentRole::Backend;
        // Prefer explicit python entry start
        if let Some(ref e) = py_entry {
            let py = if info.path.join(".venv/pyvenv.cfg").is_file() {
                ".venv/bin/python"
            } else if detect::has_command("python3") {
                "python3"
            } else {
                "python"
            };
            c.start = vec![format!("{} {}", py, e)];
            c.prepare = prepare_commands(&info);
            // Only keep python prepare
            c.prepare.retain(|p| p.contains("pip") || p.contains("requirements") || p.contains("venv"));
        }
        c.ports = discover_ports_scoped(&info.path, RuntimeScope::Python);
        out.push(c);
    }

    // --- Rust ---
    if has_rust {
        let mut r_info = info.clone();
        r_info.kinds = vec![ProjectKind::Rust];
        r_info.has_package_json = false;
        r_info.package_scripts = None;
        r_info.has_requirements = false;
        let mut c = component_from_info(r_info);
        c.id = format!("{}-rust", info.name);
        c.role = ComponentRole::Backend;
        c.ports = discover_ports_scoped(&info.path, RuntimeScope::Rust);
        out.push(c);
    }

    // --- Go ---
    if has_go {
        let mut g_info = info.clone();
        g_info.kinds = vec![ProjectKind::Go];
        g_info.has_package_json = false;
        g_info.package_scripts = None;
        let mut c = component_from_info(g_info);
        c.id = format!("{}-go", info.name);
        c.role = ComponentRole::Backend;
        c.ports = discover_ports_scoped(&info.path, RuntimeScope::Go);
        out.push(c);
    }

    if out.is_empty() {
        out.push(component_from_info(info));
    }
    out
}

#[derive(Clone, Copy)]
enum RuntimeScope {
    Node,
    Python,
    Rust,
    Go,
    Any,
}

fn discover_ports_for(c: &Component) -> Vec<u16> {
    let scope = if c.info.kinds.iter().any(|k| {
        matches!(
            k,
            ProjectKind::Node | ProjectKind::Vite | ProjectKind::React | ProjectKind::Electron
        )
    }) {
        RuntimeScope::Node
    } else if c.info.kinds.contains(&ProjectKind::Python) {
        RuntimeScope::Python
    } else if c.info.kinds.contains(&ProjectKind::Rust) {
        RuntimeScope::Rust
    } else if c.info.kinds.contains(&ProjectKind::Go) {
        RuntimeScope::Go
    } else {
        RuntimeScope::Any
    };
    discover_ports_scoped(&c.info.path, scope)
}

/// Discover ports from files relevant to a given runtime scope only.
fn discover_ports_scoped(dir: &Path, scope: RuntimeScope) -> Vec<u16> {
    let mut ports = Vec::new();
    let mut files: Vec<String> = Vec::new();

    match scope {
        RuntimeScope::Node => {
            files.extend(
                [
                    "package.json",
                    "vite.config.js",
                    "vite.config.ts",
                    "vite.config.mjs",
                    "next.config.js",
                    ".env",
                    ".env.local",
                    "server.js",
                    "server.ts",
                    "server-vite.js",
                    "index.js",
                ]
                .iter()
                .map(|s| s.to_string()),
            );
            if let Ok(s) = fs::read_to_string(dir.join("package.json")) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(scripts) = v.get("scripts").and_then(|x| x.as_object()) {
                        for val in scripts.values() {
                            if let Some(cmd) = val.as_str() {
                                for token in cmd.split_whitespace() {
                                    if token.ends_with(".js")
                                        || token.ends_with(".ts")
                                        || token.ends_with(".mjs")
                                    {
                                        files.push(token.to_string());
                                    }
                                }
                                // ports declared in the script string itself
                                for p in extract_ports_from_text(cmd) {
                                    if !ports.contains(&p) {
                                        ports.push(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        RuntimeScope::Python => {
            files.extend(
                [
                    "server.py",
                    "app.py",
                    "main.py",
                    "run.py",
                    ".env",
                    ".env.local",
                    "pyproject.toml",
                ]
                .iter()
                .map(|s| s.to_string()),
            );
        }
        RuntimeScope::Rust => {
            files.extend([".env", "config.toml"].iter().map(|s| s.to_string()));
        }
        RuntimeScope::Go => {
            files.extend([".env", "main.go"].iter().map(|s| s.to_string()));
        }
        RuntimeScope::Any => {
            return discover_ports(&ProjectInfo {
                path: dir.to_path_buf(),
                name: String::new(),
                kinds: vec![],
                markers: vec![],
                has_package_json: false,
                has_cargo_toml: false,
                has_pyproject: false,
                has_requirements: false,
                has_cmake: false,
                has_tauri: false,
                has_vite: false,
                has_go_mod: false,
                package_scripts: None,
            });
        }
    }

    for name in &files {
        if let Ok(s) = fs::read_to_string(dir.join(name)) {
            for p in extract_ports_from_text(&s) {
                if !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
    }
    ports
}

fn component_from_info(info: ProjectInfo) -> Component {
    let mut script_path = None;
    for m in &info.markers {
        if let Some(rest) = m.strip_prefix("script:") {
            script_path = Some(PathBuf::from(rest));
        }
    }
    let prepare = prepare_commands(&info);
    let start = if let Some(ref sp) = script_path {
        vec![crate::platform::shell_script_cmd(sp)]
    } else {
        start_commands(&info)
    };
    Component {
        id: info.name.clone(),
        info,
        role: ComponentRole::Unknown,
        ownership: Ownership::Project,
        prepare,
        start,
        ports: Vec::new(),
        health_url: None,
        readiness: Readiness::Unknown,
        script_path,
        evidence: vec![],
    }
}

fn prepare_commands(info: &ProjectInfo) -> Vec<String> {
    let owned = crate::deps::is_axiom_owned_workspace(&info.path);
    let plan = crate::deps::plan_prepare(info, owned);
    // Encode remove_first as a special prepare step so orchestrate can act on it
    let mut cmds = Vec::new();
    if let Some(ref rm) = plan.remove_first {
        cmds.push(format!("__axiom_remove__{}", rm.display()));
    }
    for r in &plan.reasons {
        cmds.push(format!("__axiom_reason__{}", r));
    }
    cmds.extend(plan.commands);
    cmds
}

fn start_commands(info: &ProjectInfo) -> Vec<String> {
    let mut cmds = Vec::new();
    match info.primary_kind() {
        ProjectKind::Node | ProjectKind::Vite | ProjectKind::React | ProjectKind::Electron | ProjectKind::Tauri => {
            if let Some(ref scripts) = info.package_scripts {
                if let Some(ref pref) = scripts.preferred {
                    cmds.push(format!("npm run {}", pref));
                } else if let Some(ref main) = scripts.main {
                    cmds.push(format!("node {}", main));
                }
            }
            if cmds.is_empty() && info.path.join("index.js").is_file() {
                cmds.push("node index.js".into());
            }
        }
        ProjectKind::Rust => cmds.push("cargo run".into()),
        ProjectKind::Python | ProjectKind::Shell => {
            for candidate in &["server.py", "app.py", "main.py", "run.py", "wsgi.py", "asgi.py", "src/main.py"] {
                if info.path.join(candidate).is_file() {
                    let py = if info.path.join(".venv/pyvenv.cfg").is_file() {
                        ".venv/bin/python"
                    } else if detect::has_command("python3") {
                        "python3"
                    } else {
                        "python"
                    };
                    cmds.push(format!("{} {}", py, candidate));
                    break;
                }
            }
        }
        ProjectKind::Go => cmds.push("go run .".into()),
        _ => {}
    }
    cmds
}

fn refine_start_commands(c: &mut Component) {
    if c.script_path.is_some() {
        return;
    }
    if let Ok(content) = fs::read_to_string(c.info.path.join("package.json")) {
        if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
            if let Some(sc) = pkg.get("scripts").and_then(|s| s.as_object()) {
                let role_scripts: &[&str] = match c.role {
                    ComponentRole::Backend | ComponentRole::Runtime => {
                        &["server", "serve", "backend", "api", "runtime", "start", "dev"]
                    }
                    ComponentRole::Frontend => &["dev", "start", "serve"],
                    ComponentRole::Desktop => &["start", "electron", "dev"],
                    ComponentRole::Worker => &["worker", "start", "dev"],
                    _ => return,
                };
                for name in role_scripts {
                    if sc.contains_key(*name) {
                        c.start = vec![format!("npm run {}", name)];
                        return;
                    }
                }
            }
        }
    }
}

fn infer_role(c: &Component) -> ComponentRole {
    let name_lower = c.info.name.to_lowercase();
    let path_lower = c.info.path.to_string_lossy().to_lowercase();
    if c.info.kinds.contains(&ProjectKind::Electron) || c.info.kinds.contains(&ProjectKind::Tauri) {
        return ComponentRole::Desktop;
    }
    let checks: &[(&[&str], ComponentRole)] = &[
        (&["runtime", "engine", "inference", "model", "llm"], ComponentRole::Runtime),
        (&["backend", "back-end", "api", "server", "service"], ComponentRole::Backend),
        (&["frontend", "front-end", "web", "ui", "client", "renderer"], ComponentRole::Frontend),
        (&["worker", "jobs", "queue"], ComponentRole::Worker),
    ];
    for (hints, role) in checks {
        for h in *hints {
            if name_lower.contains(h) || path_lower.contains(h) {
                return role.clone();
            }
        }
    }
    if c.info.has_vite || c.info.kinds.contains(&ProjectKind::React) {
        return ComponentRole::Frontend;
    }
    if c.info.kinds.contains(&ProjectKind::Python) || c.info.kinds.contains(&ProjectKind::Shell) {
        return ComponentRole::Backend;
    }
    ComponentRole::Unknown
}

fn infer_edges(
    components: &[Component],
    providers: &[model::Provider],
    _refs: &[ServiceRef],
) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    let by_role = |role: ComponentRole| -> Vec<String> {
        components
            .iter()
            .filter(|c| c.role == role)
            .map(|c| c.id.clone())
            .collect()
    };
    let frontends = by_role(ComponentRole::Frontend);
    let backends = by_role(ComponentRole::Backend);
    let runtimes = by_role(ComponentRole::Runtime);
    let desktops = by_role(ComponentRole::Desktop);

    for f in &frontends {
        for b in &backends {
            edges.push((f.clone(), b.clone()));
        }
        for r in &runtimes {
            edges.push((f.clone(), r.clone()));
        }
    }
    for b in &backends {
        for r in &runtimes {
            edges.push((b.clone(), r.clone()));
        }
    }
    for d in &desktops {
        for b in &backends {
            edges.push((d.clone(), b.clone()));
        }
        for r in &runtimes {
            edges.push((r.clone(), r.clone())); // no-op fix below
            edges.push((d.clone(), r.clone()));
        }
    }
    edges.retain(|(a, b)| a != b);

    // Frontend/Backend depend on external providers that are required
    for p in providers {
        if p.id == "system-node" || p.id == "system-python" {
            continue; // toolchain, not startup order dependency
        }
        if matches!(p.readiness, Readiness::Ready | Readiness::Unavailable(_)) {
            for f in &frontends {
                edges.push((f.clone(), p.id.clone()));
            }
            for b in &backends {
                edges.push((b.clone(), p.id.clone()));
            }
            for r in &runtimes {
                edges.push((r.clone(), p.id.clone()));
            }
        }
    }
    edges
}

fn discover_ports(info: &ProjectInfo) -> Vec<u16> {
    let mut ports = Vec::new();
    let mut files: Vec<String> = [
        "package.json", ".env", ".env.local", ".env.development",
        "vite.config.js", "vite.config.ts", "vite.config.mjs",
        "next.config.js", "docker-compose.yml", "compose.yml",
        "server.js", "server.ts", "server-vite.js", "index.js",
        "main.js", "app.js", "server.py", "app.py", "main.py",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // Also scan script targets from package.json
    if let Ok(s) = fs::read_to_string(info.path.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(scripts) = v.get("scripts").and_then(|x| x.as_object()) {
                for val in scripts.values() {
                    if let Some(cmd) = val.as_str() {
                        for token in cmd.split_whitespace() {
                            if token.ends_with(".js") || token.ends_with(".ts") || token.ends_with(".mjs") {
                                files.push(token.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    for name in &files {
        if let Ok(s) = fs::read_to_string(info.path.join(name)) {
            for p in extract_ports_from_text(&s) {
                if !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
    }
    ports
}

fn extract_ports_from_text(text: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let push = |ports: &mut Vec<u16>, p: u16| {
        if (1024..=65535).contains(&p) && !ports.contains(&p) {
            ports.push(p);
        }
    };
    for part in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '=' && c != ':' && c != '-') {
        if let Some(rest) = part.strip_prefix("PORT=") {
            if let Ok(p) = rest.parse::<u16>() {
                push(&mut ports, p);
            }
        }
        if let Some(rest) = part.strip_prefix("--port=") {
            if let Ok(p) = rest.parse::<u16>() {
                push(&mut ports, p);
            }
        }
    }
    // :NNNN
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if let Ok(p) = std::str::from_utf8(&bytes[i + 1..j]).unwrap_or("").parse::<u16>() {
                push(&mut ports, p);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    // listen(NNNN) / listen( NNNN )
    let lower = text.to_lowercase();
    for key in ["listen(", "port:"] {
        let mut rest = lower.as_str();
        while let Some(idx) = rest.find(key) {
            let after = &rest[idx + key.len()..];
            let digits: String = after.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(p) = digits.parse::<u16>() {
                push(&mut ports, p);
            }
            rest = &after[digits.len().max(1)..];
        }
    }
    ports
}

fn discover_service_refs(root: &Path) -> Vec<ServiceRef> {
    use walkdir::WalkDir;
    let mut reqs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let interesting = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json", ".env", ".yml", ".yaml", ".toml", ".md", ".py", ".sh"];

    for entry in WalkDir::new(root)
        .max_depth(8)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if let Some(name) = e.file_name().to_str() {
                let skip = ["node_modules", "target", ".git", "dist", "build", ".venv", "venv", "coverage"];
                if skip.contains(&name) {
                    return false;
                }
            }
            true
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let ok = interesting.iter().any(|e| name.ends_with(e)) || name.starts_with(".env");
        if !ok {
            continue;
        }
        if entry.metadata().map(|m| m.len()).unwrap_or(0) > 512_000 {
            continue;
        }
        let text = match fs::read_to_string(entry.path()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !text.contains("localhost") && !text.contains("127.0.0.1") && !text.contains("0.0.0.0") {
            continue;
        }
        let is_readme = name.contains("readme");
        let weight: u8 = if is_readme {
            30
        } else if name.starts_with(".env") || name.contains("vite.config") || name == "package.json" {
            5
        } else if name.ends_with(".ts") || name.ends_with(".js") || name.ends_with(".py") {
            10
        } else {
            15
        };
        for port in extract_ports_from_text(&text) {
            let key = format!("127.0.0.1:{}", port);
            if seen.insert(key.clone()) {
                reqs.push(ServiceRef {
                    host: "127.0.0.1".into(),
                    port: Some(port),
                    raw: key,
                    source: entry.path().display().to_string(),
                    weight,
                });
            }
        }
    }
    reqs
}

fn collect_mentions(root: &Path) -> Vec<String> {
    use walkdir::WalkDir;
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if let Some(name) = e.file_name().to_str() {
                let skip = ["node_modules", "target", ".git", "dist", "build", ".venv", "venv"];
                if skip.contains(&name) {
                    return false;
                }
            }
            true
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !(name.ends_with(".md")
            || name.ends_with(".json")
            || name.ends_with(".ts")
            || name.ends_with(".js")
            || name.ends_with(".py")
            || name.ends_with(".toml")
            || name.starts_with(".env")
            || name.ends_with(".yml"))
        {
            continue;
        }
        if entry.metadata().map(|m| m.len()).unwrap_or(0) > 200_000 {
            continue;
        }
        if let Ok(t) = fs::read_to_string(entry.path()) {
            let l = t.to_lowercase();
            for key in ["ollama", "docker", "postgres", "postgresql", "mysql", "redis", "openai", "lm studio"] {
                if l.contains(key) {
                    out.push(format!("{}:{}", entry.path().display(), key));
                }
            }
        }
    }
    out
}

fn dedupe_components(components: &mut Vec<Component>) {
    use std::collections::HashMap;
    let mut by_path: HashMap<PathBuf, Component> = HashMap::new();
    for c in components.drain(..) {
        let key = c.info.path.canonicalize().unwrap_or_else(|_| c.info.path.clone());
        if let Some(existing) = by_path.get(&key) {
            let prefer_new = existing.info.kinds.contains(&ProjectKind::Shell)
                && !c.info.kinds.contains(&ProjectKind::Shell);
            if prefer_new {
                by_path.insert(key, c);
            }
        } else {
            by_path.insert(key, c);
        }
    }
    *components = by_path.into_values().collect();
    let non_shell: Vec<PathBuf> = components
        .iter()
        .filter(|c| !c.info.kinds.contains(&ProjectKind::Shell))
        .map(|c| c.info.path.canonicalize().unwrap_or_else(|_| c.info.path.clone()))
        .collect();
    components.retain(|c| {
        if !c.info.kinds.contains(&ProjectKind::Shell) {
            return true;
        }
        if let Some(ref sp) = c.script_path {
            for np in &non_shell {
                if sp.starts_with(np) {
                    return false;
                }
            }
        }
        true
    });
}

fn uniquify_ids(components: &mut [Component]) {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in components.iter_mut() {
        let base = c.id.clone();
        let n = counts.entry(base.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            c.id = format!("{}-{}", base, n);
        }
    }
}

/// Print a human-readable application graph summary.
pub fn print_graph(g: &ApplicationGraph) {
    println!("Application:");
    println!("  {}", g.name);
    println!();
    println!("Components:");
    if g.components.is_empty() {
        println!("  (none)");
    }
    for c in &g.components {
        println!("  • {} ({:?}) — {}", c.id, c.role, c.info.display_kinds());
        println!("      {}", c.info.path.display());
        if !c.ports.is_empty() {
            println!("      provides ports: {:?}", c.ports);
        }
        if !c.evidence.is_empty() {
            for e in &c.evidence {
                println!("      evidence: {}", e);
            }
        }
    }
    println!();
    println!("Providers:");
    if g.providers.is_empty() {
        println!("  (none detected)");
    }
    for p in &g.providers {
        let status = match &p.readiness {
            Readiness::Ready => "ready",
            Readiness::Unavailable(m) => m.as_str(),
            _ => "unknown",
        };
        let mark = if p.reachable { "✓" } else { "·" };
        println!("  {} {} — {} ({})", mark, p.name, status, format!("{:?}", p.ownership).to_lowercase());
        if let Some(ref v) = p.version {
            println!("      version: {}", v);
        }
        if !p.endpoints.is_empty() {
            println!("      endpoints: {}", p.endpoints.join(", "));
        }
    }
    if !g.service_refs.is_empty() {
        println!();
        println!("Service references:");
        for r in &g.service_refs {
            println!("  • {}  (from {})", r.raw, r.source);
        }
    }
}
