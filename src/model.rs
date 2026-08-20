//! Core Application Graph model — components, providers, readiness.

use std::path::PathBuf;
use crate::detect::{ProjectKind, ProjectInfo};

/// Who owns this piece of the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership {
    /// Code inside the project; Axiom may prepare/start/stop.
    Project,
    /// User-installed local software; Axiom may detect/start with approval.
    UserExternal,
    /// Remote API / cloud; Axiom only verifies config, never starts a process.
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Unknown,
    Starting,
    Ready,
    Failed(String),
    Stopped,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRole {
    Frontend,
    Backend,
    Runtime,
    Worker,
    Desktop,
    Database,
    Cache,
    Unknown,
}

/// A project-owned runnable unit (discovered under the project tree).
#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub info: ProjectInfo,
    pub role: ComponentRole,
    pub ownership: Ownership,
    pub prepare: Vec<String>,
    pub start: Vec<String>,
    /// Ports this component is expected to provide (discovered, never hardcoded).
    pub ports: Vec<u16>,
    pub health_url: Option<String>,
    pub readiness: Readiness,
    pub script_path: Option<PathBuf>,
    pub evidence: Vec<String>,
}

/// External capability the application needs (Ollama, Docker, system runtimes, …).
#[derive(Debug, Clone)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub ownership: Ownership,
    pub installed: bool,
    pub running: bool,
    pub reachable: bool,
    pub version: Option<String>,
    pub can_start: bool,
    pub start_command: Option<String>,
    pub endpoints: Vec<String>,
    pub evidence: Vec<String>,
    pub readiness: Readiness,
    /// Ports associated with this provider if discovered from the project.
    pub ports: Vec<u16>,
}

/// A service URL/port reference found in project config or source.
#[derive(Debug, Clone)]
pub struct ServiceRef {
    pub host: String,
    pub port: Option<u16>,
    pub raw: String,
    pub source: String,
    /// Confidence: config/source > scripts > readme
    pub weight: u8,
}

/// Full application model.
#[derive(Debug, Clone)]
pub struct ApplicationGraph {
    pub root: PathBuf,
    pub name: String,
    pub components: Vec<Component>,
    pub providers: Vec<Provider>,
    pub service_refs: Vec<ServiceRef>,
    /// Edges: (dependent_id, dependency_id)
    pub edges: Vec<(String, String)>,
}

impl ApplicationGraph {
    /// Topological order: dependencies first. IDs may be component or provider ids.
    pub fn start_order_ids(&self) -> Vec<String> {
        let mut ordered = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp = std::collections::HashSet::new();

        let all_ids: Vec<String> = self
            .components
            .iter()
            .map(|c| c.id.clone())
            .chain(self.providers.iter().map(|p| p.id.clone()))
            .collect();

        fn visit(
            id: &str,
            graph: &ApplicationGraph,
            visited: &mut std::collections::HashSet<String>,
            temp: &mut std::collections::HashSet<String>,
            ordered: &mut Vec<String>,
        ) {
            if visited.contains(id) {
                return;
            }
            if temp.contains(id) {
                return;
            }
            temp.insert(id.to_string());
            for (from, to) in &graph.edges {
                if from == id {
                    visit(to, graph, visited, temp, ordered);
                }
            }
            temp.remove(id);
            visited.insert(id.to_string());
            ordered.push(id.to_string());
        }

        for id in &all_ids {
            visit(id, self, &mut visited, &mut temp, &mut ordered);
        }
        ordered
    }

    pub fn plan_fingerprint(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for c in &self.components {
            parts.push(format!("c:{}:{}:{}", c.id, c.start.join(";"), c.prepare.join(";")));
        }
        for p in &self.providers {
            parts.push(format!("p:{}:{}:{}", p.id, p.can_start, p.start_command.as_deref().unwrap_or("")));
        }
        parts.sort();
        parts.join("|")
    }

    pub fn required_local_ports(&self) -> Vec<u16> {
        let mut ports = Vec::new();
        for r in &self.service_refs {
            if let Some(p) = r.port {
                if r.host == "127.0.0.1" || r.host == "localhost" {
                    if !ports.contains(&p) {
                        ports.push(p);
                    }
                }
            }
        }
        ports
    }

    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }

    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }
}

/// Classify host as local loopback vs remote.
pub fn is_loopback(host: &str) -> bool {
    let h = host.to_lowercase();
    h == "localhost" || h == "127.0.0.1" || h == "::1" || h == "0.0.0.0"
}
