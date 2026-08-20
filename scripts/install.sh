#!/bin/sh
# Axiom installer
# Usage: curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install.sh | sh
#
# Installs a prebuilt Axiom binary. Does NOT require Rust, Cargo, Node, or any toolchain.
# Supports macOS (Apple Silicon + Intel) and Linux (x86_64 + aarch64).

set -e

# ---------------------------------------------------------------------------
# Configuration — change these when the public repo / releases exist
# ---------------------------------------------------------------------------
REPO="${AXIOM_REPO:-axiom-dev/axiom}"
# Official install can override with: AXIOM_VERSION=v0.1.0
VERSION="${AXIOM_VERSION:-latest}"
INSTALL_DIR="${AXIOM_INSTALL_DIR:-}"
# ---------------------------------------------------------------------------

RED=''
GREEN=''
YELLOW=''
BOLD=''
RESET=''
if [ -t 1 ]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  BOLD='\033[1m'
  RESET='\033[0m'
fi

info()  { printf "${BOLD}==>${RESET} %s\n" "$*"; }
ok()    { printf "${GREEN}✓${RESET} %s\n" "$*"; }
warn()  { printf "${YELLOW}!${RESET} %s\n" "$*"; }
err()   { printf "${RED}error:${RESET} %s\n" "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Detect OS + architecture
# ---------------------------------------------------------------------------
detect_platform() {
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH="$(uname -m)"

  case "$OS" in
    darwin)  OS_NAME="macos" ;;
    linux)   OS_NAME="linux" ;;
    *)       err "Unsupported OS: $OS. Axiom currently supports macOS and Linux." ;;
  esac

  case "$ARCH" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    arm64|aarch64)  ARCH_NAME="aarch64" ;;
    *)              err "Unsupported architecture: $ARCH" ;;
  esac

  # Asset naming convention used by GitHub Releases
  # Examples: axiom-macos-aarch64, axiom-linux-x86_64
  ASSET_NAME="axiom-${OS_NAME}-${ARCH_NAME}"
  BINARY_NAME="axiom"
}

# ---------------------------------------------------------------------------
# Resolve install directory (user-owned, no sudo)
# ---------------------------------------------------------------------------
resolve_install_dir() {
  if [ -n "$INSTALL_DIR" ]; then
    return
  fi

  # Prefer ~/.axiom/bin (keeps everything under Axiom's home)
  if [ -n "$HOME" ]; then
    INSTALL_DIR="$HOME/.axiom/bin"
  else
    err "HOME is not set; cannot determine install location"
  fi
}

# ---------------------------------------------------------------------------
# Download helper (curl preferred, wget fallback)
# ---------------------------------------------------------------------------
download() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    # --connect-timeout and --max-time prevent hangs on missing hosts/releases
    curl -fsSL --connect-timeout 10 --max-time 60 --retry 2 --retry-delay 1 \
      -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q --timeout=30 -O "$dest" "$url"
  else
    err "Need curl or wget to download Axiom"
  fi
}

# ---------------------------------------------------------------------------
# Resolve the download URL from GitHub Releases
# ---------------------------------------------------------------------------
resolve_download_url() {
  if [ -n "${AXIOM_BINARY_URL:-}" ]; then
    # Explicit override (useful for testing / private mirrors)
    DOWNLOAD_URL="$AXIOM_BINARY_URL"
    return
  fi

  if [ "$VERSION" = "latest" ]; then
    # Use the /releases/latest redirect — GitHub serves the asset list
    # We construct the conventional asset URL. If the release does not
    # exist yet this will 404 and we give a clear message.
    API_URL="https://api.github.com/repos/${REPO}/releases/latest"
    # Try to discover the real asset name via API if possible
    if command -v curl >/dev/null 2>&1; then
      ASSET_URL=$(curl -fsSL --connect-timeout 5 --max-time 15 "$API_URL" 2>/dev/null \
        | grep -o "\"browser_download_url\": \"[^\"]*${ASSET_NAME}[^\"]*\"" \
        | head -1 \
        | sed 's/.*"browser_download_url": "\([^"]*\)".*/\1/') || true
      if [ -n "$ASSET_URL" ]; then
        DOWNLOAD_URL="$ASSET_URL"
        return
      fi
    fi
    # Fallback to conventional path (works once a release with that asset exists)
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"
  else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"
  fi
}

