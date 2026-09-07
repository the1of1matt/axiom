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
            // Never treat tooling / CI / test trees as project roots from loose .py markers
            if is_non_app_path(parent) || is_non_app_path(entry.path()) {
                let n = name.as_ref();
                if n.ends_with(".py") || n == "Dockerfile" || n == "Makefile" {
                    continue;
                }
            }
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
    // Cross-platform: try `command -v` via sh, then bare which
    if std::process::Command::new("sh")
        .args(["-c", &format!("command -v {} >/dev/null 2>&1", cmd)])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return true;
    }
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Path segments that must never supply application entry points.
pub fn is_non_app_path(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let bad = [
        "/.github/",
        "/action/",
        "/actions/",
        "/.git/",
        "/node_modules/",
        "/target/",
        "/__pycache__/",
        "/.venv/",
        "/venv/",
        "/tests/",
        "/test/",
        "/docs/",
        "/doc/",
        "/examples/",
        "/example/",
        "/benchmarks/",
        "/benches/",
        "/fixtures/",
        "/testdata/",
        "/.tox/",
        "/site-packages/",
    ];
    bad.iter().any(|b| s.contains(b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonEntryKind {
    Module { module: String },
    Script { rel: String },
    ConsoleScript { name: String },
    None,
}

#[derive(Debug, Clone)]
pub struct PythonEntry {
    pub kind: PythonEntryKind,
    pub confidence: u8,
    pub evidence: String,
}

/// Project-aware Python entry resolution.
/// Prefer metadata → framework conventions → root scripts → never tooling paths.
pub fn resolve_python_entry(root: &Path) -> Option<PythonEntry> {
    // 1. pyproject.toml [project.scripts] / [project.gui-scripts]
    if let Some(e) = pyproject_scripts(root) {
        return Some(e);
    }
    // 2. Django
    if root.join("manage.py").is_file() && !is_non_app_path(&root.join("manage.py")) {
        return Some(PythonEntry {
            kind: PythonEntryKind::Script {
                rel: "manage.py".into(),
            },
            confidence: 85,
            evidence: "manage.py (Django-style)".into(),
        });
    }
    // 3. Package __main__.py under src/ or package dir (not tests/action)
    if let Some(e) = find_python_main_module(root) {
        return Some(e);
    }
    // 4. Root-level application scripts only (explicit names, not under tooling dirs)
    let root_candidates = [
        "server.py",
        "app.py",
        "run.py",
        "wsgi.py",
        "asgi.py",
        "main.py",
        "src/main.py",
        "src/app.py",
        "src/server.py",
    ];
    for rel in &root_candidates {
        let p = root.join(rel);
        if p.is_file() && !is_non_app_path(&p) {
            // Reject if this looks like GH Actions wrapper content
            if looks_like_actions_wrapper(&p) {
                continue;
            }
            return Some(PythonEntry {
                kind: PythonEntryKind::Script {
                    rel: (*rel).into(),
                },
                confidence: 70,
                evidence: format!("root script {}", rel),
            });
        }
    }
    // 5. Library-only package: no application entry
    if root.join("pyproject.toml").is_file() || root.join("setup.py").is_file() {
        return Some(PythonEntry {
            kind: PythonEntryKind::None,
            confidence: 60,
            evidence: "Python package metadata present; no application entry point".into(),
        });
    }
    None
}

fn looks_like_actions_wrapper(path: &Path) -> bool {
    let Ok(s) = fs::read_to_string(path) else {
        return false;
    };
    let l = s.to_lowercase();
    l.contains("github.actions")
        || l.contains("from actions")
        || (l.contains("os.environ") && l.contains("github_") && s.lines().count() < 80)
}

fn pyproject_scripts(root: &Path) -> Option<PythonEntry> {
    let content = fs::read_to_string(root.join("pyproject.toml")).ok()?;
    // Minimal TOML parse for [project.scripts] name = "module:func"
    let mut in_scripts = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_scripts = t == "[project.scripts]" || t == "[project.gui-scripts]";
            continue;
        }
        if !in_scripts {
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // name = "pkg.module:main"
        if let Some((name, _rest)) = t.split_once('=') {
            let name = name.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(PythonEntry {
                    kind: PythonEntryKind::ConsoleScript {
                        name: name.to_string(),
                    },
                    confidence: 95,
                    evidence: format!("pyproject.toml [project.scripts] → {}", name),
                });
            }
        }
    }
    // Poetry scripts
    let mut in_poetry = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_poetry = t == "[tool.poetry.scripts]";
            continue;
        }
        if !in_poetry {
            continue;
        }
        if let Some((name, _)) = t.split_once('=') {
            let name = name.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(PythonEntry {
                    kind: PythonEntryKind::ConsoleScript {
                        name: name.to_string(),
                    },
                    confidence: 90,
                    evidence: format!("pyproject.toml [tool.poetry.scripts] → {}", name),
                });
            }
        }
    }
    None
}

