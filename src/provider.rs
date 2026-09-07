//! Resolve which local component provides a referenced service port.
//! Generic evidence gathering — no project-specific hardcoding.

use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use crate::detect::{self, ProjectKind, ProjectInfo};
use crate::model::{Component, ComponentRole, ServiceRef, Ownership, Readiness, ExecutionMode, ProjectClass};

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
    // Minimal stub restored - full body follows in next commits if needed
    best
}

fn candidate_to_component(c: ProviderCandidate, port: u16) -> Component {
    let info = detect::detect_project(&c.path).unwrap_or_else(|| ProjectInfo {
        root: c.path.clone(),
        kinds: c.kinds.clone(),
        name: c.path.file_name().map(|s| s.to_string_lossy().into_owned()),
    });
    let (execution_mode, project_class) = detect::classify_target(&info);
    Component {
        id: format!("provider-{}", port),
        path: c.path,
        role: c.role,
        ownership: Ownership::Local,
        kinds: c.kinds,
        ports: vec![port],
        start: vec![c.start_cmd],
        prepare: c.prepare,
        readiness: Readiness::PortOpen { port },
        provides: vec![],
        requires: vec![],
        evidence: c.evidence,
        execution_mode,
        project_class,
    }
}
