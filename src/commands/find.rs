use anyhow::Result;
use crate::scan;
use crate::util;

pub fn run(name: &str, limit: usize) -> Result<()> {
    util::print_header();

    let results = scan::find_projects(name, limit)?;

    if results.is_empty() {
        println!("No projects found matching \"{}\"", name);
        return Ok(());
    }

    println!("Found {} project(s) matching \"{}\":", results.len(), name);
    println!();
    for (i, r) in results.iter().enumerate() {
        println!("  [{}] {}", i + 1, r.name);
        println!("      {}", r.path.display());
        if !r.kinds.is_empty() {
            println!("      kinds: {}", r.kinds.join(", "));
        }
    }
    Ok(())
}
