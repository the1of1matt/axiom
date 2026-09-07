# Axiom Roadmap

## MVP (done)

- [x] Four core commands: `new`, `find`, `run`, `doctor`
- [x] Minimal project creation (Rust + Node + Python markers)
- [x] Safe project scanner (no code execution, symlink-loop safe, system-dir skip)
- [x] Basic project type detection
- [x] Toolchain presence checks
- [x] Safe entry-point execution (cargo / node / python)
- [x] `~/.axiom` directory layout
- [x] MIT license, README, basic tests, GitHub Actions skeleton

## Near-term

- [ ] Proper library crate so unit tests can call `detect::inspect` directly
- [ ] `axiom run path/to/archive.zip` with hardened extraction (path traversal protection, temp dir under `~/.axiom/tmp`, never overwrite original)
- [ ] Better multi-match resolution for `find` / `run` (interactive or scored ranking)
- [ ] Verbose / quiet / JSON output modes
- [ ] `axiom clean` for Axiom-owned temporary data only

## Toolchain management

- [ ] Download and cache isolated Node / Rust / Python toolchains under `~/.axiom/toolchains`
- [ ] Per-project toolchain pinning (e.g. `.axiom/toolchain.toml`)
- [ ] Never touch the user’s global installations
- [ ] Cold / warm / hot cache philosophy

## Trust model

- [ ] Explicit stages: inspect → prepare → build → run
- [ ] User confirmation (or policy file) before any network or package-manager action
- [ ] Never auto-run arbitrary npm/cargo/python scripts discovered in a project

## Distribution

- [ ] GitHub Releases with signed binaries for:
  - macOS Apple Silicon (primary)
  - macOS Intel
  - Linux x86_64 / aarch64
  - Windows (later)
- [ ] Tiny bootstrap: `curl -fsSL https://…/install.sh | sh`
- [ ] Homebrew / other package managers (optional)

## Later detectors & runners

- [ ] Go (`go.mod`)
- [ ] Java / Gradle / Maven
- [ ] .NET
- [ ] Pure C/C++ beyond CMake
- [ ] More precise Vite / Next / Remix / etc. detection
- [ ] Tauri & Electron full prepare/build/run (still without shipping those frameworks)

## Non-goals

- Becoming a package manager that replaces npm/cargo/pip
- Requiring a daemon or always-on server
- Forcing a new project template ecosystem
- Shipping React / Tauri / Electron / Go as dependencies of Axiom itself
