//! Resolve which local component provides a referenced service port.
//! Generic evidence gathering — no project-specific hardcoding.

use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use crate::detect::{self, ProjectKind, ProjectInfo};
use crate::model::{Component, ComponentRole, ServiceRef, Ownership, Readiness};

#[derive(Debug, Clone)]
pub struct ProviderCandidate {
    pub path: PathBuf,
    pub start_cmd: String,
    pub prepare: Vec<String>,
    pub port: u16,
    pub evidence: Vec<String>,
    pub score: i32,
    pub role: ComponentRole,
    pub kinds: Vec<ProjectKind>,
}

/// For each unresolved requirement, search the project for a provider.
pub fn resolve_providers(
    root: &Path,
    requirements: &[ServiceRef],
    existing: &[Component],
) -> Vec<Component> {
    let mut new_components = Vec::new();
    let owned_ports: Vec<u16> = existing.iter().flat_map(|c| c.ports.clone()).collect();

    for req in requirements {
        let Some(port) = req.port else { continue };
        if owned_ports.contains(&port) {
            continue;
        }
        // Already resolved by a previous requirement
        if new_components.iter().any(|c: &Component| c.ports.contains(&port)) {
            continue;
        }

        if let Some(candidate) = find_provider_for_port(root, port) {
            let comp = candidate_to_component(candidate, port);
            new_components.push(comp);
        }
    }
    new_components
}

fn find_provider_for_port(root: &Path, port: u16) -> Option<ProviderCandidate> {
    let mut best: Option<ProviderCandidate> = None;

    let skip = [
        "node_modules", "target", ".git", "dist", "build", ".venv", "venv",
        "__pycache__", ".next", ".nuxt", "coverage", ".cache", ".axiom",
    ];

    for entry in WalkDir::new(root)
        .max_depth(10)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if let Some(name) = e.file_name().to_str() {
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
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        let ext_ok = name.ends_with(".py")
            || name.ends_with(".sh")
            || name.ends_with(".bash")
            || name.ends_with(".ts")
            || name.ends_with(".js")
            || name.ends_with(".mjs")
            || name.ends_with(".toml")
            || name.ends_with(".yml")
            || name.ends_with(".yaml")
            || name.ends_with(".json")
            || name.ends_with(".md")
            || name == "makefile"
            || name == "dockerfile"
            || name.starts_with(".env");
        if !ext_ok {
            continue;
        }
        if entry.metadata().map(|m| m.len()).unwrap_or(0) > 400_000 {
            continue;
        }
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let port_str = port.to_string();
        if !text.contains(&port_str) && !text_mentions_generic_server(&text) {
            // Still consider Python server files even without explicit port (default 8000 etc is guessed only with strong framework signal + requirement)
            if !(name.ends_with(".py") && text_mentions_generic_server(&text)) {
                continue;
            }
        }

        if let Some(cand) = score_file(root, path, &text, port, &name) {
            if best.as_ref().map(|b| b.score < cand.score).unwrap_or(true) {
                best = Some(cand);
            }
        }
    }

    // Also try synthesizing uvicorn from FastAPI modules even without port in file
    if best.as_ref().map(|b| b.score < 50).unwrap_or(true) {
        if let Some(cand) = find_fastapi_module(root, port) {
            if best.as_ref().map(|b| b.score < cand.score).unwrap_or(true) {
                best = Some(cand);
            }
        }
    }

    best
}

fn text_mentions_generic_server(text: &str) -> bool {
    let l = text.to_lowercase();
    [
        "fastapi", "flask", "uvicorn", "hypercorn", "aiohttp", "starlette",
        "gunicorn", "django", "tornado", "sanic", "litestar",
        "express()", "createServer", "listen(", "actix_web", "axum::",
        "rocket::", "gin.Default", "fiber.New",
    ]
    .iter()
    .any(|k| l.contains(&k.to_lowercase()))
}

