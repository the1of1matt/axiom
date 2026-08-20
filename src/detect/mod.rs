use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProjectKind {
    Node,
    Vite,
    React,
    Python,
    Rust,
    CMake,
    Tauri,
    Electron,
    Go,
    Java,
    Shell,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageScripts {
    pub start: Option<String>,
    pub dev: Option<String>,
    pub serve: Option<String>,
    pub electron: Option<String>,
    pub preferred: Option<String>,
    pub main: Option<String>,
    /// All script names present
    pub all_names: Vec<String>,
}

impl PackageScripts {
    pub fn from_json(pkg: &Value) -> Self {
        let scripts = pkg.get("scripts").cloned().unwrap_or(Value::Null);
        let get = |name: &str| {
            scripts
                .get(name)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let start = get("start");
        let dev = get("dev");
        let serve = get("serve");
        let electron = get("electron");

        let mut all_names = Vec::new();
        if let Some(obj) = scripts.as_object() {
            for k in obj.keys() {
                all_names.push(k.clone());
            }
        }

        let preferred = if start.is_some() {
            Some("start".into())
        } else if dev.is_some() {
            Some("dev".into())
        } else if serve.is_some() {
            Some("serve".into())
        } else if electron.is_some() {
            Some("electron".into())
        } else {
            None
        };

        let main = pkg
            .get("main")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        PackageScripts {
            start,
            dev,
            serve,
            electron,
            preferred,
            main,
            all_names,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
    pub kinds: Vec<ProjectKind>,
    pub markers: Vec<String>,
    pub has_package_json: bool,
    pub has_cargo_toml: bool,
    pub has_pyproject: bool,
    pub has_requirements: bool,
    pub has_cmake: bool,
    pub has_tauri: bool,
    pub has_vite: bool,
    pub has_go_mod: bool,
    pub package_scripts: Option<PackageScripts>,
}

impl ProjectInfo {
    pub fn primary_kind(&self) -> ProjectKind {
        if self.kinds.contains(&ProjectKind::Tauri) {
            ProjectKind::Tauri
        } else if self.kinds.contains(&ProjectKind::Electron) {
            ProjectKind::Electron
        } else if self.kinds.contains(&ProjectKind::Rust) {
            ProjectKind::Rust
        } else if self.kinds.contains(&ProjectKind::React) {
            ProjectKind::React
        } else if self.kinds.contains(&ProjectKind::Vite) {
            ProjectKind::Vite
        } else if self.kinds.contains(&ProjectKind::Node) {
            ProjectKind::Node
        } else if self.kinds.contains(&ProjectKind::Python) {
            ProjectKind::Python
        } else if self.kinds.contains(&ProjectKind::Go) {
            ProjectKind::Go
        } else if self.kinds.contains(&ProjectKind::Shell) {
            ProjectKind::Shell
        } else if self.kinds.contains(&ProjectKind::CMake) {
            ProjectKind::CMake
        } else if self.kinds.contains(&ProjectKind::Java) {
            ProjectKind::Java
        } else {
            ProjectKind::Unknown
        }
    }

    pub fn display_kinds(&self) -> String {
        if self.kinds.is_empty() {
            "Unknown".to_string()
        } else {
            self.kinds
                .iter()
                .map(|k| format!("{:?}", k))
                .collect::<Vec<_>>()
                .join("/")
        }
    }
}

pub fn inspect(path: &Path) -> Option<ProjectInfo> {
    if !path.is_dir() {
        return None;
    }

    let mut markers = Vec::new();
    let mut kinds = Vec::new();

    let has_package_json = path.join("package.json").is_file();
    let has_cargo_toml = path.join("Cargo.toml").is_file();
    let has_pyproject = path.join("pyproject.toml").is_file();
    let has_requirements = path.join("requirements.txt").is_file();
    let has_cmake = path.join("CMakeLists.txt").is_file();
    let has_tauri = path.join("src-tauri").is_dir()
        || path.join("tauri.conf.json").is_file()
        || path.join("src-tauri/tauri.conf.json").is_file();
    let has_vite = path.join("vite.config.js").is_file()
        || path.join("vite.config.ts").is_file()
        || path.join("vite.config.mjs").is_file()
        || path.join("vite.config.mts").is_file();
    let has_go_mod = path.join("go.mod").is_file();
    let has_pom = path.join("pom.xml").is_file();
    let has_gradle = path.join("build.gradle").is_file()
        || path.join("build.gradle.kts").is_file();

    if has_package_json {
        markers.push("package.json".into());
        kinds.push(ProjectKind::Node);
    }
    if has_cargo_toml {
        markers.push("Cargo.toml".into());
        kinds.push(ProjectKind::Rust);
    }
    if has_pyproject {
        markers.push("pyproject.toml".into());
        kinds.push(ProjectKind::Python);
    }
    if has_requirements {
        markers.push("requirements.txt".into());
        kinds.push(ProjectKind::Python);
    }
    if has_cmake {
        markers.push("CMakeLists.txt".into());
        kinds.push(ProjectKind::CMake);
    }
    if has_tauri {
        markers.push("tauri".into());
        kinds.push(ProjectKind::Tauri);
    }
    if has_vite {
        markers.push("vite.config".into());
        kinds.push(ProjectKind::Vite);
    }
    if has_go_mod {
        markers.push("go.mod".into());
        kinds.push(ProjectKind::Go);
    }
    if has_pom || has_gradle {
        markers.push(if has_pom { "pom.xml" } else { "build.gradle" }.into());
        kinds.push(ProjectKind::Java);
    }

    let mut package_scripts = None;

    if has_package_json {
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                package_scripts = Some(PackageScripts::from_json(&pkg));

                let deps = pkg.get("dependencies").cloned().unwrap_or(Value::Null);
                let dev_deps = pkg.get("devDependencies").cloned().unwrap_or(Value::Null);
                let has_dep = |name: &str| {
                    deps.get(name).is_some()
                        || dev_deps.get(name).is_some()
                        || content.contains(&format!("\"{}\"", name))
                };
                if has_dep("react") && !kinds.contains(&ProjectKind::React) {
                    kinds.push(ProjectKind::React);
                    markers.push("react".into());
                }
                if has_dep("electron") && !kinds.contains(&ProjectKind::Electron) {
                    kinds.push(ProjectKind::Electron);
                    markers.push("electron".into());
                }
            }
        }
    }

    // Python entry files even without requirements
    let py_entries = [
        "main.py", "server.py", "app.py", "run.py", "wsgi.py", "asgi.py",
        "src/main.py", "manage.py",
    ];
    for pe in &py_entries {
        if path.join(pe).is_file() && !kinds.contains(&ProjectKind::Python) {
            markers.push((*pe).into());
            kinds.push(ProjectKind::Python);
            break;
        }
    }

    if markers.is_empty() {
        if path.join("src/main.rs").is_file() || path.join("main.rs").is_file() {
            markers.push("main.rs".into());
            kinds.push(ProjectKind::Rust);
        } else if path.join("index.js").is_file() || path.join("src/index.js").is_file() {
            markers.push("index.js".into());
            kinds.push(ProjectKind::Node);
        } else {
            return None;
        }
    }

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Some(ProjectInfo {
        path: path.to_path_buf(),
        name,
        kinds,
        markers,
        has_package_json,
        has_cargo_toml,
        has_pyproject,
        has_requirements,
        has_cmake,
        has_tauri,
        has_vite,
        has_go_mod,
        package_scripts,
    })
}

/// Recursively find project roots under `root`.
pub fn find_projects_under(root: &Path, max_depth: usize) -> Vec<ProjectInfo> {
    use walkdir::WalkDir;
    use std::collections::HashSet;

    let mut found = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    let marker_names = [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "CMakeLists.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "tauri.conf.json",
        "server.py",
        "app.py",
        "main.py",
        "run.py",
        "manage.py",
        "wsgi.py",
        "asgi.py",
        "Dockerfile",
        "docker-compose.yml",
        "compose.yml",
        "Makefile",
    ];

    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if let Some(name) = e.file_name().to_str() {
                let skip = [
                    "node_modules", "target", ".git", "dist", "build",
                    ".next", ".nuxt", "vendor", "__pycache__", ".venv",
                    "venv", ".tox", "coverage", ".cache", ".axiom",
                    ".turbo", "out", "storybook-static",
                ];
                if skip.contains(&name) {
                    return false;
                }
            }
            true
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !marker_names.iter().any(|m| *m == name) {
            continue;
        }
        if let Some(parent) = entry.path().parent() {
            let canon = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            if seen.contains(&canon) {
                continue;
            }
            if let Some(info) = inspect(parent) {
                seen.insert(canon);
                found.push(info);
            }
        }
    }

    // Always include root if it is a project
    if let Some(info) = inspect(root) {
        let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if !seen.contains(&canon) {
            found.insert(0, info);
        }
    }

    // Also discover start-like shell scripts as Shell components
    discover_shell_components(root, max_depth, &mut found, &mut seen);

    found
}

/// Find shell scripts that appear to start services.
fn discover_shell_components(
    root: &Path,
    max_depth: usize,
    found: &mut Vec<ProjectInfo>,
    seen: &mut std::collections::HashSet<PathBuf>,
) {
    use walkdir::WalkDir;

    let start_hints = [
        "start", "run", "serve", "server", "runtime", "backend", "api", "dev",
        "launch", "boot",
    ];

    for entry in WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if let Some(name) = e.file_name().to_str() {
                let skip = ["node_modules", "target", ".git", ".venv", "venv", "dist", "build"];
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
        if !(name.ends_with(".sh") || name.ends_with(".bash")) {
            continue;
        }
        // Name or content suggests it starts something
        let name_match = start_hints.iter().any(|h| name.contains(h));
        let content = fs::read_to_string(entry.path()).unwrap_or_default();
        let content_l = content.to_lowercase();
        let content_match = ["uvicorn", "gunicorn", "python", "node ", "npm ", "cargo ", "go run"]
            .iter()
            .any(|h| content_l.contains(h));

        if !name_match && !content_match {
            continue;
        }

        let parent = entry.path().parent().unwrap_or(root);
        let canon = entry.path().canonicalize().unwrap_or_else(|_| entry.path().to_path_buf());
        if seen.contains(&canon) {
            continue;
        }
        // Avoid duplicating if parent is already a full project with same role
        seen.insert(canon);

        let mut markers = vec![name.clone()];
        let mut kinds = vec![ProjectKind::Shell];
        if content_l.contains("python") || content_l.contains("uvicorn") {
            kinds.push(ProjectKind::Python);
            markers.push("python-in-script".into());
        }
        if content_l.contains("node") || content_l.contains("npm") {
            kinds.push(ProjectKind::Node);
        }

        found.push(ProjectInfo {
            path: parent.to_path_buf(),
            name: entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("script")
                .to_string(),
            kinds,
            markers,
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
        // Store script path in markers for start command
        if let Some(last) = found.last_mut() {
            last.markers.push(format!("script:{}", entry.path().display()));
        }
    }
}

pub fn has_command(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn toolchain_status(info: &ProjectInfo) -> Vec<(String, bool, String)> {
    let mut status = Vec::new();
    match info.primary_kind() {
        ProjectKind::Node
        | ProjectKind::Vite
        | ProjectKind::React
        | ProjectKind::Electron
        | ProjectKind::Tauri => {
            let node = has_command("node");
            let npm = has_command("npm");
            status.push(("node".into(), node, if node { "found".into() } else { "missing".into() }));
            status.push(("npm".into(), npm, if npm { "found".into() } else { "missing".into() }));
        }
        ProjectKind::Rust => {
            let rustc = has_command("rustc");
            let cargo = has_command("cargo");
            status.push(("rustc".into(), rustc, if rustc { "found".into() } else { "missing".into() }));
            status.push(("cargo".into(), cargo, if cargo { "found".into() } else { "missing".into() }));
        }
        ProjectKind::Python | ProjectKind::Shell => {
            let python = has_command("python3") || has_command("python");
            status.push(("python".into(), python, if python { "found".into() } else { "missing".into() }));
        }
        ProjectKind::Go => {
            let go = has_command("go");
            status.push(("go".into(), go, if go { "found".into() } else { "missing".into() }));
        }
        ProjectKind::CMake => {
            let cmake = has_command("cmake");
            status.push(("cmake".into(), cmake, if cmake { "found".into() } else { "missing".into() }));
        }
        ProjectKind::Java | ProjectKind::Unknown => {}
    }
    status
}
