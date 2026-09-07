use std::fs;
use std::path::PathBuf;
use anyhow::Result;
use crate::util;

pub fn run(yes: bool) -> Result<()> {
    util::print_header();
    println!("Uninstalling Axiom...\n");

    let mut removed = Vec::new();
    let mut skipped = Vec::new();

    // 1. Binary locations we may have installed to
    let binary_candidates = possible_binary_paths();

    for path in &binary_candidates {
        if path.is_file() {
            if !yes {
                println!("Will remove binary: {}", path.display());
            } else {
                match fs::remove_file(path) {
                    Ok(()) => {
                        println!("✓ Removed {}", path.display());
                        removed.push(path.display().to_string());
                    }
                    Err(e) => {
                        eprintln!("✗ Could not remove {}: {e}", path.display());
                        skipped.push(path.display().to_string());
                    }
                }
            }
        }
    }

    // 2. Axiom home (~/.axiom) — only our data
    if let Ok(home) = util::axiom_home() {
        if home.exists() {
            if !yes {
                println!("Will remove Axiom data: {}", home.display());
            } else {
                match fs::remove_dir_all(&home) {
                    Ok(()) => {
                        println!("✓ Removed {}", home.display());
                        removed.push(home.display().to_string());
                    }
                    Err(e) => {
                        eprintln!("✗ Could not remove {}: {e}", home.display());
                        skipped.push(home.display().to_string());
                    }
                }
            }
        } else if !yes {
            println!("Axiom data directory not found (already clean).");
        }
    }

    if !yes {
        println!();
        println!("This will ONLY remove Axiom's own binary and ~/.axiom data.");
        println!("Your projects and other software will NOT be touched.");
        println!();
        println!("Re-run with --yes to confirm:");
        println!("  axiom uninstall --yes");
        return Ok(());
    }

    println!();
    if removed.is_empty() && skipped.is_empty() {
        println!("Nothing to remove — Axiom does not appear to be installed.");
    } else if skipped.is_empty() {
        println!("✓ Axiom has been uninstalled.");
        println!();
        println!("You may also want to remove any PATH entries that pointed to Axiom");
        println!("from your shell profile (~/.zshrc, ~/.bashrc, etc.).");
    } else {
        println!("Uninstall finished with some items left. See messages above.");
    }

    Ok(())
}

fn possible_binary_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Where the install script puts it
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".axiom").join("bin").join("axiom"));
        paths.push(home.join(".local").join("bin").join("axiom"));
    }

    // Current executable (if running from an installed location)
    if let Ok(exe) = std::env::current_exe() {
        // Only consider it if it looks like an install path, not a cargo target
        let s = exe.to_string_lossy();
        if !s.contains("/target/") && !s.contains("\\target\\") {
            paths.push(exe);
        }
    }

    // Dedup
    paths.sort();
    paths.dedup();
    paths
}
