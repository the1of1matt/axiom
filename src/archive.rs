//! Safe ZIP extraction into Axiom-managed temporary workspaces.

use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};
use anyhow::{bail, Context, Result};
use zip::ZipArchive;
use crate::util;

/// Extract a ZIP archive into a unique directory under ~/.axiom/tmp/
/// Returns the path to the extraction root.
/// Protects against path traversal (Zip Slip).
pub fn extract_zip(zip_path: &Path) -> Result<PathBuf> {
    if !zip_path.is_file() {
        bail!("not a file: {}", zip_path.display());
    }

    let home = util::axiom_home()?;
    let tmp_root = home.join("tmp");
    fs::create_dir_all(&tmp_root)
        .with_context(|| format!("failed to create {}", tmp_root.display()))?;

    // Unique workspace name from timestamp + stem
    let stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archive");
    // Sanitize stem
    let stem: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = tmp_root.join(format!("{}_{}", stem, ts));

    if dest.exists() {
        fs::remove_dir_all(&dest).ok();
    }
    fs::create_dir_all(&dest)?;

    let file = File::open(zip_path)
        .with_context(|| format!("cannot open {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("invalid or corrupt ZIP: {}", zip_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .with_context(|| format!("ZIP entry {} unreadable", i))?;

        let raw_name = entry.name().to_string();

        // --- Zip Slip protection ---
        let out_path = match safe_join(&dest, &raw_name) {
            Some(p) => p,
            None => {
                // Skip dangerous entries rather than aborting the whole extract
                eprintln!("  ! skipped unsafe path in archive: {}", raw_name);
                continue;
            }
        };

        if entry.is_dir() || raw_name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }

        // Ensure parent dirs exist
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut outfile = File::create(&out_path)
            .with_context(|| format!("cannot create {}", out_path.display()))?;
        io::copy(&mut entry, &mut outfile)?;

        // Best-effort preserve unix permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
            }
        }
    }

    Ok(dest)
}

/// Join `base` with `name` only if the result stays under `base`.
/// Returns None on path traversal attempts.
fn safe_join(base: &Path, name: &str) -> Option<PathBuf> {
    // Reject absolute paths and Windows drive paths
    let name_path = Path::new(name);
    if name_path.is_absolute() {
        return None;
    }

    let mut out = base.to_path_buf();
    for comp in name_path.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            // ParentDir or Prefix/RootDir → reject
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return None;
            }
        }
    }

    // Final check: canonical or at least starts-with
    // (dest may not fully exist yet, so string prefix is the practical check)
    let base_str = base.to_string_lossy();
    let out_str = out.to_string_lossy();
    if !out_str.starts_with(base_str.as_ref()) {
        return None;
    }
    Some(out)
}

/// Remove an Axiom-managed temp extraction directory.
pub fn cleanup_temp(path: &Path) -> Result<()> {
    if let Ok(home) = util::axiom_home() {
        let tmp = home.join("tmp");
        // Only delete if it's under ~/.axiom/tmp
        if path.starts_with(&tmp) && path.exists() {
            fs::remove_dir_all(path)
                .with_context(|| format!("failed to clean {}", path.display()))?;
        }
    }
    Ok(())
}

pub fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}
