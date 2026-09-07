# Axiom

<img width="974" height="256" alt="Firefly_RemoveBackground-removebg-preview" src="https://github.com/user-attachments/assets/4a654820-a8a0-4342-aba2-8abed2b558f3" />

<p align="center">
  <a href="https://github.com/the1of1matt/axiom/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT">
  </a>
  <a href="https://axiom.fwh.is/">
    <img src="https://img.shields.io/badge/website-axiom.fwh.is-blue.svg" alt="Website">
  </a>
  <a href="https://github.com/the1of1matt/axiom/releases">
    <img src="https://img.shields.io/badge/release-v0.1.2--aegis-green.svg" alt="Release: v0.1.2-aegis">
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

**Current release:** [Axiom v0.1.2-aegis](https://github.com/the1of1matt/axiom/releases/tag/v0.1.2-aegis) — a hardening release driven by real-world project benchmarks.

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

## Aegis (v0.1.2) — what improved

Aegis is a **correctness and reliability** release. Changes were driven by failures observed against public open-source projects, not by hardcoded special cases.

### Smarter project detection

- Prefers **metadata** (package.json scripts, pyproject scripts, Cargo targets) over arbitrary files
- Skips tooling paths (`action/`, `.github/`, `tests/`, `examples/`, …) as application entry points
- Classifies projects as application / CLI / library / package when evidence supports it

### Dependency preparation and isolation

- **Node:** health checks, repair when broken, OS/arch-scoped dependency cache
- **Python:** managed virtualenvs under `~/.axiom/cache/python/…` (or project `.venv`); does **not** treat global site-packages as "prepared"
- Requirements discovery includes `requirements.txt`, `requirements/*.txt`, `pyproject.toml`, `setup.py`, Pipfile

### Rust toolchain and workspaces

- Reads `rust-toolchain` / `rust-toolchain.toml` and Cargo **edition** (e.g. edition 2024)
- Plans `rustup toolchain install` when a required channel is missing
- Workspace-aware starts; libraries verified with `cargo check` instead of blind `cargo run`

### Execution semantics and verification

- **Persistent servers** vs **one-shot CLIs** vs **libraries**
- Exit code **0 is success** for one-shot commands (not a "crash")
- Port/HTTP readiness checks for servers; structured verification notes for packages

### Diagnostics

Failures report project type, mode, command, path, and actionable hints (e.g. missing `python3-venv`) instead of generic "component failed".

---

## Supported stacks

| Stack | Detection | Prepare | Run / verify |
|-------|-----------|---------|--------------|
| Node / npm / Vite / React / Electron | Yes | npm/yarn/pnpm + cache | scripts / server |
| Python | Yes | managed venv + pip | app/CLI/package modes |
| Rust | Yes | cargo / rustup when available | bin / lib / workspace |
| ZIP archives of the above | Yes | same | same |
| Go / Java / C++ | Detected only | — | limited / unsupported for full orchestration |

Axiom does **not** claim to run every repository on GitHub.

---

## Benchmark integrity (Aegis)

Public projects were used to find failure modes. Historical results are not rewritten. Re-runs are labeled **BEFORE Aegis** vs **AFTER Aegis**.

Statuses used:

| Status | Meaning |
|--------|---------|
| **PASS** | Detection, preparation, and execution/verification succeeded for the intended mode |
| **FAIL** | Axiom attempted the work and failed (bug or environment) |
| **INCONCLUSIVE** | Ambiguous entry point, missing host tooling (e.g. no `ensurepip`), or upstream broken |
| **UNSUPPORTED** | Outside supported orchestration (e.g. pure library with no run target, other languages) |

### Representative outcomes (fixture verification after Aegis)

| Class | Example shape | After Aegis |
|-------|---------------|-------------|
| Rust CLI | `src/main.rs` binary | **PASS** — exit 0 treated as success |
| Rust library | `src/lib.rs` only | **PASS** — `cargo check`, not "server crash" |
| Python package + `action/main.py` | metadata package (Black-like) | Does **not** run Actions wrappers as the app |
| Python web app | Flask + `requirements.txt` | Plans managed venv + install; needs host `python3-venv` |
| Node one-shot / server | package.json scripts | Prepare + start; server mode when frameworks detected |

Full upstream clones may still **FAIL** or be **INCONCLUSIVE** for host reasons (old global Rust without rustup, missing Python venv support). That is reported honestly.

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

## Dependency cache (Node)

```text
~/.axiom/cache/node/<os>-<arch>/<fingerprint>/
```

Fingerprints include lockfile content, OS, architecture, and Node major version. A macOS cache is never restored on Windows.

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
├── cache/          # node + python managed envs
├── toolchains/
├── packages/
├── projects/
└── tmp/            # ZIP extracts (Axiom-owned)
```

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for **v0.1.2-aegis** notes.

---

## License

MIT. See [LICENSE](LICENSE).
