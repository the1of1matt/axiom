#!/bin/sh
# Axiom installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/the1of1matt/axiom/main/scripts/install.sh | sh
#
# Installs a prebuilt Axiom binary. Does NOT require Rust, Cargo, Node, npm,
# Python, Go, Homebrew, or any other development toolchain.
# Supports macOS (Apple Silicon + Intel) and Linux (x86_64 + aarch64).

set -e

REPO="${AXIOM_REPO:-the1of1matt/axiom}"
VERSION="${AXIOM_VERSION:-latest}"
INSTALL_DIR="${AXIOM_INSTALL_DIR:-}"

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

detect_platform() {
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH="$(uname -m)"

  case "$OS" in
    darwin)  OS_NAME="macos" ;;
    linux)   OS_NAME="linux" ;;
    *)       err "Unsupported OS: $OS. Axiom currently supports macOS and Linux.\nWindows users: download axiom-windows-x64.zip from GitHub Releases." ;;
  esac

  case "$ARCH" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    arm64|aarch64)  ARCH_NAME="aarch64" ;;
    *)              err "Unsupported architecture: $ARCH" ;;
  esac

  # Must match GitHub Release asset names produced by release.yml
  ASSET_BASE="axiom-${OS_NAME}-${ARCH_NAME}"
  ASSET_ARCHIVE="${ASSET_BASE}.tar.gz"
  BINARY_NAME="axiom"
}

resolve_install_dir() {
  if [ -n "$INSTALL_DIR" ]; then
    return
  fi
  if [ -z "$HOME" ]; then
    err "HOME is not set; cannot determine install location"
  fi

  # Prefer ~/.local/bin when it is already on PATH (common on Linux; some Mac setups).
  # Always keep a canonical copy under ~/.axiom/bin as well.
  AXIOM_BIN_DIR="$HOME/.axiom/bin"
  LOCAL_BIN_DIR="$HOME/.local/bin"

  case ":$PATH:" in
    *":${LOCAL_BIN_DIR}:"*)
      INSTALL_DIR="$LOCAL_BIN_DIR"
      ;;
    *)
      INSTALL_DIR="$AXIOM_BIN_DIR"
      ;;
  esac
}

download() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --connect-timeout 15 --max-time 120 --retry 3 --retry-delay 1 \
      -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q --timeout=60 -O "$dest" "$url"
  else
    err "Need curl or wget to download Axiom"
  fi
}

resolve_download_url() {
  if [ -n "${AXIOM_BINARY_URL:-}" ]; then
    DOWNLOAD_URL="$AXIOM_BINARY_URL"
    return
  fi

  if [ "$VERSION" = "latest" ]; then
    API_URL="https://api.github.com/repos/${REPO}/releases/latest"
    if command -v curl >/dev/null 2>&1; then
      # Prefer browser_download_url that matches our archive name exactly
      ASSET_URL=$(curl -fsSL --connect-timeout 10 --max-time 20 "$API_URL" 2>/dev/null \
        | grep -o "\"browser_download_url\": \"[^\"]*${ASSET_ARCHIVE}\"" \
        | head -1 \
        | sed 's/.*"browser_download_url": "\([^"]*\)".*/\1/') || true
      if [ -n "$ASSET_URL" ]; then
        DOWNLOAD_URL="$ASSET_URL"
        return
      fi
    fi
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_ARCHIVE}"
  else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_ARCHIVE}"
  fi
}

# Append PATH export to a profile file once (idempotent via marker).
append_path_line() {
  profile="$1"
  marker="# Axiom CLI"
  path_line="export PATH=\"${AXIOM_BIN_DIR}:\$PATH\""

  # Ensure parent exists; create empty profile if needed
  if [ ! -f "$profile" ]; then
    # Only create well-known profiles, not arbitrary paths
    touch "$profile" 2>/dev/null || return 1
  fi

  if grep -q "$marker" "$profile" 2>/dev/null; then
    return 0
  fi

  {
    echo ""
    echo "$marker"
    echo "$path_line"
  } >> "$profile"
  return 0
}

setup_path() {
  AXIOM_BIN_DIR="$HOME/.axiom/bin"

  NEED_PATH=1
  case ":$PATH:" in
    *":${AXIOM_BIN_DIR}:"*) NEED_PATH=0 ;;
  esac
  # Also OK if the binary is already findable (e.g. installed to ~/.local/bin on PATH)
  if command -v axiom >/dev/null 2>&1; then
    NEED_PATH=0
  fi

  if [ "$NEED_PATH" -eq 0 ]; then
    ok "axiom is on your PATH"
    return
  fi

  info "Configuring PATH so 'axiom' works in new terminals"

  # macOS Terminal/iTerm often start login shells → .zprofile;
  # interactive zsh also reads .zshrc. Write both.
  # Linux bash: .bashrc / .profile. Cover the common set so a new terminal works.
  UPDATED=""
  for profile in \
    "$HOME/.zprofile" \
    "$HOME/.zshrc" \
    "$HOME/.bash_profile" \
    "$HOME/.bashrc" \
    "$HOME/.profile"
  do
    # On macOS default shell is zsh — always ensure zsh profiles exist.
    # For bash-only files, only write if the file already exists (except .profile).
    case "$profile" in
      */.zprofile|*/.zshrc)
        if append_path_line "$profile"; then
          UPDATED="${UPDATED} $(basename "$profile")"
        fi
        ;;
      */.profile)
        if append_path_line "$profile"; then
          UPDATED="${UPDATED} $(basename "$profile")"
        fi
        ;;
      *)
        if [ -f "$profile" ]; then
          if append_path_line "$profile"; then
            UPDATED="${UPDATED} $(basename "$profile")"
          fi
        fi
        ;;
    esac
  done

  if [ -n "$UPDATED" ]; then
    ok "PATH updated in:${UPDATED}"
  else
    warn "Could not update shell profiles automatically."
    echo "  Add this line to your shell config:"
    echo "    export PATH=\"${AXIOM_BIN_DIR}:\$PATH\""
  fi

  # Make the current shell session work when the installer is sourced,
  # and print a one-liner for curl|sh users in the same terminal.
  export PATH="${AXIOM_BIN_DIR}:$PATH"
}

