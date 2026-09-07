# Changelog

## [v0.1.2-aegis] — 2026-09-07

**Codename: Aegis** — hardening release driven by real-world public-project benchmarks.

### Focus

Correctness and reliability for Node, Python, and Rust projects. No language-support expansion. No benchmark-specific hardcoding.

### Added

- `ExecutionMode` and `ProjectClass` on application components (server / one-shot CLI / library / build-only / ambiguous)
- Project-aware Python entry resolution (pyproject scripts, package `__main__`, root app scripts; excludes `action/`, `.github/`, tests, examples)
- Managed Python virtualenv preparation under Axiom cache (with clear failure if host lacks `ensurepip` / `python3-venv`)
- Real Python dependency health checks (does not treat global site-packages as prepared)
- Rust toolchain hints from `rust-toolchain`, `rust-toolchain.toml`, and Cargo edition
- Workspace-aware Rust start planning; library path uses `cargo check`
- Structured verification output for non-server targets
- Improved diagnostics (command, path, mode, actionable hints)

### Fixed

- `python_deps_look_ok` effectively always true → real venv + requirement sampling
- Arbitrary `.py` files (e.g. GitHub Actions wrappers) selected as application entry points
- Process exit code 0 treated as failure for legitimate one-shot CLIs
- Supervise loop hanging when no persistent children remained
- Blind `cargo run` on pure library crates

### Benchmark policy

Historical benchmark evidence is preserved. Post-Aegis re-runs are reported separately with PASS / FAIL / INCONCLUSIVE / UNSUPPORTED. Success requires Axiom-attributable preparation where isolation is claimed.

### Not in this release

- Go / Java / C++ full orchestration
- Automatic Rust install without rustup
- Global `pip install` fallback when venv cannot be created

---

## [v0.1.1] — 2026-08-21

Installer PATH fixes; release asset naming aligned with `.tar.gz` / `.zip` archives; repo URL corrections.

## [v0.1.0] — 2026-08-20

Initial public MVP: discover, prepare (Node), orchestrate, ZIP support, cross-platform builds.
