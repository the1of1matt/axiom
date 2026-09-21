use std::path::PathBuf;
use std::io::{self, Write};
use anyhow::{Result, bail};
use crate::archive;
use crate::graph;
use crate::orchestrate;
use crate::util;
use crate::scan;

pub fn run(target: Option<&str>, verbose: bool, keep_temp: bool) -> Result<()> {
    util::print_header();
    util::ensure_axiom_dirs()?;

    let mut cleanup: Option<PathBuf> = None;

    let input = match target {
        None | Some(".") | Some("./") => std::env::current_dir()?,
        Some(t) => {
            let p = PathBuf::from(t);
            if p.exists() {
                p
            } else {
                let results = scan::find_projects(t, 10)?;
                if results.is_empty() {
                    bail!("no project found matching \"{}\"", t);
                }
                if results.len() == 1 {
                    results[0].path.clone()
                } else {
                    println!("Multiple matches for \"{}\":", t);
                    for (i, r) in results.iter().enumerate() {
                        println!("  [{}] {} — {}", i + 1, r.name, r.path.display());
                    }
                    eprint!("Select: ");
                    io::stdout().flush().ok();
                    let mut line = String::new();
                    io::stdin().read_line(&mut line)?;
                    let idx: usize = line.trim().parse().unwrap_or(1);
                    results
                        .get(idx.wrapping_sub(1))
                        .map(|r| r.path.clone())
                        .ok_or_else(|| anyhow::anyhow!("invalid selection"))?
                }
            }
        }
    };

    if input.extension().and_then(|e| e.to_str()) == Some("app")
        || input
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".app"))
            .unwrap_or(false)
    {
        println!("✓ Application bundle detected");
        if std::env::consts::OS == "macos" {
            let status = std::process::Command::new("open").arg(&input).status()?;
            if status.success() {
                println!("✓ Launched");
                return Ok(());
            }
        }
        println!("  · inspecting as directory...");
    }

    // Single-file artifacts (source, JAR, script, native binary)
    if input.is_file()
        && !archive::is_zip(&input)
        && !archive::is_tar_gz(&input)
    {
        if let Some(app) = graph::build_application_from_file(&input) {
            println!("✓ Artifact detected");
            println!("  {}", input.display());
            println!();
            graph::print_graph(&app);
            println!();
            if app.components.is_empty() {
                bail!("no runnable component for {}", input.display());
            }
            // JAR without Main-Class
            if app.components.iter().any(|c| {
                c.info.kinds.contains(&crate::detect::ProjectKind::Jar) && c.start.is_empty()
            }) {
                println!("This JAR does not declare an executable Main-Class.");
                println!("It may be a library rather than an executable application.");
                return Ok(());
            }
            return run_graph(app, verbose, keep_temp, None);
        }
    }

    let work_root = if archive::is_zip(&input) {
        println!("✓ ZIP detected");
        print!("✓ Extracting... ");
        io::stdout().flush().ok();
        let dest = archive::extract_zip(&input)?;
        println!("done");
        cleanup = Some(dest.clone());
        dest
    } else if archive::is_tar_gz(&input) {
        println!("✓ TAR.GZ detected");
        print!("✓ Extracting... ");
        io::stdout().flush().ok();
        let dest = archive::extract_tar_gz(&input)?;
        println!("done");
        cleanup = Some(dest.clone());
        dest
    } else if input.is_dir() {
        println!("✓ Project discovered");
        println!("  {}", input.display());
        input
    } else {
        bail!(
            "not a directory, archive, source file, JAR, executable, or .app: {}",
            input.display()
        );
    };

    println!();
    let app = graph::build_application(&work_root);
    graph::print_graph(&app);
    println!();

    if app.components.is_empty() {
        cleanup_if(&cleanup, keep_temp);
        bail!("no project components discovered");
    }

    run_graph(app, verbose, keep_temp, cleanup)
}

fn run_graph(
    app: crate::model::ApplicationGraph,
    verbose: bool,
    keep_temp: bool,
    cleanup: Option<PathBuf>,
) -> Result<()> {
    let work_root = app.root.clone();

    // Trust
    let plan_fp = app.plan_fingerprint();
    let trust_key = work_root
        .canonicalize()
        .unwrap_or_else(|_| work_root.clone())
        .to_string_lossy()
        .to_string();

    if !util::is_trusted_plan(&trust_key, &plan_fp) {
        println!("Axiom wants to run:");
        println!();
        let mut n = 1;
        for id in app.start_order_ids() {
            if let Some(c) = app.component(&id) {
                for p in &c.prepare {
                    if p.starts_with("__axiom_") {
                        continue;
                    }
                    println!("  {}. [prepare:{}] {}", n, c.id, p);
                    n += 1;
                }
                for s in &c.start {
                    println!("  {}. [start:{}] {}", n, c.id, s);
                    n += 1;
                }
            } else if let Some(p) = app.provider(&id) {
                if p.can_start && !p.reachable {
                    if let Some(ref cmd) = p.start_command {
                        println!("  {}. [provider:{}] {}", n, p.name, cmd);
                        n += 1;
                    }
                }
            }
        }
        println!();
        if !util::prompt_yes_no("Trust this project?", true) {
            println!("Aborted.");
            cleanup_if(&cleanup, keep_temp);
            return Ok(());
        }
        util::grant_trust_plan(&trust_key, &plan_fp)?;
        println!("✓ Trusted");
        println!();
    } else {
        println!("✓ Already trusted");
        println!();
    }

    let mut running = match orchestrate::orchestrate(&app, verbose) {
        Ok(r) => r,
        Err(e) => {
            cleanup_if(&cleanup, keep_temp);
            return Err(e);
        }
    };

    let result = orchestrate::supervise(&mut running);
    running.shutdown();
    cleanup_if(&cleanup, keep_temp);
    result
}

fn cleanup_if(cleanup: &Option<PathBuf>, keep_temp: bool) {
    if let Some(ref c) = cleanup {
        if keep_temp {
            println!("· Keeping temp (--keep-temp): {}", c.display());
        } else {
            print!("✓ Cleaning temporary workspace... ");
            io::stdout().flush().ok();
            match archive::cleanup_temp(c) {
                Ok(()) => println!("done"),
                Err(e) => println!("(skipped: {})", e),
            }
        }
    }
}
