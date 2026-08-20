use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use crate::util;

pub fn run(name: &str, path: Option<&str>) -> Result<()> {
    util::print_header();

    // Sanitize name
    let name = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if name.is_empty() {
        anyhow::bail!("invalid project name");
    }

    let base = match path {
        Some(p) => Path::new(p).to_path_buf(),
        None => std::env::current_dir()?,
    };

    let project_dir = base.join(&name);
    if project_dir.exists() {
        anyhow::bail!("directory already exists: {}", project_dir.display());
    }

    fs::create_dir_all(&project_dir)
        .with_context(|| format!("failed to create {}", project_dir.display()))?;

    // Create a minimal multi-language starter that Axiom can detect and run.
    // We create a simple "hello" that works with multiple runtimes if present.

    // 1. A simple Rust binary (preferred for native)
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(
        src_dir.join("main.rs"),
        r#"fn main() {
    println!("Hello from Axiom!");
    println!("Project: {} created successfully.", env!("CARGO_PKG_NAME"));
}
"#,
    )?;

    fs::write(
        project_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#
        ),
    )?;

    // 2. Also a package.json so Node detection works, with a simple script
    fs::write(
        project_dir.join("package.json"),
        format!(
            r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "description": "Created by Axiom",
  "main": "index.js",
  "scripts": {{
    "start": "node index.js",
    "dev": "node index.js"
  }},
  "keywords": [],
  "author": "",
  "license": "MIT"
}}
"#
        ),
    )?;

    fs::write(
        project_dir.join("index.js"),
        r#"console.log("Hello from Axiom!");
console.log("This is a minimal Node project.");
"#,
    )?;

    // 3. Python entry
    fs::write(
        project_dir.join("main.py"),
        r#"#!/usr/bin/env python3
print("Hello from Axiom!")
print("This is a minimal Python project.")
"#,
    )?;

    // README
    fs::write(
        project_dir.join("README.md"),
        format!(
            r#"# {name}

Created with [Axiom](https://github.com/axiom-dev/axiom).

## Run

```bash
axiom run
```

Or manually:

- Rust: `cargo run`
- Node: `node index.js` or `npm start`
- Python: `python3 main.py`
"#
        ),
    )?;

    // .gitignore
    fs::write(
        project_dir.join(".gitignore"),
        r#"target/
node_modules/
__pycache__/
*.pyc
.DS_Store
.env
"#,
    )?;

    println!("✓ Created project");
    println!("  {}", project_dir.display());
    println!();
    println!("Detected markers:");
    println!("  Cargo.toml  (Rust)");
    println!("  package.json (Node)");
    println!("  main.py     (Python)");
    println!();
    println!("Next:");
    println!("  cd {}", name);
    println!("  axiom run");
    println!();

    Ok(())
}
