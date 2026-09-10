# Axiom

<img width="974" height="256" alt="Firefly_RemoveBackground-removebg-preview" src="https://github.com/user-attachments/assets/4a654820-a8a0-4342-aba2-8abed2b558f3" />

<p align="center">
  <a href="https://github.com/the1of1matt/axiom/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT">
  </a>
  <a href="https://axiom.fwh.is/">
    <img src="https://img.shields.io/badge/website-axiom.fwh.is-blue.svg" alt="Website">
  </a>
  <a href="https://github.com/the1of1matt/axiom/releases/tag/v0.1.3">
    <img src="https://img.shields.io/badge/release-v0.1.3-green.svg" alt="Release: v0.1.3">
  </a>
</p>


---

**Eliminate developer toolchain / setup hell.**

Axiom is a small, free, open-source **local native CLI**.  
Install it **once**. Then create, find, and run projects with almost no commands.

```bash
axiom new my-app
cd my-app
axiom run
```

Or for an existing project anywhere on your machine:

```bash
axiom find my-app
axiom run my-app
axiom run ~/Desktop/folder
axiom run ~/Downloads/project.zip
```

**Current release:** [Axiom v0.1.3](https://github.com/the1of1matt/axiom/releases/tag/v0.1.3)

---

## What's new in v0.1.3

v0.1.3 expands Axiom beyond Node, Python, and Rust into real end-to-end orchestration for additional ecosystems:

- **Go** — module detection (`go.mod` / `go.work`), CLI vs library classification, `go mod download` when needed, `go run` / `go build` verification
- **Plain Java** — conventional `src/main/java` layouts, main-class discovery, compile into an Axiom-managed output directory, run
- **Maven** — `pom.xml`, wrapper preference (`mvnw` / `mvnw.cmd`), dependency resolution, application run (`exec:java` / Spring Boot when declared), library package verification
- **Gradle** — `build.gradle` / `.kts` and settings files, wrapper preference (`gradlew`), application `run` vs library `build` verification
- **CMake** — out-of-source builds, executable run, library build verification
- **Make** — target inspection, build and run when appropriate
- **Meson** — project detection and orchestration scaffolding (see Supported stacks for verification status)
- Shared **prepare → build → run → verify** pipeline, caching/isolation under `~/.axiom`, and structured diagnostics

This release does **not** use a codename.

---

## Install (end users)

**You do not need Rust, Cargo, Node, npm, Python, Go, Homebrew, or any other development toolchain to install Axiom.**

### macOS / Linux — one-line install

```bash
curl -fsSL https://raw.githubusercontent.com/the1of1matt/axiom/main/scripts/install.sh | sh
```

Then **open a new terminal window** and run:

```bash
axiom --version
axiom doctor
```

The installer will:

1. Detect your OS (macOS / Linux) and CPU (Apple Silicon / Intel / x86_64 / aarch64)
2. Download the matching prebuilt archive from [GitHub Releases](https://github.com/the1of1matt/axiom/releases)
3. Extract and install the binary to `~/.axiom/bin/axiom` (user-owned, no `sudo`)
4. Add `~/.axiom/bin` to your PATH in shell startup files (`.zprofile`, `.zshrc`, `.bash_profile`, `.bashrc`, `.profile` as appropriate)
5. Verify the binary runs
6. Print exactly what it did

### Windows

1. Download `axiom-windows-x64.zip` from [Releases](https://github.com/the1of1matt/axiom/releases/latest).
2. Extract somewhere permanent (e.g. `%LOCALAPPDATA%\Axiom`).
3. Add that folder to your user PATH, or run `axiom.exe` by full path.

```powershell
.\axiom.exe --version
.\axiom.exe doctor
.\axiom.exe run .\project.zip
```

### Uninstall

```bash
axiom uninstall --yes
```

Removes **only** Axiom's binary and `~/.axiom` data — never your projects.

---

## Supported stacks

| Stack | Detect | Prepare | Build | Run | Verify |
|-------|--------|---------|-------|-----|--------|
| Node / npm / Vite / React / Electron | Yes | Yes | Yes | Yes | Yes |
| Python | Yes | Yes | Yes | Yes | Yes |
| Rust | Yes | Yes | Yes | Yes | Yes |
| ZIP archives of supported projects | Yes | Yes | Yes | Yes | Yes |
| Go | Yes | Yes | Yes | Yes | Yes |
| Plain Java | Yes | Yes | Yes | Yes | Yes |
| Java / Maven | Yes | Yes | Yes | Yes | Yes |
| Java / Gradle | Yes | Yes | Yes | Yes | Yes |
| C++ / CMake | Yes | Yes | Yes | Yes | Yes |
| C++ / Make | Yes | Yes | Yes | Yes | Yes |
| C++ / Meson | Yes | Scaffolding | Scaffolding | Scaffolding | Not E2E-verified in v0.1.3* |

\* Meson detection and command planning are implemented. Full end-to-end verification was **not** completed in the v0.1.3 release environment because Meson was unavailable there. Treat Meson support as provisional until confirmed on a host with `meson` and `ninja`.

Axiom does **not** claim to run every repository on GitHub. Libraries are verified with appropriate build/check operations rather than being forced to "run" as applications.

---

## Commands

| Command | What it does |
|---------|----------------|
| `axiom new` | Creates a minimal multi-runtime starter |
| `axiom find` | Safely scans common directories for matching projects |
| `axiom run` | Discovers components, prepares deps, orchestrates run |
| `axiom doctor` | Reports OS, arch, toolchains, Axiom home, health |
| `axiom uninstall` | Removes Axiom binary + `~/.axiom` only |

---

## Dependency cache and isolation

```text
~/.axiom/cache/node/<os>-<arch>/<fingerprint>/
~/.axiom/cache/python/...
~/.axiom/cache/build/...   # e.g. CMake / javac outputs when used
```

Node fingerprints include lockfile content, OS, architecture, and Node major version. A macOS cache is never restored on Windows. Python prefers managed virtualenvs under Axiom cache (or project `.venv`) rather than treating global site-packages as prepared.

---

## Historical note — v0.1.2-aegis

[v0.1.2-aegis](https://github.com/the1of1matt/axiom/releases/tag/v0.1.2-aegis) was a correctness and reliability release for Node, Python, and Rust:

- Metadata-first detection (skip `action/`, `.github/`, tests/examples as false entry points)
- Node dependency health/repair and OS/arch-scoped cache
- Python managed venvs and honest dependency checks
- Rust toolchain/workspace awareness; libraries via `cargo check`
- Persistent server vs one-shot CLI vs library execution modes
- Structured PASS / FAIL / partial verification notes

Those behaviors remain in v0.1.3; this release adds Go, Java, and C++ orchestration on the same architecture.

---

## Build from source (developers only)

```bash
git clone https://github.com/the1of1matt/axiom.git
cd axiom
cargo build --release
```

End users should **never** be told to run `cargo build`.

---

## Project layout after install

```text
~/.axiom/
├── bin/axiom
├── cache/          # node, python, and build outputs
├── toolchains/
├── packages/
├── projects/
└── tmp/            # ZIP extracts (Axiom-owned)
```

---

## License

MIT. See [LICENSE](LICENSE).
