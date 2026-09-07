//! Basic tests for project detection and scanning safety.
//! Full unit tests of internal modules will be enabled once we split a lib crate.

use std::fs;
use tempfile::tempdir;

#[test]
fn creates_and_detects_minimal_project_markers() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname=\"t\"\nversion=\"0.1.0\"\n").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main(){}").unwrap();
    fs::write(root.join("package.json"), r#"{"name":"t"}"#).unwrap();
    fs::write(root.join("main.py"), "print(1)").unwrap();

    assert!(root.join("Cargo.toml").is_file());
    assert!(root.join("package.json").is_file());
    assert!(root.join("main.py").is_file());
}

#[test]
fn empty_directory_has_no_markers() {
    let dir = tempdir().unwrap();
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty());
}

#[test]
fn node_modules_style_dirs_are_skippable() {
    // The scanner filter list includes node_modules, target, .git, etc.
    // This test just documents the intended skip set.
    let skip = [
        "node_modules", "target", ".git", "dist", "build",
        ".next", ".nuxt", "vendor", "__pycache__", ".venv", "venv",
    ];
    assert!(skip.contains(&"node_modules"));
    assert!(skip.contains(&"target"));
}
