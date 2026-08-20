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
```

---

## Install (end users)

**You do not need Rust, Cargo, Node, npm, Python, Go, or any development toolchain.**

### One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/axiom-dev/axiom/main/scripts/install.sh | sh
```

Then open a **new terminal** (or `source` your shell profile) and run:

```bash
axiom --version
axiom doctor
```

The installer will:

1. Detect your OS (macOS / Linux) and CPU (Apple Silicon / Intel / x86_64 / aarch64)
2. Download the matching prebuilt binary from GitHub Releases
3. Install it to `~/.axiom/bin/axiom` (user-owned, no `sudo`)
4. Add that directory to your PATH (via `~/.zshrc` or `~/.bashrc`)
5. Verify the binary runs
6. Print exactly what it did

### Uninstall

```bash
axiom uninstall --yes
```

This removes **only** Axiom’s own binary and `~/.axiom` data.  
It never touches your projects or unrelated software.

### Manual install (if you prefer)

1. Go to [Releases](https://github.com/axiom-dev/axiom/releases)
2. Download the asset that matches your machine, e.g.:
   - `axiom-macos-aarch64` (Apple Silicon)
   - `axiom-macos-x86_64` (Intel Mac)
   - `axiom-linux-x86_64`
   - `axiom-linux-aarch64`
3. `chmod +x axiom-*` and move it to a directory on your PATH (or to `~/.axiom/bin/`)

---

## Build from source (developers only)

Only needed if you are **developing Axiom itself**.

```bash
git clone https://github.com/axiom-dev/axiom.git
cd axiom
cargo build --release
# binary: target/release/axiom
```

Requirements for developers: a recent Rust toolchain (`rustup`).

End users should **never** be told to run `cargo build`.

---

## Current status (MVP 0.1)

| Command        | What it does                                              |
|----------------|-----------------------------------------------------------|
| `axiom new`    | Creates a minimal multi-runtime starter                   |
| `axiom find`   | Safely scans common directories for matching projects     |
| `axiom run`    | Detects project type, checks toolchain, runs safe entry   |
| `axiom doctor` | Reports OS, arch, toolchains, Axiom home, project health  |
| `axiom uninstall` | Removes Axiom binary + `~/.axiom` only                 |

Supported detection markers (MVP):

- `Cargo.toml` → Rust  
- `package.json` (+ React / Electron / Vite signals)  
- `pyproject.toml` / `requirements.txt` / `main.py` → Python  
- `CMakeLists.txt` → CMake (detection only)  
- Tauri / Electron markers (detection only)

**Security notes**

- Discovery never executes project code.
- `axiom run` never automatically runs `npm install` or arbitrary scripts.
- Only well-known safe entry points are invoked.
- ZIP support is planned and will be hardened against path traversal.

---

## GitHub Releases setup (maintainers)

The install script expects assets on a GitHub Release with these **exact** names:

```
axiom-macos-aarch64
axiom-macos-x86_64
axiom-linux-x86_64
axiom-linux-aarch64
```

### First-time release checklist

1. Create the public repository (e.g. `axiom-dev/axiom`) and push this code.
2. Update `REPO` in `scripts/install.sh` if the owner/name differs.
3. Create a tag and release:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. Build binaries (or let GitHub Actions do it — see `.github/workflows/release.yml`):

   ```bash
   # On each platform / via cross:
   cargo build --release
   strip target/release/axiom          # Unix
   # Rename:
   cp target/release/axiom axiom-macos-aarch64   # etc.
   ```

5. Upload the four binaries as release assets (no extension, or keep the name exact).
6. Publish the release.
7. Users can then run:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/axiom-dev/axiom/main/scripts/install.sh | sh
   ```

Until the repository and first release exist, the public `curl | sh` URL will 404.  
You can still test locally:

```bash
AXIOM_BINARY_URL=file:///path/to/axiom-linux-x86_64 sh scripts/install.sh
```

---

## Project layout after install

```
~/.axiom/
├── bin/axiom          # the CLI binary
├── toolchains/        # future isolated toolchains
├── packages/
├── cache/
├── projects/
└── tmp/
```

Axiom never modifies your global Node / Rust / Python installations.

---

## Quick start (after install)

```bash
axiom new hello
cd hello
axiom run
axiom doctor
axiom find hello
```

---

## Tests (developers)

```bash
cargo test
```

---

## Roadmap

See [ROADMAP.md](ROADMAP.md).

---

## License

MIT. See [LICENSE](LICENSE).

## Contributing

Prefer small, boring, reliable changes. Keep the command surface tiny  
(`new`, `find`, `run`, `doctor`, `uninstall`). New capabilities should almost  
always be internal operations of those commands.


## Windows

### Users (release binary)

1. Download `axiom-windows-x64.zip` from [GitHub Releases](https://github.com/OWNER/axiom/releases).
2. Extract somewhere permanent (e.g. `%LOCALAPPDATA%\Axiom`).
3. Add that folder to your user PATH, or run `axiom.exe` by full path.

```powershell
# Example after extract
.\axiom.exe --version
.\axiom.exe run .\project.zip
```

### Developers (build from source on Windows)

```powershell
# Requires Rust MSVC: https://rustup.rs
cargo build --release
.\target\release\axiom.exe run .\project.zip
```

Or use the helper script:

```powershell
.\scripts\build-windows.ps1
```

### CI

Push a tag `v*` or run the **Release** workflow. The `windows-latest` job builds
`x86_64-pc-windows-msvc` and uploads `axiom-windows-x64.zip`.
