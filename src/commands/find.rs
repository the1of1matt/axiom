use anyhow::Result;
use crate::scan;
use crate::util;

pub fn run(name: &str, limit: usize) -> Result<()> {
    util::print_header();

    let results = scan::find_projects(name, limit)?;

    if results.is_empty() {
        println!("No projects found matching \"{}\"", name);
        println!();
        println!("Tips:");
        println!("  • Axiom searches common directories under your home");
        println!("  • Try a more specific name or full path");
        println!("  • Create one with: axiom new {}", name);
        return Ok(());
    }

    println!("Found {} project(s) matching \"{}\":", results.len(), name);
    println!();

    for (i, info) in results.iter().enumerate() {
        println!("  [{}] {}", i + 1, info.name);
        println!("      Path:    {}", info.path.display());
        println!("      Type:    {}", info.display_kinds());
        println!("      Markers: {}", info.markers.join(", "));
        println!();
    }

    if results.len() == 1 {
        println!("Run with:");
        println!("  axiom run {}", results[0].name);
    } else {
        println!("Run a specific one with:");
        println!("  axiom run <path-or-name>");
    }

    Ok(())
}
