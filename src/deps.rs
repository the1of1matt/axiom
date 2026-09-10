//! Dependency health detection and repair — generic across ecosystems.
//!
//! Safety:
//! - Never delete node_modules merely because it exists.
//! - Destructive rebuild only when health check fails AND either:
//!     (a) workspace is Axiom-owned (extracted ZIP under ~/.axiom/tmp), or
//!     (b) user confirms (existing project directories).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use crate::detect::{self, ProjectInfo, ProjectKind};
use crate::util;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepHealth {
    /// No dependency install needed / already healthy
    Healthy,
    /// Missing install entirely
    Missing,
    /// Present but broken/incomplete
    Broken(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct PreparePlan {
    pub commands: Vec<String>,
    /// If set, remove this directory before running commands (only when safe).
    pub remove_first: Option<PathBuf>,
    pub reasons: Vec<String>,
}

/// True when path is under Axiom's temp extraction workspace.
pub fn is_axiom_owned_workspace(path: &Path) -> bool {
    if let Some(tmp) = crate::platform::axiom_tmp() {
        if let (Ok(c), Ok(t)) = (path.canonicalize(), tmp.canonicalize()) {
            return c.starts_with(&t);
        }
    }
    let s = path.to_string_lossy().replace('\\', "/");
    s.contains(".axiom") && s.contains("/tmp/")
}

/// Build prepare steps for a component based on dependency health.
pub fn plan_prepare(info: &ProjectInfo, axiom_owned: bool) -> PreparePlan {
    let mut plan = PreparePlan {
        commands: Vec::new(),
        remove_first: None,
        reasons: Vec::new(),
    };

    // Node / npm projects
    if info.has_package_json
        || info.kinds.iter().any(|k| {
            matches!(
                k,
                ProjectKind::Node
                    | ProjectKind::Vite
                    | ProjectKind::React
                    | ProjectKind::Electron
                    | ProjectKind::Tauri
            )
        })
    {
        let health = check_node_health(&info.path);
        match health {
            DepHealth::Healthy => {
                // nothing
            }
            DepHealth::Missing => {
                plan.reasons.push("node_modules missing".into());
                plan.commands.push(node_install_cmd(&info.path));
            }
            DepHealth::Broken(issues) => {
                for i in &issues {
                    plan.reasons.push(i.clone());
                }
                if axiom_owned {
                    plan.reasons
                        .push("Axiom-owned workspace — rebuilding node_modules".into());
                    plan.remove_first = Some(info.path.join("node_modules"));
                    plan.commands.push(node_install_cmd(&info.path));
                } else {
                    // Destructive repair requires confirmation at execution time
                    plan.reasons.push(
                        "existing project — will ask before removing node_modules".into(),
                    );
                    plan.remove_first = Some(info.path.join("node_modules"));
                    plan.commands.push(node_install_cmd(&info.path));
                }
            }
        }
    }

    // Python — always prefer an Axiom-managed or project-local venv
    if info.kinds.contains(&ProjectKind::Python) || info.has_requirements || info.has_pyproject {
        let py_plan = plan_python_prepare(info);
        plan.reasons.extend(py_plan.reasons);
        plan.commands.extend(py_plan.commands);
    }

    // Rust toolchain — ensure required channel when detectable
    if info.has_cargo_toml {
        if let Some(spec) = detect::rust_toolchain_spec(&info.path) {
            if !rust_toolchain_available(&spec) {
                plan.reasons.push(format!(
                    "Rust toolchain '{}' required by project metadata",
                    spec
                ));
                if detect::has_command("rustup") {
                    plan.commands
                        .push(format!("rustup toolchain install {}", spec));
                    plan.commands
                        .push(format!("rustup override set {} --path .", spec));
                } else {
                    plan.reasons.push(
                        "rustup not found — cannot install required toolchain automatically".into(),
                    );
                }
            }
        }
    }

    // Go modules
    if info.has_go_mod || info.kinds.contains(&ProjectKind::Go) {
        let go_plan = plan_go_prepare(info);
        plan.reasons.extend(go_plan.reasons);
        plan.commands.extend(go_plan.commands);
    }

    // Java (Maven / Gradle)
    if info.kinds.contains(&ProjectKind::Java) {
        let j_plan = plan_java_prepare(info);
        plan.reasons.extend(j_plan.reasons);
        plan.commands.extend(j_plan.commands);
    }

    // C++ build systems — configuration is deferred to start/build commands
    if info.kinds.contains(&ProjectKind::CMake)
        || info.kinds.contains(&ProjectKind::Make)
        || info.kinds.contains(&ProjectKind::Meson)
    {
        let c_plan = plan_cpp_prepare(info);
        plan.reasons.extend(c_plan.reasons);
        plan.commands.extend(c_plan.commands);
    }

    plan
}

/// Go module dependency preparation. Prefer vendor when present; otherwise `go mod download`
/// only when the module graph is not already resolvable (keeps cache-hit runs fast).
fn plan_go_prepare(info: &ProjectInfo) -> PreparePlan {
    let mut plan = PreparePlan {
        commands: Vec::new(),
        remove_first: None,
        reasons: Vec::new(),
    };

    if !detect::has_command("go") {
        plan.reasons.push("go toolchain not found on PATH".into());
        return plan;
    }

    if let Some(req) = detect::go_required_version(&info.path) {
        if let Some(have) = detect::go_version_string() {
            // Compare major.minor loosely (go1.22.2 vs 1.22)
            let req_norm = req.trim_start_matches("go");
            let have_norm = have.trim_start_matches("go");
            if version_less(have_norm, req_norm) {
                plan.reasons.push(format!(
                    "project requires Go {} but available is {}",
                    req, have
                ));
                // Still attempt; go itself will error clearly
            }
        }
    }

    // Vendor mode: no download needed when vendor/modules.txt exists
    if info.path.join("vendor/modules.txt").is_file() {
        plan.reasons.push("vendor/ present — using vendored modules".into());
        return plan;
    }

    if !info.path.join("go.mod").is_file() {
        return plan;
    }

    // Fast path: if go.sum exists and `go list` succeeds with -mod=readonly, skip download
    if go_modules_ready(&info.path) {
        plan.reasons.push("Go modules already available".into());
        return plan;
    }

    plan.reasons.push("Go module dependencies need download".into());
    plan.commands.push("go mod download".into());
    plan
}

fn go_modules_ready(root: &Path) -> bool {
    // Lightweight check: go list with readonly mod mode
    let status = Command::new("go")
        .args(["list", "-e", "-m", "-mod=readonly"])
        .current_dir(root)
        .env("GOFLAGS", "-mod=readonly")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

/// Compare dotted version strings (1.22.0 vs 1.21). True if a < b.
fn version_less(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..av.len().max(bv.len()) {
        let x = av.get(i).copied().unwrap_or(0);
        let y = bv.get(i).copied().unwrap_or(0);
        if x < y {
            return true;
        }
        if x > y {
            return false;
        }
    }
    false
}

fn plan_java_prepare(info: &ProjectInfo) -> PreparePlan {
    let mut plan = PreparePlan {
        commands: Vec::new(),
        remove_first: None,
        reasons: Vec::new(),
    };

    let has_pom = info.path.join("pom.xml").is_file();
    let has_gradle = info.path.join("build.gradle").is_file()
        || info.path.join("build.gradle.kts").is_file();
    let mvnw = if cfg!(windows) {
        info.path.join("mvnw.cmd")
    } else {
        info.path.join("mvnw")
    };
    let gradlew = if cfg!(windows) {
        info.path.join("gradlew.bat")
    } else {
        info.path.join("gradlew")
    };

    if has_pom {
        let mvn_cmd = if mvnw.is_file() {
            // Prefer wrapper
            if cfg!(windows) {
                ".\\mvnw.cmd".to_string()
            } else {
                "./mvnw".to_string()
            }
        } else if detect::has_command("mvn") {
            "mvn".to_string()
        } else {
            plan.reasons.push(
                "Maven project detected but neither mvnw wrapper nor mvn is available".into(),
            );
            return plan;
        };
        // Dependency resolution only — avoid full package on prepare
        plan.reasons.push("Maven dependency resolution".into());
        plan.commands
            .push(format!("{} -q -DskipTests dependency:resolve", mvn_cmd));
        return plan;
    }

    if has_gradle {
        let gradle_cmd = if gradlew.is_file() {
            if cfg!(windows) {
                ".\\gradlew.bat".to_string()
            } else {
                "./gradlew".to_string()
            }
        } else if detect::has_command("gradle") {
            "gradle".to_string()
        } else {
            plan.reasons.push(
                "Gradle project detected but neither gradlew wrapper nor gradle is available"
                    .into(),
            );
            return plan;
        };
        plan.reasons.push("Gradle dependency resolution".into());
        // dependencies task is lighter than a full build
        plan.commands
            .push(format!("{} -q dependencies", gradle_cmd));
        return plan;
    }

    // Plain Java — no remote deps to fetch
    plan.reasons.push("plain Java project — no dependency manager".into());
    plan
}

fn plan_cpp_prepare(info: &ProjectInfo) -> PreparePlan {
    let mut plan = PreparePlan {
        commands: Vec::new(),
        remove_first: None,
        reasons: Vec::new(),
    };

    if info.kinds.contains(&ProjectKind::CMake) {
        if !detect::has_command("cmake") {
            plan.reasons.push("cmake not found on PATH".into());
        } else {
            plan.reasons
                .push("CMake project — configure/build deferred to run".into());
        }
    }
    if info.kinds.contains(&ProjectKind::Make) {
        if !detect::has_command("make") {
            plan.reasons.push("make not found on PATH".into());
        }
    }
    if info.kinds.contains(&ProjectKind::Meson) {
        if !detect::has_command("meson") {
            plan.reasons.push("meson not found on PATH".into());
        }
    }
    plan
}

/// Plan Python venv creation + dependency install. Never assumes global site-packages.
fn plan_python_prepare(info: &ProjectInfo) -> PreparePlan {
    let mut plan = PreparePlan {
        commands: Vec::new(),
        remove_first: None,
        reasons: Vec::new(),
    };

    let req_files = python_requirement_files(&info.path);
    let needs_deps = !req_files.is_empty()
        || info.has_pyproject
        || info.path.join("setup.py").is_file()
        || info.path.join("Pipfile").is_file();

    if !needs_deps {
        return plan;
    }

    if python_deps_look_ok(&info.path) {
        plan.reasons
            .push("Python managed environment looks healthy".into());
        return plan;
    }

    let venv = preferred_venv_dir(&info.path);
    let py = if detect::has_command("python3") {
        "python3"
    } else {
        "python"
    };

    // Create venv if missing (handled specially in execute_prepare for ensurepip fallbacks)
    if !venv.join("pyvenv.cfg").is_file() {
        plan.reasons
            .push(format!("creating managed venv at {}", venv.display()));
        plan.commands
            .push(format!("__axiom_venv__{} {}", py, venv.display()));
    } else {
        plan.reasons
            .push("existing venv incomplete — installing declared dependencies".into());
    }

    let pip = venv_pip(&venv);
    for req in &req_files {
        let rel = req
            .strip_prefix(&info.path)
            .unwrap_or(req)
            .display()
            .to_string();
        plan.commands
            .push(format!("{} install -r {}", pip, rel));
    }
    if req_files.is_empty() {
        if info.has_pyproject || info.path.join("setup.py").is_file() {
            plan.commands.push(format!("{} install -e .", pip));
        } else if info.path.join("Pipfile").is_file() && detect::has_command("pipenv") {
            plan.commands.push("pipenv install".into());
        }
    }

    plan
}

/// Requirement files Axiom understands.
pub fn python_requirement_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let candidates = [
        "requirements.txt",
        "requirements/base.txt",
        "requirements/prod.txt",
        "requirements/production.txt",
        "requirements/dev.txt",
        "requirements-dev.txt",
    ];
    for c in &candidates {
        let p = dir.join(c);
        if p.is_file() {
            out.push(p);
        }
    }
    // requirements/*.txt (any)
    let req_dir = dir.join("requirements");
    if req_dir.is_dir() {
        if let Ok(rd) = fs::read_dir(&req_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("txt") {
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

fn preferred_venv_dir(project: &Path) -> PathBuf {
    // Prefer existing project .venv
    let local = project.join(".venv");
    if local.join("pyvenv.cfg").is_file() {
        return local;
    }
    // Axiom-managed env keyed by project path fingerprint
    if let Some(base) = crate::platform::axiom_cache() {
        let key = simple_path_fp(project);
        let managed = base.join("python").join(key).join("venv");
        return managed;
    }
    local
}

fn simple_path_fp(path: &Path) -> String {
    let s = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

fn venv_pip(venv: &Path) -> String {
    let unix = venv.join("bin/pip");
    let win = venv.join("Scripts/pip.exe");
    if cfg!(windows) {
        win.display().to_string()
    } else if unix.is_file() {
        unix.display().to_string()
    } else {
        // Will exist after venv creation
        unix.display().to_string()
    }
}

fn venv_python(venv: &Path) -> String {
    let unix = venv.join("bin/python");
    let win = venv.join("Scripts/python.exe");
    if cfg!(windows) {
        win.display().to_string()
    } else {
        unix.display().to_string()
    }
}

/// Interpreter Axiom should use for this project (managed venv preferred).
pub fn python_interpreter(project: &Path) -> String {
    let venv = preferred_venv_dir(project);
    let py = venv_python(&venv);
    if Path::new(&py).is_file() || venv.join("pyvenv.cfg").is_file() {
        return py;
    }
    if project.join(".venv/bin/python").is_file() {
        return project
            .join(".venv/bin/python")
            .display()
            .to_string();
    }
    if detect::has_command("python3") {
        "python3".into()
    } else {
        "python".into()
    }
}

fn rust_toolchain_available(spec: &str) -> bool {
    // If rustup is present, ask it; else compare rustc version heuristically
    if detect::has_command("rustup") {
        let out = Command::new("rustup")
            .args(["run", spec, "rustc", "--version"])
            .output();
        if let Ok(o) = out {
            return o.status.success();
        }
    }
    // edition2024 / 1.85 style: check rustc version string
    if let Ok(o) = Command::new("rustc").arg("--version").output() {
        let v = String::from_utf8_lossy(&o.stdout);
        // extract major.minor
        if let Some(ver) = v.split_whitespace().nth(1) {
            if let Some((need_maj, need_min)) = parse_maj_min(spec) {
                if let Some((have_maj, have_min)) = parse_maj_min(ver) {
                    return have_maj > need_maj
                        || (have_maj == need_maj && have_min >= need_min);
                }
            }
        }
    }
    false
}

fn parse_maj_min(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.trim().trim_start_matches('v').split('.');
    let maj = parts.next()?.parse().ok()?;
    let min = parts.next()?.parse().ok()?;
    Some((maj, min))
}

fn node_install_cmd(dir: &Path) -> String {
    // Flags: keep lockfile reproducibility; use local npm cache; skip non-essential work.
    // --prefer-offline: use cache when possible without ignoring the registry when needed
    // --no-audit / --no-fund: skip network reports that don't affect install correctness
    if dir.join("package-lock.json").is_file() || dir.join("npm-shrinkwrap.json").is_file() {
        "npm ci --prefer-offline --no-audit --no-fund".into()
    } else if dir.join("yarn.lock").is_file() && detect::has_command("yarn") {
        "yarn install --frozen-lockfile".into()
    } else if dir.join("pnpm-lock.yaml").is_file() && detect::has_command("pnpm") {
        "pnpm install --frozen-lockfile".into()
    } else {
        "npm install --prefer-offline --no-audit --no-fund".into()
    }
}

/// Point npm at a persistent Axiom-managed cache so ZIP temp installs reuse downloads.
fn configure_npm_env(cmd: &mut Command) {
    if let Some(base) = crate::platform::axiom_cache() {
        let cache = base.join("npm");
        let _ = fs::create_dir_all(&cache);
        cmd.env("npm_config_cache", &cache);
        cmd.env("npm_config_prefer_offline", "true");
        cmd.env("npm_config_audit", "false");
        cmd.env("npm_config_fund", "false");
        cmd.env("npm_config_update_notifier", "false");
    }
}

/// Inspect node_modules for missing packages and broken .bin links.
pub fn check_node_health(dir: &Path) -> DepHealth {
    let pkg_path = dir.join("package.json");
    if !pkg_path.is_file() {
        return DepHealth::Healthy;
    }
    let nm = dir.join("node_modules");
    if !nm.is_dir() {
        return DepHealth::Missing;
    }

    let content = match fs::read_to_string(&pkg_path) {
        Ok(c) => c,
        Err(_) => return DepHealth::Healthy,
    };
    let pkg: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return DepHealth::Healthy,
    };

    let mut issues = Vec::new();

    // Collect declared dependency names
    let mut deps: Vec<String> = Vec::new();
    for key in &["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(obj) = pkg.get(*key).and_then(|v| v.as_object()) {
            for name in obj.keys() {
                deps.push(name.clone());
            }
        }
    }

    // Sample declared packages for existence
    for name in deps.iter().take(50) {
        let path = nm.join(name);
        if !path.is_dir() {
            issues.push(format!("missing package: {}", name));
        }
    }

    // Inspect every entry in node_modules/.bin
    let bin_dir = nm.join(".bin");
    if bin_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() || name.ends_with(".cmd") || name.ends_with(".ps1") {
                    continue;
                }

                // Broken symlink
                let meta = p.symlink_metadata().ok();
                let is_link = meta
                    .as_ref()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                if is_link {
                    if fs::metadata(&p).is_err() {
                        issues.push(format!("broken bin link: {}", name));
                        continue;
                    }
                }

                // Resolve relative targets inside shim scripts
                if let Ok(text) = fs::read_to_string(&p) {
                    // Common patterns: ../electron/cli.js  or  basedir/../package/...
                    for line in text.lines().take(30) {
                        let line = line.trim();
                        // skip shebang / empty
                        if line.starts_with('#') || line.is_empty() {
                            continue;
                        }
                        // extract path-like tokens containing "../" and a package path
                        for token in line.split_whitespace() {
                            let tok = token.trim_matches(|c: char| {
                                c == '"' || c == '\'' || c == '`' || c == ';' || c == ')' || c == '('
                            });
                            if tok.contains("node_modules") || tok.starts_with("../") || tok.starts_with("./")
                            {
                                // resolve relative to .bin
                                let candidate = if tok.starts_with('/') {
                                    PathBuf::from(tok)
                                } else {
                                    bin_dir.join(tok)
                                };
                                // Only flag if it looks like a JS entry or binary path
                                if (tok.ends_with(".js")
                                    || tok.ends_with(".cjs")
                                    || tok.ends_with(".mjs")
                                    || tok.contains("/cli")
                                    || tok.contains("/bin/"))
                                    && !candidate.exists()
                                {
                                    // normalize ..
                                    let canon = normalize_rel(&bin_dir, tok);
                                    if !canon.exists() {
                                        issues.push(format!(
                                            "bin '{}' references missing file: {}",
                                            name, tok
                                        ));
                                    }
                                }
                            }
                        }
                        // electron specific: require('electron') paths
                        if line.contains("cli.js") && line.contains("electron") {
                            let cli = nm.join("electron").join("cli.js");
                            if !cli.is_file() {
                                issues.push(
                                    "electron bin shim present but electron/cli.js missing".into(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Declared packages that are commonly CLI-driven: verify package integrity
    let critical = [
        "electron",
        "vite",
        "next",
        "webpack",
        "typescript",
        "react-scripts",
        "esbuild",
        "parcel",
        "nuxt",
        "svelte-kit",
        "@electron/rebuild",
    ];
    for crit in &critical {
        let declared = deps.iter().any(|d| d == crit || d.ends_with(&format!("/{}", crit)));
        if !declared {
            // also check scripts text
            let scripts = pkg
                .get("scripts")
                .and_then(|s| s.as_object())
                .map(|o| {
                    o.values()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            if !scripts.contains(crit) {
                continue;
            }
        }

        let pkg_dir = nm.join(crit);
        if !pkg_dir.is_dir() {
            issues.push(format!("required package '{}' is not installed", crit));
            continue;
        }

        // electron package integrity
        if *crit == "electron" {
            let cli = pkg_dir.join("cli.js");
            let index = pkg_dir.join("index.js");
            let dist = pkg_dir.join("dist");
            let path_txt = pkg_dir.join("path.txt");
            if !cli.is_file() && !index.is_file() {
                issues.push("electron package incomplete (missing cli.js/index.js)".into());
            }
            // path.txt points at the downloaded binary — absence often means incomplete postinstall
            if !path_txt.is_file() && !dist.is_dir() {
                // Not always present on all versions; only warn if cli also weak
                if !cli.is_file() {
                    issues.push("electron binary not installed (path.txt/dist missing)".into());
                }
            }
        }

        // .bin shim for this tool
        
        // electron bin name is "electron"
        let bin_name = if crit.contains('/') {
            crit.split('/').last().unwrap_or(crit)
        } else {
            crit
        };
        let bin_shim = bin_dir.join(bin_name);
        if bin_shim.exists() {
            if bin_shim
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && fs::metadata(&bin_shim).is_err()
            {
                issues.push(format!("broken '{}' executable in node_modules/.bin", bin_name));
            }
        }
    }

    // package-lock present but node_modules looks truncated
    if dir.join("package-lock.json").is_file()
        || dir.join("npm-shrinkwrap.json").is_file()
        || dir.join("yarn.lock").is_file()
        || dir.join("pnpm-lock.yaml").is_file()
    {
        let count = count_top_level_packages(&nm);
        if count < 3 && deps.len() > 3 {
            issues.push(format!(
                "node_modules looks truncated ({} top-level packages, {} declared)",
                count,
                deps.len()
            ));
        }
    }

    // Platform marker: if electron was installed for another OS, path.txt may point nowhere
    let electron_path = nm.join("electron").join("path.txt");
    if electron_path.is_file() {
        if let Ok(target) = fs::read_to_string(&electron_path) {
            let target = target.trim();
            if !target.is_empty() {
                let resolved = nm.join("electron").join(target);
                if !resolved.exists() && !Path::new(target).exists() {
                    issues.push(
                        "electron binary path.txt points to missing binary (possible platform mismatch)"
                            .into(),
                    );
                }
            }
        }
    }

    if issues.is_empty() {
        DepHealth::Healthy
    } else {
        issues.sort();
        issues.dedup();
        DepHealth::Broken(issues)
    }
}

fn normalize_rel(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out
}

fn count_top_level_packages(nm: &Path) -> usize {
    let Ok(entries) = fs::read_dir(nm) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            e.path().is_dir() && !s.starts_with('.')
        })
        .count()
}

/// Create a Python virtualenv with fallbacks when `ensurepip` is missing.
fn ensure_python_venv(spec: &str, cwd: &Path) -> anyhow::Result<()> {
    // spec: "<python> <venv_path>"
    let parts: Vec<&str> = spec.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("invalid venv spec: {}", spec);
    }
    let py = parts[0];
    let venv_path = parts[1..].join(" ");
    let venv = PathBuf::from(&venv_path);
    if venv.join("pyvenv.cfg").is_file() {
        return Ok(());
    }
    if let Some(parent) = venv.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // 1. python -m venv
    let st = Command::new(py)
        .args(["-m", "venv", &venv_path])
        .current_dir(cwd)
        .status()?;
    if st.success() && venv.join("pyvenv.cfg").is_file() {
        println!("  ✓ venv ready at {}", venv.display());
        return Ok(());
    }
    println!("  ⚠ python -m venv failed (often missing ensurepip); trying fallbacks...");

    // 2. virtualenv module / CLI
    if detect::has_command("virtualenv") {
        let st = Command::new("virtualenv")
            .arg(&venv_path)
            .current_dir(cwd)
            .status()?;
        if st.success() && venv.join("pyvenv.cfg").is_file() {
            println!("  ✓ venv ready via virtualenv");
            return Ok(());
        }
    }
    let st = Command::new(py)
        .args(["-m", "virtualenv", &venv_path])
        .current_dir(cwd)
        .status();
    if let Ok(st) = st {
        if st.success() && venv.join("pyvenv.cfg").is_file() {
            println!("  ✓ venv ready via python -m virtualenv");
            return Ok(());
        }
    }

    // 3. Last resort: venv without pip, then bootstrap pip via get-pip if possible
    let st = Command::new(py)
        .args(["-m", "venv", "--without-pip", &venv_path])
        .current_dir(cwd)
        .status()?;
    if st.success() && venv.join("pyvenv.cfg").is_file() {
        println!("  · venv created without pip — bootstrapping pip...");
        let py_bin = venv_python(&venv);
        // try ensurepip inside venv
        let _ = Command::new(&py_bin)
            .args(["-m", "ensurepip", "--upgrade"])
            .status();
        if Path::new(&venv_pip(&venv)).is_file()
            || Command::new(&py_bin)
                .args(["-m", "pip", "--version"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        {
            println!("  ✓ venv ready (pip bootstrapped)");
            return Ok(());
        }
        anyhow::bail!(
            "created venv at {} but pip is unavailable.\n  \
             Install python3-venv / ensurepip on this system, then re-run axiom.",
            venv.display()
        );
    }

    anyhow::bail!(
        "could not create Python virtualenv at {}.\n  \
         Install the platform python3-venv package (or virtualenv), then re-run axiom.\n  \
         Axiom will not install into the global site-packages.",
        venv.display()
    );
}

/// True only when an Axiom-relevant venv exists AND declared requirements appear installed.
/// Never returns true merely because imports work on the host global interpreter.
pub fn python_deps_look_ok(dir: &Path) -> bool {
    let venv = preferred_venv_dir(dir);
    if !venv.join("pyvenv.cfg").is_file() {
        // No managed/project venv → not prepared by Axiom standards
        return false;
    }
    let site = find_site_packages(&venv);
    let Some(site) = site else {
        return false;
    };
    // If no requirement files, presence of venv is enough
    let reqs = python_requirement_files(dir);
    if reqs.is_empty() {
        if dir.join("pyproject.toml").is_file() || dir.join("setup.py").is_file() {
            // Expect the project package itself under site-packages or an .egg-link / dist-info
            return site_has_any_dist(&site);
        }
        return true;
    }
    // Sample package names from first requirements file
    let Ok(content) = fs::read_to_string(&reqs[0]) else {
        return false;
    };
    let mut checked = 0;
    let mut found = 0;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let name = line
            .split(|c: char| c == '=' || c == '>' || c == '<' || c == '~' || c == '[' || c == '!')
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase()
            .replace('_', "-");
        if name.is_empty() {
            continue;
        }
        checked += 1;
        // dist-info or package dir
        let pkg_dir = name.replace('-', "_");
        if dir_contains_dist(&site, &name) || site.join(&pkg_dir).is_dir() {
            found += 1;
        }
        if checked >= 8 {
            break;
        }
    }
    if checked == 0 {
        return true;
    }
    found * 2 >= checked // at least half of sampled deps present
}

fn find_site_packages(venv: &Path) -> Option<PathBuf> {
    let lib = venv.join("lib");
    if lib.is_dir() {
        if let Ok(rd) = fs::read_dir(&lib) {
            for e in rd.flatten() {
                let sp = e.path().join("site-packages");
                if sp.is_dir() {
                    return Some(sp);
                }
            }
        }
    }
    let win = venv.join("Lib").join("site-packages");
    if win.is_dir() {
        return Some(win);
    }
    None
}

fn dir_contains_dist(site: &Path, name: &str) -> bool {
    let Ok(rd) = fs::read_dir(site) else {
        return false;
    };
    let prefix = format!("{}-", name.to_lowercase());
    let prefix2 = format!("{name}-");
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if n.starts_with(&prefix) || n.starts_with(&prefix2.to_lowercase()) {
            if n.contains("dist-info") || n.contains("egg-info") {
                return true;
            }
        }
    }
    false
}

fn site_has_any_dist(site: &Path) -> bool {
    let Ok(rd) = fs::read_dir(site) else {
        return false;
    };
    rd.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        n.contains("dist-info") || n.contains("egg-info") || n.contains("egg-link")
    })
}

/// Execute prepare plan. Returns Ok after verification.
pub fn execute_prepare(
    cwd: &Path,
    plan: &PreparePlan,
    axiom_owned: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    if plan.commands.is_empty() {
        println!("  ✓ Dependencies ready (reusing existing installation)");
        return Ok(());
    }

    if !plan.reasons.is_empty() {
        println!("  Dependency status:");
        for r in &plan.reasons {
            println!("    ⚠ {}", r);
        }
    }

    // Fingerprint from *current* project files (before install may create lockfile)
    let fp = node_fingerprint(cwd);

    // Try cache restore before network install
    if needs_node_install(plan) {
        println!("  → Checking dependency cache...");
        if let Some(ref fp) = fp {
            if try_restore_node_cache(cwd, fp) {
                match check_node_health(cwd) {
                    DepHealth::Healthy => {
                        println!("  ✓ Dependencies ready");
                        return Ok(());
                    }
                    DepHealth::Broken(issues) => {
                        // Soft issues after restore: still usable if packages present
                        let n = count_top_level_packages(&cwd.join("node_modules"));
                        if n >= 3 {
                            println!("  ✓ Dependencies ready (restored; soft warnings ignored)");
                            for i in issues.iter().take(2) {
                                println!("      · {}", i);
                            }
                            return Ok(());
                        }
                        println!("  ⚠ Cache restore incomplete:");
                        for i in issues.iter().take(3) {
                            println!("      • {}", i);
                        }
                        println!("  → Falling back to package manager install");
                        let _ = fs::remove_dir_all(cwd.join("node_modules"));
                    }
                    DepHealth::Missing => {
                        println!("  → Cache miss content; installing");
                    }
                }
            } else {
                println!("  · No matching cache entry");
            }
        }
    }

    if let Some(ref remove) = plan.remove_first {
        if remove.exists() {
            if axiom_owned {
                println!("  → Removing broken dependency tree (Axiom-owned workspace)...");
                let _ = fs::remove_dir_all(remove);
            } else {
                let ok = util::prompt_yes_no(
                    &format!("Remove and rebuild {}?", remove.display()),
                    true,
                );
                if ok {
                    println!("  → Removing broken dependency tree...");
                    let _ = fs::remove_dir_all(remove);
                } else {
                    anyhow::bail!("dependency repair declined by user");
                }
            }
        }
    }

    for cmd_str in &plan.commands {
        if let Some(rest) = cmd_str.strip_prefix("__axiom_venv__") {
            println!("  → python -m venv ({})", rest);
            ensure_python_venv(rest, cwd)?;
            continue;
        }
        // Strip display-only prefixes
        let cmd_str = cmd_str.as_str();
        println!("  → {}", cmd_str);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let mut cmd = if parts[0] == "npm"
            || parts[0] == "npx"
            || parts[0] == "yarn"
            || parts[0] == "pnpm"
        {
            crate::platform::command(parts[0])
        } else {
            Command::new(parts[0])
        };
        for a in &parts[1..] {
            cmd.arg(a);
        }
        cmd.current_dir(cwd);
        if parts[0] == "npm" {
            configure_npm_env(&mut cmd);
        }
        let status = cmd.status()?;
        if !status.success() {
            if cmd_str.starts_with("npm ci") {
                println!("  ⚠ npm ci failed — falling back to npm install");
                let mut cmd2 = crate::platform::command("npm");
                cmd2.args([
                    "install",
                    "--prefer-offline",
                    "--no-audit",
                    "--no-fund",
                ]);
                cmd2.current_dir(cwd);
                configure_npm_env(&mut cmd2);
                let status2 = cmd2.status()?;
                if !status2.success() {
                    anyhow::bail!("npm install failed with {}", status2);
                }
            } else if cmd_str.contains("pip") && cmd_str.contains("install") {
                // Retry with --break-system-packages is intentionally NOT done (isolation goal).
                anyhow::bail!(
                    "{} failed with {}\n  Hint: ensure the managed venv was created successfully.",
                    cmd_str,
                    status
                );
            } else {
                anyhow::bail!("{} failed with {}", cmd_str, status);
            }
        }
    }

    // Post-verify + cache store
    if cwd.join("package.json").is_file() {
        match check_node_health(cwd) {
            DepHealth::Healthy => {
                println!("  ✓ Dependencies ready");
                if let Some(ref fp) = fp {
                    println!("  → Updating dependency cache...");
                    if let Err(e) = store_node_cache(cwd, fp) {
                        if verbose {
                            println!("  · cache store skipped: {}", e);
                        }
                    }
                }
            }
            DepHealth::Missing => {
                anyhow::bail!("dependencies still missing after prepare");
            }
            DepHealth::Broken(issues) => {
                println!("  ⚠ Dependency check still reports issues:");
                for i in issues.iter().take(5) {
                    println!("      • {}", i);
                }
                // Still cache if the tree is substantially present (install succeeded).
                // Soft warnings (optional bins) should not block reuse.
                let n = count_top_level_packages(&cwd.join("node_modules"));
                if n >= 3 {
                    println!("  · Continuing — caching tree for reuse ({} packages)", n);
                    if let Some(ref fp) = fp {
                        let _ = store_node_cache(cwd, fp);
                    }
                } else {
                    println!("  · Continuing — runtime may still work (not caching incomplete tree)");
                }
            }
        }
    } else {
        println!("  ✓ Prepare complete");
    }

    Ok(())
}

fn needs_node_install(plan: &PreparePlan) -> bool {
    plan.commands.iter().any(|c| {
        c.starts_with("npm ") || c.starts_with("yarn ") || c.starts_with("pnpm ")
    })
}

/// Fingerprint: lockfile hash + OS + arch + node major.
pub fn node_fingerprint(dir: &Path) -> Option<String> {
    let lock_content = if dir.join("package-lock.json").is_file() {
        fs::read(dir.join("package-lock.json")).ok()?
    } else if dir.join("npm-shrinkwrap.json").is_file() {
        fs::read(dir.join("npm-shrinkwrap.json")).ok()?
    } else if dir.join("yarn.lock").is_file() {
        fs::read(dir.join("yarn.lock")).ok()?
    } else if dir.join("pnpm-lock.yaml").is_file() {
        fs::read(dir.join("pnpm-lock.yaml")).ok()?
    } else if dir.join("package.json").is_file() {
        fs::read(dir.join("package.json")).ok()?
    } else {
        return None;
    };

    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for b in &lock_content {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    for b in std::env::consts::OS.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    for b in std::env::consts::ARCH.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    if let Ok(out) = Command::new("node").arg("-p").arg("process.versions.node").output() {
        let v = String::from_utf8_lossy(&out.stdout);
        let major = v.split('.').next().unwrap_or("0");
        for b in major.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    Some(format!("{:016x}", h))
}

fn cache_dir(fp: &str) -> Option<PathBuf> {
    let base = crate::platform::axiom_cache()?;
    // Namespace by OS-ARCH so platforms can never share trees
    let ns = format!("{}-{}", crate::platform::os_token(), crate::platform::arch_token());
    Some(base.join("node").join(ns).join(fp))
}

fn try_restore_node_cache(project: &Path, fp: &str) -> bool {
    let Some(cache) = cache_dir(fp) else {
        return false;
    };
    let marker = cache.join(".axiom-ok");
    let cached_nm = cache.join("node_modules");
    if !marker.is_file() || !cached_nm.is_dir() {
        return false;
    }
    // Verify marker claims same OS/arch
    if let Ok(meta) = fs::read_to_string(&marker) {
        if !meta.contains(crate::platform::os_token()) || !meta.contains(crate::platform::arch_token()) {
            return false;
        }
    }
    let dest = project.join("node_modules");
    println!("  ✓ Cache hit — restoring node_modules");
    match crate::platform::copy_dir_recursive(&cached_nm, &dest) {
        Ok(()) => {
            // Count top-level packages for UX
            let n = count_top_level_packages(&dest);
            if n > 0 {
                println!("  ✓ Restored {} top-level packages", n);
            }
            println!("  → Skipping package manager install");
            true
        }
        Err(_) => false,
    }
}

fn store_node_cache(project: &Path, fp: &str) -> anyhow::Result<()> {
    let nm = project.join("node_modules");
    if !nm.is_dir() {
        return Ok(());
    }
    let Some(cache) = cache_dir(fp) else {
        return Ok(());
    };
    // Write into staging dir, then rename into place (atomic-ish)
    let staging = cache.with_extension("staging");
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    fs::create_dir_all(&staging)?;
    let staged_nm = staging.join("node_modules");
    crate::platform::copy_dir_recursive(&nm, &staged_nm)?;
    let marker_body = format!(
        "ok
os={}
arch={}
",
        crate::platform::os_token(),
        crate::platform::arch_token()
    );
    fs::write(staging.join(".axiom-ok"), marker_body)?;
    // Replace final cache dir
    if cache.exists() {
        let _ = fs::remove_dir_all(&cache);
    }
    fs::rename(&staging, &cache)?;
    println!("  ✓ Saved dependency cache");
    Ok(())
}
