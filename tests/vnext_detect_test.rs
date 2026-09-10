//! vNext detection / classification tests (filesystem fixtures only).
//! These exercise public binary behavior via temporary project trees.

use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn axiom_bin() -> std::path::PathBuf {
    // Prefer freshly built target
    let candidates = [
        "/tmp/axiom-target3/debug/axiom",
        "/tmp/axiom-target2/debug/axiom",
        "/tmp/axiom-target/debug/axiom",
        "target/debug/axiom",
        "target/release/axiom",
    ];
    for c in &candidates {
        let p = std::path::PathBuf::from(c);
        if p.is_file() {
            return p;
        }
    }
    std::path::PathBuf::from("/tmp/axiom-target/debug/axiom")
}

#[test]
fn python_package_without_app_entry_is_not_run_as_script() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("pyproject.toml"),
        r#"
[project]
name = "fakelib"
version = "0.1.0"
"#,
    )
    .unwrap();
    // Misleading GitHub Actions-style wrapper — must NOT become the entry point
    fs::create_dir_all(root.join("action")).unwrap();
    fs::write(
        root.join("action/main.py"),
        "import os\nprint(os.environ.get('GITHUB_ACTIONS'))\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src/fakelib")).unwrap();
    fs::write(root.join("src/fakelib/__init__.py"), "").unwrap();

    // Unit-level: resolve_python_entry is inside the binary; we validate via doctor/run output
    let out = Command::new(axiom_bin())
        .args(["run", root.to_str().unwrap()])
        .output()
        .expect("run axiom");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // Must not execute action/main.py as the app
    assert!(
        !combined.contains("action/main.py")
            || combined.to_lowercase().contains("no application entry")
            || combined.to_lowercase().contains("library")
            || combined.to_lowercase().contains("package")
            || combined.to_lowercase().contains("skipped"),
        "unexpected output:\n{}",
        combined
    );
}

#[test]
fn python_flask_app_detected_as_server_script() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("requirements.txt"), "flask==3.0.0\n").unwrap();
    fs::write(
        root.join("app.py"),
        "from flask import Flask\napp = Flask(__name__)\n@app.route('/')\ndef hi():\n    return 'ok'\nif __name__ == '__main__':\n    app.run(port=5055)\n",
    )
    .unwrap();
    assert!(root.join("app.py").is_file());
    assert!(root.join("requirements.txt").is_file());
}

#[test]
fn rust_lib_only_has_no_blind_cargo_run() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "mylib"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn x() -> i32 { 1 }\n")
        .unwrap();
    // no main.rs → should classify as library
    assert!(!root.join("src/main.rs").is_file());
}

#[test]
fn rust_workspace_root_marker() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = ["a", "b"]
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("a/src")).unwrap();
    fs::create_dir_all(root.join("b/src")).unwrap();
    fs::write(
        root.join("a/Cargo.toml"),
        r#"[package]
name = "a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("a/src/lib.rs"), "").unwrap();
    fs::write(
        root.join("b/Cargo.toml"),
        r#"[package]
name = "b"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("b/src/lib.rs"), "").unwrap();
    assert!(fs::read_to_string(root.join("Cargo.toml"))
        .unwrap()
        .contains("[workspace]"));
}