fn find_python_main_module(root: &Path) -> Option<PythonEntry> {
    let candidates = [
        root.join("src"),
        root.join("lib"),
        root.to_path_buf(),
    ];
    for base in &candidates {
        if !base.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(base) {
            for e in entries.flatten() {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                if is_non_app_path(&p) {
                    continue;
                }
                let main = p.join("__main__.py");
                if main.is_file() {
                    let mod_name = p.file_name()?.to_str()?.to_string();
                    // Prefer src layout package name
                    return Some(PythonEntry {
                        kind: PythonEntryKind::Module { module: mod_name },
                        confidence: 80,
                        evidence: format!("package __main__.py at {}", main.display()),
                    });
                }
            }
        }
    }
    None
}

/// Classify execution mode from project metadata and planned start commands.
pub fn classify_target(
    info: &ProjectInfo,
    start: &[String],
) -> (crate::model::ExecutionMode, crate::model::ProjectClass) {
    use crate::model::{ExecutionMode, ProjectClass};

    // Rust
    if info.has_cargo_toml {
        return classify_rust(info);
    }

    // Node
    if info.has_package_json {
        return classify_node(info, start);
    }

    // Python
    if info.kinds.contains(&ProjectKind::Python) || info.has_pyproject || info.has_requirements {
        return classify_python(info, start);
    }

    if start.is_empty() {
        return (ExecutionMode::Ambiguous, ProjectClass::Unknown);
    }
    (ExecutionMode::OneShotCli, ProjectClass::Unknown)
}

fn classify_rust(info: &ProjectInfo) -> (crate::model::ExecutionMode, crate::model::ProjectClass) {
    use crate::model::{ExecutionMode, ProjectClass};
    let cargo = fs::read_to_string(info.path.join("Cargo.toml")).unwrap_or_default();
    let has_bin = cargo.contains("[[bin]]")
        || info.path.join("src/main.rs").is_file()
        || info.path.join("src/bin").is_dir();
    let is_workspace = cargo.contains("[workspace]");
    let is_lib_only = info.path.join("src/lib.rs").is_file() && !has_bin && !is_workspace;

    if is_lib_only {
        return (ExecutionMode::Library, ProjectClass::Library);
    }
    if is_workspace {
        // Workspace root: prefer not to `cargo run` blindly
        if has_bin || cargo.contains("default-members") {
            return (ExecutionMode::OneShotCli, ProjectClass::Application);
        }
        return (ExecutionMode::BuildOnly, ProjectClass::Application);
    }
    if has_bin {
        // CLI tools rarely open ports; treat as one-shot unless ports discovered later
        return (ExecutionMode::OneShotCli, ProjectClass::Cli);
    }
    (ExecutionMode::BuildOnly, ProjectClass::Library)
}

fn classify_node(
    info: &ProjectInfo,
    start: &[String],
) -> (crate::model::ExecutionMode, crate::model::ProjectClass) {
    use crate::model::{ExecutionMode, ProjectClass};
    let pkg = fs::read_to_string(info.path.join("package.json")).unwrap_or_default();
    let lower = pkg.to_lowercase();
    let is_lib = lower.contains("\"main\"")
        && !lower.contains("\"start\"")
        && !lower.contains("\"dev\"")
        && info.package_scripts.as_ref().map(|s| s.preferred.is_none()).unwrap_or(true);

    if info.kinds.contains(&ProjectKind::Electron) || info.kinds.contains(&ProjectKind::Tauri) {
        return (ExecutionMode::PersistentServer, ProjectClass::Application);
    }
    if lower.contains("express")
        || lower.contains("next")
        || lower.contains("fastify")
        || lower.contains("koa")
        || lower.contains("\"dev\"")
        || lower.contains("vite")
    {
        return (ExecutionMode::PersistentServer, ProjectClass::Application);
    }
    if is_lib {
        return (ExecutionMode::Library, ProjectClass::Library);
    }
    if start.iter().any(|s| s.contains("start") || s.contains("dev") || s.contains("serve")) {
        return (ExecutionMode::PersistentServer, ProjectClass::Application);
    }
    (ExecutionMode::OneShotCli, ProjectClass::Application)
}