fn score_file(root: &Path, path: &Path, text: &str, port: u16, name: &str) -> Option<ProviderCandidate> {
    let l = text.to_lowercase();
    let port_str = port.to_string();
    let mut evidence = Vec::new();
    let mut score: i32 = 0;
    let mut start_cmd: Option<String> = None;
    let mut prepare = Vec::new();
    let mut kinds = Vec::new();
    let mut role = ComponentRole::Backend;

    let is_readme = name == "readme.md" || name == "readme" || name.starts_with("readme.");
    let is_config = name.ends_with(".toml")
        || name.ends_with(".yml")
        || name.ends_with(".yaml")
        || name.ends_with(".json")
        || name.starts_with(".env")
        || name.starts_with("vite.config")
        || name == "dockerfile";
    let is_source = name.ends_with(".py")
        || name.ends_with(".ts")
        || name.ends_with(".js")
        || name.ends_with(".rs")
        || name.ends_with(".go");
    let is_script = name.ends_with(".sh") || name.ends_with(".bash") || name == "makefile";

    // --- Port presence ---
    let has_port = text.contains(&port_str);
    if has_port {
        score += if is_readme { 5 } else if is_config { 25 } else if is_script { 30 } else if is_source { 20 } else { 10 };
        evidence.push(format!("mentions port {}", port));
    }

    // --- Framework signals ---
    if l.contains("fastapi") {
        score += 40;
        evidence.push("FastAPI".into());
        kinds.push(ProjectKind::Python);
        role = ComponentRole::Runtime;
    }
    if l.contains("flask") {
        score += 35;
        evidence.push("Flask".into());
        kinds.push(ProjectKind::Python);
    }
    if l.contains("uvicorn") {
        score += 45;
        evidence.push("uvicorn".into());
        kinds.push(ProjectKind::Python);
        role = ComponentRole::Runtime;
    }
    if l.contains("hypercorn") || l.contains("gunicorn") {
        score += 35;
        evidence.push("ASGI/WSGI server".into());
        kinds.push(ProjectKind::Python);
    }
    if l.contains("aiohttp") || l.contains("starlette") {
        score += 30;
        kinds.push(ProjectKind::Python);
    }

    // --- Explicit command patterns in file ---
    if let Some(cmd) = extract_uvicorn_command(text, port) {
        score += 50;
        evidence.push(format!("explicit command: {}", cmd));
        start_cmd = Some(cmd);
        kinds.push(ProjectKind::Python);
        role = ComponentRole::Runtime;
    }
    if start_cmd.is_none() {
        if let Some(cmd) = extract_python_server_command(path, text, port) {
            score += 35;
            evidence.push(format!("python server: {}", cmd));
            start_cmd = Some(cmd);
            kinds.push(ProjectKind::Python);
        }
    }
    if start_cmd.is_none() && is_script {
        if let Some(cmd) = extract_shell_start(path, text, port) {
            score += 40;
            evidence.push(format!("shell start: {}", cmd));
            start_cmd = Some(cmd);
        }
    }

    // README: only a weak hint — require additional signal
    if is_readme {
        score = score.min(25); // cap README-only evidence
        if start_cmd.is_none() {
            // Parse README for uvicorn lines
            if let Some(cmd) = extract_uvicorn_command(text, port) {
                score += 15;
                start_cmd = Some(cmd);
                evidence.push("README startup command".into());
                kinds.push(ProjectKind::Python);
            }
        }
    }

    // Need minimum evidence
    if score < 30 && start_cmd.is_none() {
        return None;
    }
    if score < 40 && !has_port && start_cmd.is_none() {
        return None;
    }

    // Synthesize start command from FastAPI module path
    if start_cmd.is_none() && name.ends_with(".py") && (l.contains("fastapi") || l.contains("flask")) {
        let module = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("server");
        // Look for app = FastAPI() variable name
        let app_var = if l.contains("app = fastapi") || l.contains("app=fastapi") {
            "app"
        } else if l.contains("application = fastapi") {
            "application"
        } else {
            "app"
        };
        start_cmd = Some(format!(
                "python3 -m uvicorn {}:{} --host 127.0.0.1 --port {}",
                module, app_var, port
            ));
        score += 20;
        evidence.push("synthesized from FastAPI module".into());
    }

    let start_cmd = start_cmd?;

    // Prepare deps if requirements nearby
    let dir = path.parent().unwrap_or(root);
    if dir.join("requirements.txt").is_file() && !dir.join(".venv/pyvenv.cfg").is_file() {
        if detect::has_command("pip3") {
            prepare.push("pip3 install -r requirements.txt".into());
        } else if detect::has_command("pip") {
            prepare.push("pip install -r requirements.txt".into());
        }
    }
    if dir.join("pyproject.toml").is_file() {
        if detect::has_command("pip3") {
            prepare.push("pip3 install -e .".into());
        }
    }

    // Path-based role hints
    let path_l = path.to_string_lossy().to_lowercase();
    if path_l.contains("runtime") || path_l.contains("inference") || path_l.contains("llm") {
        role = ComponentRole::Runtime;
    }

    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        kinds.push(ProjectKind::Unknown);
    }

    Some(ProviderCandidate {
        path: dir.to_path_buf(),
        start_cmd,
        prepare,
        port,
        evidence,
        score,
        role,
        kinds,
    })
}

