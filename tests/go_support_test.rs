//! Go fixture integrity and toolchain smoke tests for v0.1.3.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn go_cli_fixture_is_valid_module() {
    let root = fixture("go-cli");
    assert!(root.join("go.mod").is_file());
    assert!(root.join("main.go").is_file());
    let content = std::fs::read_to_string(root.join("go.mod")).unwrap();
    assert!(content.contains("module example.com/hello"));
    assert!(content.contains("go 1.21"));
    let main = std::fs::read_to_string(root.join("main.go")).unwrap();
    assert!(main.contains("package main"));
}

#[test]
fn go_lib_fixture_is_library_only() {
    let root = fixture("go-lib");
    let lib = std::fs::read_to_string(root.join("lib.go")).unwrap();
    assert!(!lib.contains("package main"));
    assert!(lib.contains("package libonly"));
}

#[test]
fn go_cmd_fixture_has_cmd_package() {
    let root = fixture("go-cmd");
    assert!(root.join("cmd/greeter/main.go").is_file());
    let main = std::fs::read_to_string(root.join("cmd/greeter/main.go")).unwrap();
    assert!(main.contains("package main"));
}

#[test]
fn go_cli_runs_with_go_toolchain() {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("skip: go not installed");
        return;
    }
    let root = fixture("go-cli");
    let out = Command::new("go")
        .args(["run", "."])
        .current_dir(&root)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hello from axiom go fixture"));
}

#[test]
fn go_cmd_package_runs() {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("skip: go not installed");
        return;
    }
    let root = fixture("go-cmd");
    let out = Command::new("go")
        .args(["run", "./cmd/greeter"])
        .current_dir(&root)
        .output()
        .expect("go run cmd");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("greeter ok"));
}