extract_binary() {
  archive="$1"
  out_dir="$2"

  mkdir -p "$out_dir"

  # Archive from release.yml contains ./axiom (and README.txt)
  if command -v tar >/dev/null 2>&1; then
    # Extract only the binary member if present; fall back to full extract
    if tar -tzf "$archive" 2>/dev/null | grep -qE '(^|/)axiom$'; then
      tar -xzf "$archive" -C "$out_dir"
    else
      tar -xzf "$archive" -C "$out_dir"
    fi
  else
    err "tar is required to extract the Axiom release archive"
  fi

  # Locate extracted binary (may be nested if archive layout differs)
  if [ -f "$out_dir/axiom" ]; then
    EXTRACTED="$out_dir/axiom"
  else
    EXTRACTED=$(find "$out_dir" -type f -name axiom 2>/dev/null | head -1)
  fi

  if [ -z "$EXTRACTED" ] || [ ! -f "$EXTRACTED" ]; then
    err "Archive did not contain an 'axiom' binary"
  fi

  chmod 755 "$EXTRACTED"
  echo "$EXTRACTED"
}

main() {
  printf "\n${BOLD}AXIOM installer${RESET}\n\n"

  detect_platform
  resolve_install_dir
  resolve_download_url

  AXIOM_BIN_DIR="$HOME/.axiom/bin"
  mkdir -p "$AXIOM_BIN_DIR" "$INSTALL_DIR" || err "Cannot create install directories"

  info "Platform:   ${OS_NAME} / ${ARCH_NAME}"
  info "Repository: ${REPO}"
  info "Version:    ${VERSION}"
  info "Asset:      ${ASSET_ARCHIVE}"
  info "Install to: ${AXIOM_BIN_DIR}/axiom"
  echo

  TMP_ROOT="${TMPDIR:-/tmp}/axiom-install-$$"
  mkdir -p "$TMP_ROOT"
  trap 'rm -rf "$TMP_ROOT"' EXIT

  ARCHIVE_PATH="${TMP_ROOT}/${ASSET_ARCHIVE}"

  info "Downloading ${DOWNLOAD_URL}"
  if ! download "$DOWNLOAD_URL" "$ARCHIVE_PATH"; then
    echo
    err "Could not download Axiom.

  URL tried: ${DOWNLOAD_URL}

  Check that a GitHub Release exists on ${REPO} with asset:
    ${ASSET_ARCHIVE}

  Or set AXIOM_BINARY_URL to a direct archive/binary URL and re-run.
"
  fi

  if [ ! -s "$ARCHIVE_PATH" ]; then
    err "Downloaded file is empty"
  fi

  info "Extracting..."
  EXTRACT_DIR="${TMP_ROOT}/extract"
  EXTRACTED=$(extract_binary "$ARCHIVE_PATH" "$EXTRACT_DIR")

  # Install canonical binary under ~/.axiom/bin
  TARGET="${AXIOM_BIN_DIR}/axiom"
  cp -f "$EXTRACTED" "$TARGET"
  chmod 755 "$TARGET"
  ok "Installed ${TARGET}"

  # If we chose ~/.local/bin (already on PATH), place a copy/symlink there too
  if [ "$INSTALL_DIR" != "$AXIOM_BIN_DIR" ]; then
    mkdir -p "$INSTALL_DIR"
    cp -f "$TARGET" "${INSTALL_DIR}/axiom"
    chmod 755 "${INSTALL_DIR}/axiom"
    ok "Also installed ${INSTALL_DIR}/axiom"
  fi

  info "Verifying binary..."
  if "$TARGET" --version >/dev/null 2>&1; then
    VER=$("$TARGET" --version 2>/dev/null || true)
    ok "Verified: ${VER}"
  elif "$TARGET" -V >/dev/null 2>&1; then
    VER=$("$TARGET" -V 2>/dev/null || true)
    ok "Verified: ${VER}"
  else
    warn "Binary installed but --version failed. It may still work."
  fi

  setup_path

  mkdir -p "$HOME/.axiom/toolchains" "$HOME/.axiom/packages" \
           "$HOME/.axiom/cache" "$HOME/.axiom/projects" "$HOME/.axiom/tmp" 2>/dev/null || true

  echo
  # Final check with PATH including our bin dir
  PATH="${AXIOM_BIN_DIR}:$PATH"
  export PATH
  if command -v axiom >/dev/null 2>&1; then
    ok "command -v axiom → $(command -v axiom)"
  else
    warn "axiom is not yet visible in this shell's PATH"
  fi

  printf "\n${GREEN}${BOLD}Axiom is installed.${RESET}\n\n"
  echo "  Binary:  ${TARGET}"
  echo "  Data:    ${HOME}/.axiom"
  echo
  echo "Open a new terminal window, then run:"
  echo "  axiom --version"
  echo "  axiom doctor"
  echo "  axiom run ~/path/to/project"
  echo
  echo "In THIS terminal, run:"
  echo "  export PATH=\"${AXIOM_BIN_DIR}:\$PATH\""
  echo "  axiom --version"
  echo
  echo "To uninstall later:"
  echo "  axiom uninstall --yes"
  echo
}

main "$@"
