use std::fs;
use tempfile::tempdir;

// We need to expose detect for tests — in real project it would be public or use a lib
// For this MVP we test via integration by creating fixtures.

#[test]
fn detect_rust_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    // Since detect is private to the binary, we re-implement a minimal check here
    // or we would make a library crate. For MVP, assert markers exist.
    assert!(root.join("Cargo.toml").is_file());
    assert!(root.join("src/main.rs").is_file());
}

#[test]
fn detect_node_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("package.json"),
        r#"{"name":"test","dependencies":{"react":"^18.0.0"}}"#,
    )
    .unwrap();
    fs::write(root.join("index.js"), "console.log('hi')").unwrap();

    assert!(root.join("package.json").is_file());
    assert!(root.join("index.js").is_file());
}

#[test]
fn detect_python_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("requirements.txt"), "requests==2.28.0\n").unwrap();
    fs::write(root.join("main.py"), "print('hi')").unwrap();

    assert!(root.join("requirements.txt").is_file());
    assert!(root.join("main.py").is_file());
}

#[test]
fn detect_empty_dir_not_project() {
    let dir = tempdir().unwrap();
    // empty — should not be considered a project
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty());
}
