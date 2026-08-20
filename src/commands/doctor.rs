use std::env;
use std::fs;
use anyhow::Result;
use crate::util;
use crate::detect;

pub fn run(path: Option<&str>, repair: bool) -> Result<()> {
    util::print_header();
    println!("Running diagnostics...\n");

    // OS / Arch
    println!("System");
    println!("  OS:           {}", env::consts::OS);
    println!("  Family:       {}", env::consts::FAMILY);
    println!("  Arch:         {}", env::consts::ARCH);
    println!("  Pointer width: {}-bit", std::mem::size_of::<usize>() * 8);
    println!();

    // Permissions / home
    println!("Environment");
    match dirs::home_dir() {
        Some(h) => {
            println!("  Home:         {}", h.display());
            let axiom = h.join(".axiom");
            if axiom.exists() {
                println!("  Axiom home:   {} (exists)", axiom.display());
            } else {
                println!("  Axiom home:   {} (will be created on first use)", axiom.display());
                if repair {
                    util::ensure_axiom_dirs()?;
                    println!("  → created ~/.axiom structure");
                }
            }
        }
        None => println!("  Home:         (could not determine)"),
    }

    if let Ok(cwd) = env::current_dir() {
        println!("  CWD:          {}", cwd.display());
    }
    println!();

    // Disk space (best effort)
    println!("Disk");
    // Simple check via statvfs is platform specific; skip detailed for MVP
    println!("  (detailed free-space reporting planned)");
    println!();

    // Axiom itself
    println!("Axiom");
    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    if let Ok(exe) = env::current_exe() {
        println!("  Binary:       {}", exe.display());
        if let Ok(meta) = fs::metadata(&exe) {
            println!("  Size:         {}", util::format_size(meta.len()));
        }
    }
    println!();

    // Common toolchains
    println!("Toolchains on PATH");
    let tools = [
        "node", "npm", "npx", "yarn", "pnpm",
        "rustc", "cargo",
        "python3", "python", "pip", "pip3",
        "cmake", "make", "ninja", "gcc", "clang",
        "git",
    ];
    for t in &tools {
        let found = detect::has_command(t);
        let mark = if found { "✓" } else { "·" };
        println!("  {} {}", mark, t);
    }
    println!();

    // Project (if given or cwd)
    let project_path = match path {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => env::current_dir().ok(),
    };

    if let Some(p) = project_path {
        println!("Project inspection");
        println!("  Path: {}", p.display());
        match detect::inspect(&p) {
            Some(info) => {
                println!("  Name:    {}", info.name);
                println!("  Kinds:   {}", info.display_kinds());
                println!("  Markers: {}", info.markers.join(", "));
                println!();
                println!("  Toolchain status for this project:");
                for (tool, ok, msg) in detect::toolchain_status(&info) {
                    let mark = if ok { "✓" } else { "✗" };
                    println!("    {} {} — {}", mark, tool, msg);
                }
            }
            None => {
                println!("  (not recognized as a project)");
            }
        }
        println!();
    }

    // Cache health (basic)
    if let Ok(home) = util::axiom_home() {
        println!("Axiom cache");
        for sub in &["toolchains", "packages", "cache", "tmp"] {
            let p = home.join(sub);
            if p.exists() {
                // Count entries roughly
                let count = fs::read_dir(&p).map(|rd| rd.count()).unwrap_or(0);
                println!("  {}/  ({} entries)", sub, count);
            } else {
                println!("  {}/  (missing)", sub);
            }
        }
    }

    println!();
    if repair {
        println!("Repair pass completed (safe operations only).");
    } else {
        println!("Tip: run `axiom doctor --repair` to create missing Axiom directories.");
    }

    Ok(())
}
