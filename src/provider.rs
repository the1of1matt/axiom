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
