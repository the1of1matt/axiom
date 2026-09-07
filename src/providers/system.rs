use std::process::Command;
use crate::model::{Provider, Ownership, Readiness};
use crate::detect;

pub fn detect_system_node() -> Option<Provider> {
    let installed = detect::has_command("node");
    let mut version = None;
    if installed {
        if let Ok(out) = Command::new("node").arg("--version").output() {
            version = Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
        }
    }
    Some(Provider {
        id: "system-node".into(),
        name: "Node.js".into(),
        ownership: Ownership::UserExternal,
        installed,
        running: installed, // interpreter is available
        reachable: installed,
        version,
        can_start: false,
        start_command: None,
        endpoints: vec![],
        evidence: vec!["system toolchain".into()],
        readiness: if installed { Readiness::Ready } else { Readiness::Unavailable("not installed".into()) },
        ports: vec![],
    })
}

pub fn detect_system_python() -> Option<Provider> {
    let installed = detect::has_command("python3") || detect::has_command("python");
    let mut version = None;
    if installed {
        let bin = if detect::has_command("python3") { "python3" } else { "python" };
        if let Ok(out) = Command::new(bin).arg("--version").output() {
            let v = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            version = Some(v.trim().to_string());
        }
    }
    Some(Provider {
        id: "system-python".into(),
        name: "Python".into(),
        ownership: Ownership::UserExternal,
        installed,
        running: installed,
        reachable: installed,
        version,
        can_start: false,
        start_command: None,
        endpoints: vec![],
        evidence: vec!["system toolchain".into()],
        readiness: if installed { Readiness::Ready } else { Readiness::Unavailable("not installed".into()) },
        ports: vec![],
    })
}