fn extract_uvicorn_command(text: &str, port: u16) -> Option<String> {
    for line in text.lines() {
        let t = line.trim().trim_start_matches('#').trim_start_matches('$').trim();
        let tl = t.to_lowercase();
        if tl.contains("uvicorn") && (t.contains(&port.to_string()) || !t.contains("--port")) {
            // Clean markdown backticks
            let cleaned = t.replace('`', "").replace("...", "").trim().to_string();
            if cleaned.to_lowercase().starts_with("uvicorn") {
                // Ensure port is present
                if cleaned.contains(&port.to_string()) {
                    return Some(cleaned);
                }
                return Some(format!("{} --host 127.0.0.1 --port {}", cleaned, port));
            }
            // "python -m uvicorn ..."
            if tl.contains("uvicorn") {
                let cleaned = t.replace('`', "").trim().to_string();
                if cleaned.contains(&port.to_string()) {
                    return Some(cleaned);
                }
                return Some(format!("{} --port {}", cleaned, port));
            }
        }
    }
    None
}

fn extract_python_server_command(path: &Path, text: &str, port: u16) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if !name.ends_with(".py") {
        return None;
    }
    let l = text.to_lowercase();
    // File that binds the port
    if text.contains(&port.to_string())
        && (l.contains("listen") || l.contains("run(") || l.contains("uvicorn") || l.contains(".serve"))
    {
        return Some(format!("python3 {}", name));
    }
    if l.contains("if __name__") && (l.contains("uvicorn.run") || l.contains("app.run")) {
        return Some(format!("python3 {}", name));
    }
    None
}

fn extract_shell_start(path: &Path, text: &str, port: u16) -> Option<String> {
    let port_str = port.to_string();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if t.contains(&port_str)
            && (t.contains("uvicorn")
                || t.contains("python")
                || t.contains("node")
                || t.contains("npm")
                || t.contains("cargo")
                || t.contains("go run"))
        {
            return Some(crate::platform::shell_script_cmd(path));
        }
    }
    // Script name suggests start and content has server tools
    let name = path.file_name()?.to_str()?.to_lowercase();
    if (name.contains("start") || name.contains("run") || name.contains("serve"))
        && text_mentions_generic_server(text)
    {
        return Some(crate::platform::shell_script_cmd(path));
    }
    None
}

fn find_fastapi_module(root: &Path, port: u16) -> Option<ProviderCandidate> {
    for entry in WalkDir::new(root)
        .max_depth(8)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if let Some(name) = e.file_name().to_str() {
                let skip = ["node_modules", ".git", ".venv", "venv", "dist", "build", "__pycache__"];
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
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("py") {
            continue;
        }
        let text = fs::read_to_string(path).ok()?;
        let l = text.to_lowercase();
        if !(l.contains("fastapi(") || l.contains("fastapi (")) {
            continue;
        }
        let module = path.file_stem()?.to_str()?;
        let app_var = if l.contains("app = fastapi") || l.contains("app=fastapi") {
            "app"
        } else {
            "app"
        };
        let dir = path.parent()?.to_path_buf();
        let start_cmd = format!(
            "python3 -m uvicorn {}:{} --host 127.0.0.1 --port {}",
            module, app_var, port
        );
        let mut prepare = Vec::new();
        if dir.join("requirements.txt").is_file() {
            if detect::has_command("pip3") {
                prepare.push("pip3 install -r requirements.txt".into());
            }
        }
        return Some(ProviderCandidate {
            path: dir,
            start_cmd,
            prepare,
            port,
            evidence: vec!["FastAPI application module".into()],
            score: 55,
            role: ComponentRole::Runtime,
            kinds: vec![ProjectKind::Python],
        });
    }
    None
}

fn candidate_to_component(c: ProviderCandidate, port: u16) -> Component {
    let name = c
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("service")
        .to_string();
    let id = format!("{}-{}", name, port);

    let info = ProjectInfo {
        path: c.path.clone(),
        name: id.clone(),
        kinds: c.kinds.clone(),
        markers: c.evidence.clone(),
        has_package_json: c.path.join("package.json").is_file(),
        has_cargo_toml: c.path.join("Cargo.toml").is_file(),
        has_pyproject: c.path.join("pyproject.toml").is_file(),
        has_requirements: c.path.join("requirements.txt").is_file(),
        has_cmake: false,
        has_tauri: false,
        has_vite: false,
        has_go_mod: false,
        package_scripts: None,
    };

    println!("  ↳ Resolved port {} → {} (score {})", port, c.start_cmd, c.score);
    for e in &c.evidence {
        println!("      evidence: {}", e);
    }

    Component {
        id,
        info,
        role: c.role,
        ownership: Ownership::Project,
        prepare: c.prepare,
        start: vec![c.start_cmd],
        ports: vec![port],
        health_url: Some(format!("http://127.0.0.1:{}/", port)),
        readiness: Readiness::Unknown,
        script_path: None,
        evidence: c.evidence,
    }
}
