use std::path::{Path, PathBuf};
use std::collections::HashSet;
use walkdir::{WalkDir, DirEntry};
use crate::detect::{self, ProjectInfo};
use crate::util;
use anyhow::Result;

/// Common directories to search under the user's home
fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // Prefer common project locations first
        for sub in &[
            "Projects", "projects", "Developer", "dev", "src", "code",
            "workspace", "Work", "work", "Documents", "Downloads",
            "Desktop", "repos", "git",
        ] {
            let p = home.join(sub);
            if p.is_dir() {
                roots.push(p);
            }
        }
        // Also search home itself (shallow)
        roots.push(home);
    }
    // Current directory and parents (limited)
    if let Ok(cwd) = std::env::current_dir() {
        roots.insert(0, cwd);
    }
    roots
}

fn is_project_marker(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    matches!(
        name.as_ref(),
        "package.json"
            | "Cargo.toml"
            | "pyproject.toml"
            | "requirements.txt"
            | "CMakeLists.txt"
            | "tauri.conf.json"
            | "vite.config.js"
            | "vite.config.ts"
            | "vite.config.mjs"
            | "main.rs"
            | "index.js"
            | "main.py"
    )
}

/// Safely find projects matching a name.
/// Does not execute any project code.
pub fn find_projects(name: &str, limit: usize) -> Result<Vec<ProjectInfo>> {
    let name_lower = name.to_lowercase();
    let mut found = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let roots = search_roots();

    eprintln!("Scanning common locations for \"{}\"...", name);

    for root in roots {
        if found.len() >= limit {
            break;
        }
        if !util::is_safe_to_scan(&root) {
            continue;
        }

        // Limit depth for home directory itself
        let max_depth = if root == dirs::home_dir().unwrap_or_default() {
            3
        } else {
            6
        };

        let walker = WalkDir::new(&root)
            .max_depth(max_depth)
            .follow_links(false) // prevent symlink loops
            .into_iter()
            .filter_entry(|e| {
                // Skip unsafe or irrelevant directories early
                if e.depth() > 0 {
                    let p = e.path();
                    if !util::is_safe_to_scan(p) {
                        return false;
                    }
                    // Skip node_modules, target, .git, etc.
                    if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                        let skip = [
                            "node_modules", "target", ".git", "dist", "build",
                            ".next", ".nuxt", "vendor", "__pycache__", ".venv",
                            "venv", ".tox", "coverage", ".cache",
                        ];
                        if skip.contains(&fname) {
                            return false;
                        }
                    }
                }
                true
            });

        for entry in walker {
            if found.len() >= limit {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // permission errors etc. — skip silently
            };

            if !entry.file_type().is_dir() {
                // Check if this is a marker file; parent is candidate
                if is_project_marker(&entry) {
                    if let Some(parent) = entry.path().parent() {
                        try_add_project(parent, &name_lower, &mut found, &mut seen_paths, limit);
                    }
                }
                continue;
            }

            // Directory named like the project
            let dir_name = entry.file_name().to_string_lossy().to_lowercase();
            if dir_name.contains(&name_lower) {
                try_add_project(entry.path(), &name_lower, &mut found, &mut seen_paths, limit);
            }
        }
    }

    // Also check if the name itself is a path
    let as_path = Path::new(name);
    if as_path.exists() {
        try_add_project(as_path, &name_lower, &mut found, &mut seen_paths, limit);
    }

    Ok(found)
}

fn try_add_project(
    path: &Path,
    name_lower: &str,
    found: &mut Vec<ProjectInfo>,
    seen: &mut HashSet<PathBuf>,
    limit: usize,
) {
    if found.len() >= limit {
        return;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if seen.contains(&canonical) {
        return;
    }
    if let Some(info) = detect::inspect(path) {
        // Prefer name match, but accept any project in matching directory
        let matches_name = info.name.to_lowercase().contains(name_lower)
            || path.to_string_lossy().to_lowercase().contains(name_lower);
        if matches_name {
            seen.insert(canonical);
            found.push(info);
        }
    }
}

/// Find a single best match or return candidates
pub fn resolve_project(target: Option<&str>) -> Result<ProjectInfo> {
    match target {
        None | Some(".") | Some("./") => {
            let cwd = std::env::current_dir()?;
            detect::inspect(&cwd)
                .ok_or_else(|| anyhow::anyhow!("current directory does not look like a project"))
        }
        Some(name) => {
            let path = Path::new(name);
            if path.exists() {
                return detect::inspect(path)
                    .ok_or_else(|| anyhow::anyhow!("path exists but is not a recognized project: {}", path.display()));
            }
            // Search by name
            let results = find_projects(name, 5)?;
            match results.len() {
                0 => Err(anyhow::anyhow!("no project found matching \"{}\"", name)),
                1 => Ok(results.into_iter().next().unwrap()),
                _ => {
                    eprintln!("Multiple projects matched \"{}\":", name);
                    for (i, p) in results.iter().enumerate() {
                        eprintln!("  [{}] {} ({})", i + 1, p.path.display(), p.display_kinds());
                    }
                    // Pick the first for now; future: interactive
                    Ok(results.into_iter().next().unwrap())
                }
            }
        }
    }
}
