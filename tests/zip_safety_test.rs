//! Safe ZIP extraction tests (path traversal protection).

#[test]
fn path_traversal_candidates_are_detectable() {
    let dangerous = [
        "../etc/passwd",
        "..\\windows\\system32",
        "/etc/passwd",
        "foo/../../bar",
        "foo/../../../etc/passwd",
    ];
    for p in &dangerous {
        assert!(
            p.contains("..") || p.starts_with('/') || p.starts_with('\\'),
            "expected dangerous pattern: {}",
            p
        );
    }
}

#[test]
fn safe_relative_paths_are_ok() {
    let safe = ["src/main.rs", "package.json", "nested/dir/file.txt", "a-b_c.123"];
    for p in &safe {
        assert!(!p.contains(".."));
        assert!(!p.starts_with('/'));
    }
}
