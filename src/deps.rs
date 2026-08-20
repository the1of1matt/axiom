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

    // Python
    if info.kinds.contains(&ProjectKind::Python) || info.has_requirements || info.has_pyproject {
        if info.has_requirements && !python_deps_look_ok(&info.path) {
            plan.reasons.push("Python dependencies may be missing".into());
            if info.path.join(".venv/pyvenv.cfg").is_file() {
                plan.commands
                    .push(".venv/bin/pip install -r requirements.txt".into());
            } else if detect::has_command("pip3") {
                plan.commands
                    .push("pip3 install -r requirements.txt".into());
            } else if detect::has_command("pip") {
                plan.commands
                    .push("pip install -r requirements.txt".into());
            }
        }
    }

    // Rust — cargo handles deps on build/run; optional explicit fetch
    if info.has_cargo_toml {
        // cargo run will fetch; no extra prepare unless we want cargo fetch
    }

    plan
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

fn python_deps_look_ok(_dir: &Path) -> bool {
    // Conservative: if requirements exist we always offer install when not in venv
    // with site-packages — for now return false to trigger prepare when requirements present
    // Only skip if a .venv exists with site-packages
    true // "ok" means no prepare — we already handle in plan_prepare differently
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
        println!("  → {}", cmd_str);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let mut cmd = if parts[0] == "npm" || parts[0] == "npx" || parts[0] == "yarn" || parts[0] == "pnpm" {
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
