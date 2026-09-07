//! External provider detection — Ollama, Docker, system toolchains, etc.
//! Ports and endpoints are never hardcoded as Axiom policy; defaults are
//! only used as *detection probes* when the project did not specify one.

use std::process::Command;
use std::net::TcpStream;
use std::time::Duration;
use crate::model::{Provider, Ownership, Readiness, ServiceRef, is_loopback};
use crate::detect;

mod ollama;
mod system;

pub use ollama::detect_ollama;
pub use system::{detect_system_node, detect_system_python};

/// Run all provider detectors relevant to the discovered service refs + project.
pub fn detect_providers(service_refs: &[ServiceRef], project_mentions: &[String]) -> Vec<Provider> {
    let mut providers = Vec::new();

    // System toolchains in parallel (independent)
    let node_h = std::thread::spawn(detect_system_node);
    let py_h = std::thread::spawn(detect_system_python);
    if let Ok(Some(p)) = node_h.join() {
        providers.push(p);
    }
    if let Ok(Some(p)) = py_h.join() {
        providers.push(p);
    }

    // Ollama: only if project evidence suggests it
    let mentions_ollama = project_mentions.iter().any(|m| m.to_lowercase().contains("ollama"))
        || service_refs.iter().any(|r| {
            r.source.to_lowercase().contains("ollama")
                || r.raw.to_lowercase().contains("ollama")
        });
    // Also if a service ref points at a host that looks like ollama path in source
    if mentions_ollama {
        // Collect candidate endpoints from service refs (dynamic ports)
        let endpoints: Vec<(String, Option<u16>)> = service_refs
            .iter()
            .filter(|r| {
                r.source.to_lowercase().contains("ollama")
                    || r.raw.to_lowercase().contains("ollama")
                    || project_mentions.iter().any(|m| {
                        let ml = m.to_lowercase();
                        ml.contains("ollama") && r.port.map(|p| m.contains(&p.to_string())).unwrap_or(false)
                    })
            })
            .map(|r| (r.host.clone(), r.port))
            .collect();
        if let Some(p) = detect_ollama(&endpoints) {
            providers.push(p);
        }
    } else {
        // Still try default probe only when "ollama" appears anywhere in mentions
        // (already false) — skip
    }

    // Docker if mentioned
    if project_mentions.iter().any(|m| {
        let l = m.to_lowercase();
        l.contains("docker") || l.contains("compose")
    }) {
        if let Some(p) = detect_docker() {
            providers.push(p);
        }
    }

    providers
}

fn detect_docker() -> Option<Provider> {
    let installed = detect::has_command("docker");
    let mut running = false;
    let mut version = None;
    if installed {
        if let Ok(out) = Command::new("docker").args(["version", "--format", "{{.Server.Version}}"]).output() {
            if out.status.success() {
                running = true;
                version = Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
        }
    }
    Some(Provider {
        id: "docker".into(),
        name: "Docker".into(),
        ownership: Ownership::UserExternal,
        installed,
        running,
        reachable: running,
        version,
        can_start: false, // starting docker daemon is privileged; don't claim we can
        start_command: None,
        endpoints: vec![],
        evidence: vec!["project references docker/compose".into()],
        readiness: if running {
            Readiness::Ready
        } else if installed {
            Readiness::Unavailable("daemon not reachable".into())
        } else {
            Readiness::Unavailable("not installed".into())
        },
        ports: vec![],
    })
}

/// TCP probe helper for provider detectors.
pub fn tcp_open(host: &str, port: u16) -> bool {
    let addr = format!("{}:{}", host, port);
    addr.parse::<std::net::SocketAddr>()
        .ok()
        .and_then(|a| TcpStream::connect_timeout(&a, Duration::from_millis(300)).ok())
        .is_some()
}
