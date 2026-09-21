# Axiom

<img width="974" height="256" alt="Firefly_RemoveBackground-removebg-preview" src="https://github.com/user-attachments/assets/4a654820-a8a0-4342-aba2-8abed2b558f3" />

<p align="center">
  <a href="https://github.com/the1of1matt/axiom/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT">
  </a>
  <a href="https://axiom.fwh.is/">
    <img src="https://img.shields.io/badge/website-axiom.fwh.is-blue.svg" alt="Website">
  </a>
  <a href="https://github.com/the1of1matt/axiom/releases/tag/v0.1.4">
    <img src="https://img.shields.io/badge/release-v0.1.4-green.svg" alt="Release: v0.1.4">
  </a>
</p>

https://github.com/user-attachments/assets/134168e1-1071-4132-97fa-162c345737e8

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

## What's new (v0.1.3 line)

v0.1.3 expanded Axiom beyond Node, Python, and Rust. Follow-on work adds broader artifact execution on the same architecture:

- **Go** — module detection, CLI vs library, `go mod download` / `go run` / `go build`
- **Java** — plain, Maven, Gradle (as in v0.1.3)
- **C** — standalone `.c` compile with clang/gcc/cc and run
- **C# / .NET** — `*.csproj` / `*.sln` via `dotnet restore` / `build` / `run`; libraries build-only
- **Executable JAR** — `java -jar` when `Main-Class` is present
- **Standalone scripts** — `axiom run app.py` / `app.js` / `app.mjs`
- **Native binaries** — ELF / Mach-O / PE detection and direct launch
- **Archives** — ZIP and TAR.GZ / TGZ into Axiom-owned temp workspaces
- **CMake / Make / Meson** — as in v0.1.3
- Pipeline: **Detect → Classify → Prepare → Build → Run → Verify**

No AI/LLM features in this iteration. No release codename.

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
| Go | Yes | Yes | Yes | Yes | Yes |
| Plain Java | Yes | Yes | Yes | Yes | Yes |
| Java / Maven | Yes | Yes | Yes | Yes | Yes |
| Java / Gradle | Yes | Yes | Yes | Yes | Yes |
| Executable JAR (`Main-Class`) | Yes | — | — | Yes | Yes |
| C (standalone `.c`) | Yes | — | Yes | Yes | Yes |
| C++ / CMake | Yes | Yes | Yes | Yes | Yes |
| C++ / Make | Yes | Yes | Yes | Yes | Yes |
| C++ / Meson | Yes | Scaffolding | Scaffolding | Scaffolding | Not E2E-verified* |
| C# / .NET (`*.csproj` / `*.sln`) | Yes | `dotnet restore` | `dotnet build` | `dotnet run`† | Yes |
| Standalone `.py` / `.js` / `.mjs` | Yes | — | — | Yes | Yes |
| Native binaries (ELF / Mach-O / PE) | Yes | — | — | Yes | Yes |
| ZIP / TAR.GZ of supported projects | Yes | Yes | Yes | Yes | Yes |

\* Meson detection and command planning are implemented; full E2E verification requires `meson` + `ninja` on the host.  
† Class libraries are built and verified; they are not launched as applications.

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
