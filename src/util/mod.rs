use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Axiom home directory: ~/.axiom
pub fn axiom_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".axiom"))
}

pub fn ensure_axiom_dirs() -> Result<()> {
    let home = axiom_home()?;
    for sub in &["toolchains", "packages", "cache", "projects", "tmp"] {
        let p = home.join(sub);
        if !p.exists() {
            fs::create_dir_all(&p)
                .with_context(|| format!("failed to create {}", p.display()))?;
        }
    }
    Ok(())
}

pub fn is_safe_to_scan(path: &Path) -> bool {
    // Skip system and sensitive directories
    let path_str = path.to_string_lossy().to_lowercase();
    let skip_prefixes = [
        "/proc", "/sys", "/dev", "/run", "/tmp", "/var/tmp",
        "/boot", "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64",
        "/root", // avoid other users
    ];
    for prefix in &skip_prefixes {
        if path_str.starts_with(prefix) {
            return false;
        }
    }
    // Skip hidden dirs at top level of home (but allow deeper)
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') && name != ".axiom" {
            // Allow scanning inside user projects that happen to be hidden
            // but skip common large/irrelevant ones
            let skip_hidden = [".cache", ".npm", ".cargo", ".rustup", ".local", ".config",
                               ".Trash", ".docker", ".vscode", ".idea", ".git"];
            if skip_hidden.contains(&name) {
                return false;
            }
        }
    }
    true
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

pub fn print_header() {
    println!("AXIOM");
    println!();
}


// ---------------------------------------------------------------------------
// Trust store — remember which projects the user approved
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TrustStore {
    /// path string -> fingerprint of package.json / Cargo.toml etc.
    pub projects: HashMap<String, String>,
}

fn trust_path() -> Result<PathBuf> {
    Ok(axiom_home()?.join("trust.json"))
}

pub fn load_trust() -> TrustStore {
    let path = match trust_path() {
        Ok(p) => p,
        Err(_) => return TrustStore::default(),
    };
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => TrustStore::default(),
    }
}

pub fn save_trust(store: &TrustStore) -> Result<()> {
    ensure_axiom_dirs()?;
    let path = trust_path()?;
    let s = serde_json::to_string_pretty(store)?;
    fs::write(&path, s)?;
    Ok(())
}

/// Simple fingerprint of key project files (mtime + size + path).
pub fn project_fingerprint(project_path: &Path) -> String {
    let mut parts = Vec::new();
    for name in &["package.json", "Cargo.toml", "pyproject.toml", "go.mod", "main.py", "index.js"] {
        let p = project_path.join(name);
        if let Ok(meta) = fs::metadata(&p) {
            parts.push(format!("{}:{}:{}", name, meta.len(), meta.modified().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)).unwrap_or(0)));
        }
    }
    if parts.is_empty() {
        format!("path:{}", project_path.display())
    } else {
        parts.join("|")
    }
}

pub fn is_trusted(project_path: &Path) -> bool {
    let store = load_trust();
    let key = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    match store.projects.get(&key) {
        Some(fp) => fp == &project_fingerprint(project_path),
        None => false,
    }
}

pub fn grant_trust(project_path: &Path) -> Result<()> {
    let mut store = load_trust();
    let key = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    store.projects.insert(key, project_fingerprint(project_path));
    save_trust(&store)
}

pub fn prompt_yes_no(question: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    eprint!("{} {} ", question, hint);
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return default_yes;
    }
    let t = line.trim().to_lowercase();
    if t.is_empty() {
        return default_yes;
    }
    t.starts_with('y')
}


#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TrustStoreV2 {
    /// key -> plan fingerprint
    pub plans: HashMap<String, String>,
    /// legacy project fingerprints
    #[serde(default)]
    pub projects: HashMap<String, String>,
}

fn trust_path_v2() -> Result<PathBuf> {
    Ok(axiom_home()?.join("trust.json"))
}

pub fn is_trusted_plan(key: &str, plan_fp: &str) -> bool {
    let path = match trust_path_v2() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let store: TrustStoreV2 = match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => return false,
    };
    store.plans.get(key).map(|fp| fp == plan_fp).unwrap_or(false)
}

pub fn grant_trust_plan(key: &str, plan_fp: &str) -> Result<()> {
    ensure_axiom_dirs()?;
    let path = trust_path_v2()?;
    let mut store: TrustStoreV2 = match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => TrustStoreV2::default(),
    };
    store.plans.insert(key.to_string(), plan_fp.to_string());
    let s = serde_json::to_string_pretty(&store)?;
    fs::write(&path, s)?;
    Ok(())
}
