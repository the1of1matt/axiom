//! Ollama provider detection.
//! Endpoint ports come from the project when available; a well-known default
//! is only used as a *probe* when the project names Ollama but omits a port.

use std::process::Command;
use crate::model::{Provider, Ownership, Readiness};
use crate::detect;
use super::tcp_open;

/// `endpoints` = (host, port) pairs discovered from the project.
pub fn detect_ollama(endpoints: &[(String, Option<u16>)]) -> Option<Provider> {
    let installed = detect::has_command("ollama");
    let mut version = None;
    if installed {
        if let Ok(out) = Command::new("ollama").arg("--version").output() {
            let v = String::from_utf8_lossy(&out.stdout);
            let v2 = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{}{}", v, v2);
            let t = combined.trim();
            if !t.is_empty() {
                version = Some(t.lines().next().unwrap_or(t).to_string());
            }
        }
    }

    // Build probe list: project endpoints first, then optional default probe
    let mut probes: Vec<(String, u16)> = Vec::new();
    for (host, port) in endpoints {
        if let Some(p) = port {
            probes.push((host.clone(), *p));
        }
    }
    // If project named Ollama but gave no port, probe the common default API port.
    // This is a detection heuristic, not a hard requirement that apps use it.
    if probes.is_empty() {
        probes.push(("127.0.0.1".into(), 11434));
    }

    let mut reachable = false;
    let mut running = false;
    let mut used_ports = Vec::new();
    let mut used_endpoints = Vec::new();

    for (host, port) in &probes {
        if tcp_open(host, *port) {
            reachable = true;
            running = true;
            used_ports.push(*port);
            used_endpoints.push(format!("{}:{}", host, port));
            // Try HTTP API tags as stronger signal
            let url = format!("http://{}:{}/api/tags", host, port);
            if http_ok(&url) {
                break;
            }
        }
    }

    // CLI says server might be up even without our probe matching
    if installed && !running {
        if let Ok(out) = Command::new("ollama").args(["list"]).output() {
            // if command returns quickly with success, daemon is likely up
            if out.status.success() {
                running = true;
                reachable = true;
            }
        }
    }

    let can_start = installed; // `ollama serve` is user-owned start
    let start_command = if can_start {
        Some("ollama serve".into())
    } else {
        None
    };

    let readiness = if reachable {
        Readiness::Ready
    } else if installed {
        Readiness::Unavailable("installed but not reachable".into())
    } else {
        Readiness::Unavailable("not installed".into())
    };

    Some(Provider {
        id: "ollama".into(),
        name: "Ollama".into(),
        ownership: Ownership::UserExternal,
        installed,
        running,
        reachable,
        version,
        can_start,
        start_command,
        endpoints: used_endpoints,
        evidence: vec!["project references Ollama".into()],
        readiness,
        ports: used_ports,
    })
}

fn http_ok(url: &str) -> bool {
    crate::platform::http_request("GET", url, 2000)
        .map(|(code, _, _)| code > 0 && code < 500)
        .unwrap_or(false)
}
