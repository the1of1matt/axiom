# Axiom

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
axiom run ~/Desktop/MGMIDIController
axiom run ~/Downloads/project.zip
```

---

## Install (end users)

**You do not need Rust, Cargo, Node, npm, Python, Go, Homebrew, or any other development toolchain.**

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

After a new terminal is opened, `axiom` should work without any manual PATH edits.

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

This removes **only** Axiom’s own binary and `~/.axiom` data.  
It never touches your projects or unrelated software.

You may optionally remove the `# Axiom CLI` PATH lines from your shell profile.

### Manual install

1. Open [Releases](https://github.com/the1of1matt/axiom/releases/latest)
2. Download the asset for your machine:
   - `axiom-macos-aarch64.tar.gz` (Apple Silicon)
   - `axiom-linux-x86_64.tar.gz`
   - `axiom-windows-x64.zip`
3. Extract; place the `axiom` / `axiom.exe` binary in a directory on your PATH  
   (recommended: `~/.axiom/bin/` on macOS/Linux)

---

## Dependency cache

For Node projects Axiom can reuse a dependency cache under:

```text
~/.axiom/cache/node/<os>-<arch>/<fingerprint>/
```

Fingerprints include lockfile content, OS, architecture, and Node major version  
so a cache from macOS is never restored on Windows, and vice versa.

Typical flow:

- **First run:** install dependencies → save cache  
- **Later runs:** cache hit → restore `node_modules` → skip package-manager install

---

## Build from source (developers only)

Only needed if you are **developing Axiom itself**.

```bash
git clone https://github.com/the1of1matt/axiom.git
cd axiom
cargo build --release
# binary: target/release/axiom
```

Requirements for developers: a recent Rust toolchain (`rustup`).

End users should **never** be told to run `cargo build`.

---

## Commands

| Command | What it does |
|---------|----------------|
| `axiom new` | Creates a minimal multi-runtime starter |
| `axiom find` | Safely scans common directories for matching projects |
| `axiom run` | Discovers components, prepares deps, orchestrates run |
| `axiom doctor` | Reports OS, arch, toolchains, Axiom home, health |
| `axiom uninstall` | Removes Axiom binary + `~/.axiom` only |

Supported stacks (detection / run, MVP and later):

- Node / npm / Electron / Vite / React  
- Python (`requirements.txt`, `pyproject.toml`)  
- Rust (`Cargo.toml`)  
- ZIP archives of the above  

---

## GitHub Releases (maintainers)

Release assets must use these **exact** names (produced by `.github/workflows/release.yml`):

```text
axiom-macos-aarch64.tar.gz
axiom-linux-x86_64.tar.gz
axiom-windows-x64.zip
```

Tag and push:

```bash
git tag v0.1.1
git push origin v0.1.1
```

Or run the **Release** workflow with a tag input.

---

## Project layout after install

```text
~/.axiom/
├── bin/axiom          # the CLI binary
├── toolchains/
├── packages/
├── cache/
├── projects/
└── tmp/
```

---

## License

MIT. See [LICENSE](LICENSE).