fn classify_python(
    info: &ProjectInfo,
    start: &[String],
) -> (crate::model::ExecutionMode, crate::model::ProjectClass) {
    use crate::model::{ExecutionMode, ProjectClass};
    let entry = resolve_python_entry(&info.path);
    if let Some(ref e) = entry {
        if matches!(e.kind, PythonEntryKind::None) {
            return (ExecutionMode::Library, ProjectClass::Package);
        }
    }
    // Framework signals
    let mut blob = String::new();
    for f in &["requirements.txt", "pyproject.toml", "setup.py", "app.py", "server.py"] {
        if let Ok(s) = fs::read_to_string(info.path.join(f)) {
            blob.push_str(&s);
        }
    }
    let l = blob.to_lowercase();
    if l.contains("flask")
        || l.contains("fastapi")
        || l.contains("django")
        || l.contains("uvicorn")
        || l.contains("gunicorn")
        || l.contains("starlette")
    {
        return (ExecutionMode::PersistentServer, ProjectClass::Application);
    }
    if start.iter().any(|s| s.contains("server.py") || s.contains("app.py") || s.contains("uvicorn")) {
        return (ExecutionMode::PersistentServer, ProjectClass::Application);
    }
    if info.has_pyproject && entry.as_ref().map(|e| matches!(e.kind, PythonEntryKind::ConsoleScript { .. })).unwrap_or(false) {
        return (ExecutionMode::OneShotCli, ProjectClass::Cli);
    }
    if start.is_empty() {
        return (ExecutionMode::Library, ProjectClass::Package);
    }
    (ExecutionMode::OneShotCli, ProjectClass::Cli)
}

/// Rust start commands with workspace / bin awareness.
pub fn rust_start_commands(info: &ProjectInfo) -> Vec<String> {
    let cargo_toml = fs::read_to_string(info.path.join("Cargo.toml")).unwrap_or_default();
    let is_workspace = cargo_toml.contains("[workspace]");

    // Prefer explicit binary targets
    let bins = rust_bin_names(&cargo_toml, &info.path);
    if is_workspace {
        // At workspace root, only run if there is a clear default bin package
        if let Some(default) = workspace_default_bin(&cargo_toml) {
            return vec![format!("cargo run -p {}", default)];
        }
        // Build-only diagnostic path — empty start means orchestrate reports library/workspace
        if bins.is_empty() {
            return vec![];
        }
    }
    if bins.len() == 1 {
        return vec![format!("cargo run --bin {}", bins[0])];
    }
    if info.path.join("src/main.rs").is_file() {
        return vec!["cargo run".into()];
    }
    if !bins.is_empty() {
        // Multiple bins — do not guess; empty start triggers diagnostic
        return vec![];
    }
    // Library crate
    vec![]
}

fn rust_bin_names(cargo_toml: &str, root: &Path) -> Vec<String> {
    let mut bins = Vec::new();
    let mut lines = cargo_toml.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "[[bin]]" {
            // look ahead for name =
            for _ in 0..6 {
                if let Some(l) = lines.peek() {
                    let t = l.trim();
                    if t.starts_with("name") {
                        if let Some((_, v)) = t.split_once('=') {
                            let name = v.trim().trim_matches('"').trim_matches('\'').to_string();
                            if !name.is_empty() {
                                bins.push(name);
                            }
                        }
                        break;
                    }
                    if t.starts_with('[') {
                        break;
                    }
                }
                lines.next();
            }
        }
    }
    // src/bin/*.rs
    let bin_dir = root.join("src/bin");
    if bin_dir.is_dir() {
        if let Ok(rd) = fs::read_dir(&bin_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        if !bins.iter().any(|b| b == stem) {
                            bins.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    bins
}

fn workspace_default_bin(cargo_toml: &str) -> Option<String> {
    // default-members = ["crate"]
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with("default-members") {
            if let Some((_, rest)) = t.split_once('=') {
                // crude: first quoted string
                if let Some(start) = rest.find('"') {
                    let rest = &rest[start + 1..];
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].trim_start_matches("./").to_string());
                    }
                }
            }
        }
    }
    None
}

/// Detect required Rust channel/edition from project files.
pub fn rust_toolchain_spec(root: &Path) -> Option<String> {
    // rust-toolchain / rust-toolchain.toml
    for name in &["rust-toolchain.toml", "rust-toolchain"] {
        let p = root.join(name);
        if let Ok(s) = fs::read_to_string(&p) {
            // channel = "1.85.0" or contents "1.85.0" / "nightly"
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with("channel") {
                    if let Some((_, v)) = t.split_once('=') {
                        return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
                    }
                }
            }
            let t = s.trim();
            if !t.is_empty() && !t.contains('[') {
                return Some(t.lines().next()?.trim().to_string());
            }
        }
    }
    // edition in Cargo.toml
    if let Ok(s) = fs::read_to_string(root.join("Cargo.toml")) {
        for line in s.lines() {
            let t = line.trim();
            if t.starts_with("edition") {
                if let Some((_, v)) = t.split_once('=') {
                    let ed = v.trim().trim_matches('"').trim_matches('\'');
                    // edition 2024 needs a recent rustc; map to channel hint
                    if ed == "2024" {
                        return Some("1.85.0".into());
                    }
                }
            }
        }
    }
    None
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
