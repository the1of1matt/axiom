//! Java and C++ fixture smoke tests for v0.1.3.
//! These validate fixture layout integrity. End-to-end Axiom runs are exercised
//! manually/in CI when the relevant toolchains are on PATH.

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn java_plain_fixture_has_main() {
    let root = fixture("java-plain");
    let src = root.join("src/main/java/com/example/Hello.java");
    assert!(src.is_file());
    let body = std::fs::read_to_string(src).unwrap();
    assert!(body.contains("public static void main"));
}

#[test]
fn java_maven_app_fixture() {
    let root = fixture("java-maven-app");
    assert!(root.join("pom.xml").is_file());
    assert!(root.join("src/main/java/com/example/App.java").is_file());
    let pom = std::fs::read_to_string(root.join("pom.xml")).unwrap();
    assert!(pom.contains("mainClass") || pom.contains("exec-maven-plugin"));
}

#[test]
fn java_maven_lib_fixture() {
    let root = fixture("java-maven-lib");
    assert!(root.join("pom.xml").is_file());
    let lib = std::fs::read_to_string(root.join("src/main/java/com/example/Lib.java")).unwrap();
    assert!(!lib.contains("public static void main"));
}

#[test]
fn java_gradle_app_fixture() {
    let root = fixture("java-gradle-app");
    assert!(root.join("build.gradle").is_file());
    assert!(root.join("settings.gradle").is_file());
    let build = std::fs::read_to_string(root.join("build.gradle")).unwrap();
    assert!(build.contains("application"));
}

#[test]
fn java_gradle_lib_fixture() {
    let root = fixture("java-gradle-lib");
    let build = std::fs::read_to_string(root.join("build.gradle")).unwrap();
    assert!(build.contains("java-library"));
}

#[test]
fn cmake_exe_fixture() {
    let root = fixture("cmake-exe");
    let cmake = std::fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    assert!(cmake.contains("add_executable"));
    assert!(root.join("main.cpp").is_file());
}

#[test]
fn cmake_lib_fixture() {
    let root = fixture("cmake-lib");
    let cmake = std::fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    assert!(cmake.contains("add_library"));
    assert!(!cmake.contains("add_executable"));
}

#[test]
fn make_exe_fixture() {
    let root = fixture("make-exe");
    assert!(root.join("Makefile").is_file());
    assert!(root.join("main.c").is_file());
}