# ---------------------------------------------------------------------------
# Main install
# ---------------------------------------------------------------------------
main() {
  printf "\n${BOLD}AXIOM installer${RESET}\n\n"

  detect_platform
  resolve_install_dir
  resolve_download_url

  info "Platform:  ${OS_NAME} / ${ARCH_NAME}"
  info "Install to: ${INSTALL_DIR}"
  info "Binary:     ${ASSET_NAME}"
  echo

  # Create install directory
  mkdir -p "$INSTALL_DIR" || err "Cannot create $INSTALL_DIR"

  TMPDIR_INSTALL="${TMPDIR:-/tmp}"
  TMP_BIN="${TMPDIR_INSTALL}/axiom-install-$$"
  trap 'rm -f "$TMP_BIN"' EXIT

  info "Downloading..."
  if ! download "$DOWNLOAD_URL" "$TMP_BIN" 2>/dev/null; then
    echo
    err "Could not download Axiom binary.

  URL tried: ${DOWNLOAD_URL}

  This usually means:
  1. The GitHub repository or release does not exist yet, or
  2. The asset name does not match (expected: ${ASSET_NAME}).

  What to do:
  • If you are the project maintainer, create a GitHub Release and
    upload assets named exactly:
      axiom-macos-aarch64
      axiom-macos-x86_64
      axiom-linux-x86_64
      axiom-linux-aarch64
  • Or set AXIOM_BINARY_URL to a direct URL of a prebuilt binary
    and re-run this script.
  • Developers building from source should use: cargo build --release
"
  fi

  # Make executable
  chmod +x "$TMP_BIN"

  # Basic sanity check — must look like an executable
  if [ ! -s "$TMP_BIN" ]; then
    err "Downloaded file is empty"
  fi

  # Move into place (atomic-ish)
  TARGET="${INSTALL_DIR}/${BINARY_NAME}"
  mv -f "$TMP_BIN" "$TARGET"
  chmod 755 "$TARGET"
  ok "Installed ${TARGET}"

  # Verify it runs
  info "Verifying..."
  if ! "$TARGET" --version >/dev/null 2>&1; then
    # Some builds may only support -V; try both
    if ! "$TARGET" -V >/dev/null 2>&1; then
      warn "Binary installed but --version failed. It may still work."
    else
      VER=$("$TARGET" -V 2>/dev/null || true)
      ok "Verified: ${VER}"
    fi
  else
    VER=$("$TARGET" --version 2>/dev/null || true)
    ok "Verified: ${VER}"
  fi

  # PATH setup
  echo
  NEED_PATH=1
  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) NEED_PATH=0 ;;
  esac

  if [ "$NEED_PATH" -eq 0 ]; then
    ok "${INSTALL_DIR} is already on your PATH"
  else
    info "Adding ${INSTALL_DIR} to PATH"

    # Detect shell profile
    PROFILE=""
    if [ -n "${ZSH_VERSION:-}" ] || [ "$(basename "${SHELL:-}")" = "zsh" ]; then
      PROFILE="$HOME/.zshrc"
    elif [ -n "${BASH_VERSION:-}" ] || [ "$(basename "${SHELL:-}")" = "bash" ]; then
      if [ "$(uname -s)" = "Darwin" ]; then
        PROFILE="$HOME/.bash_profile"
      else
        PROFILE="$HOME/.bashrc"
      fi
    else
      # Fallback
      if [ -f "$HOME/.zshrc" ]; then
        PROFILE="$HOME/.zshrc"
      elif [ -f "$HOME/.bashrc" ]; then
        PROFILE="$HOME/.bashrc"
      elif [ -f "$HOME/.profile" ]; then
        PROFILE="$HOME/.profile"
      fi
    fi

    MARKER="# Axiom CLI"
    PATH_LINE="export PATH=\"${INSTALL_DIR}:\$PATH\""

    if [ -n "$PROFILE" ]; then
      if [ -f "$PROFILE" ] && grep -q "$MARKER" "$PROFILE" 2>/dev/null; then
        ok "PATH entry already present in ${PROFILE}"
      else
        {
          echo ""
          echo "$MARKER"
          echo "$PATH_LINE"
        } >> "$PROFILE"
        ok "Added PATH entry to ${PROFILE}"
        warn "Restart your terminal or run:  source ${PROFILE}"
      fi
    else
      warn "Could not detect shell profile. Add this to your shell config:"
      echo "  ${PATH_LINE}"
    fi
  fi

  # Create the rest of ~/.axiom structure
  mkdir -p "$HOME/.axiom/toolchains" "$HOME/.axiom/packages" \
           "$HOME/.axiom/cache" "$HOME/.axiom/projects" "$HOME/.axiom/tmp" 2>/dev/null || true

  echo
  printf "${GREEN}${BOLD}Axiom is installed.${RESET}\n\n"
  echo "  Binary:  ${TARGET}"
  echo "  Data:    ${HOME}/.axiom"
  echo
  echo "Try it:"
  echo "  axiom --version"
  echo "  axiom doctor"
  echo "  axiom new hello && cd hello && axiom run"
  echo
  echo "To uninstall later:"
  echo "  axiom uninstall --yes"
  echo
}

main "$@"
