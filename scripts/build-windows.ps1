# Build Axiom for Windows (run on Windows with Rust MSVC toolchain)
$ErrorActionPreference = "Stop"
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc

$bin = "target\x86_64-pc-windows-msvc\release\axiom.exe"
if (-not (Test-Path $bin)) { $bin = "target\release\axiom.exe" }
if (-not (Test-Path $bin)) { throw "axiom.exe not found" }

New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item $bin dist\axiom.exe -Force
@"
Axiom — run projects without toolchain hell

Usage:
  .\axiom.exe run path\to\project
  .\axiom.exe run project.zip
  .\axiom.exe doctor
"@ | Set-Content -Path dist\README.txt -Encoding utf8

Compress-Archive -Path dist\axiom.exe,dist\README.txt -DestinationPath dist\axiom-windows-x64.zip -Force
Write-Host "Created dist\axiom-windows-x64.zip"
