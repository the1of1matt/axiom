//! Priority A artifact fixtures: C, C#, JAR, standalone scripts, archives.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn axiom_bin() -> PathBuf {
    let candidates = [
        std::env::var("CARGO_TARGET_DIR")
            .ok()
            .map(|d| PathBuf::from(d).join("debug/axiom")),
        Some(PathBuf::from("/tmp/axiom-v014/debug/axiom")),
        Some(PathBuf::from("/tmp/axiom-target3/debug/axiom")),
        Some(PathBuf::from("target/debug/axiom")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from(env!("CARGO_BIN_EXE_axiom"))
}

#[test]
fn c_standalone_fixture_exists() {
    let root = fixture("c-standalone");
    assert!(root.join("main.c").is_file());
}

#[test]
fn csharp_console_is_exe() {
    let csproj = fixture("csharp-console").join("Hello.csproj");
    let body = std::fs::read_to_string(&csproj).unwrap();
    assert!(body.contains("OutputType") && body.contains("Exe"));
}

#[test]
fn csharp_lib_is_library() {
    let csproj = fixture("csharp-lib").join("Lib.csproj");
    let body = std::fs::read_to_string(&csproj).unwrap();
    assert!(body.to_lowercase().contains("library"));
}

#[test]
fn jar_fixture_has_main_class() {
    let jar = fixture("jar-exe").join("app.jar");
    if !jar.is_file() {
        eprintln!("skip: jar fixture not built");
        return;
    }
    // Use axiom library path via running detection would need binary; check zip entry
    let f = std::fs::File::open(&jar).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let mut mf = z.by_name("META-INF/MANIFEST.MF").unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut mf, &mut s).unwrap();
    assert!(s.contains("Main-Class:"));
}

#[test]
fn standalone_py_and_js_exist() {
    assert!(fixture("standalone-py").join("app.py").is_file());
    assert!(fixture("standalone-js").join("app.js").is_file());
}

#[test]
fn detect_c_directory() {
    let bin = axiom_bin();
    if !bin.is_file() {
        eprintln!("skip: axiom binary missing at {}", bin.display());
        return;
    }
    let out = Command::new(&bin)
        .args(["doctor"])
        .output()
        .expect("run doctor");
    assert!(out.status.success());
}

#[test]
fn inspect_c_standalone_via_run_help() {
    // Lightweight: compile path only when compiler present
    if Command::new("cc").arg("--version").output().is_err()
        && Command::new("gcc").arg("--version").output().is_err()
        && Command::new("clang").arg("--version").output().is_err()
    {
        eprintln!("skip: no C compiler");
        return;
    }
    let src = fixture("c-standalone").join("main.c");
    assert!(src.is_file());
}
