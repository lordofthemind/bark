#!/usr/bin/env bash
# bark installer
# Usage: curl -fsSL https://raw.githubusercontent.com/lordofthemind/bark/master/install.sh | bash
#
# Installs the pre-built bark binary for your platform.
# Respects BARK_INSTALL_DIR env var (default: ~/.local/bin).

set -euo pipefail

REPO="lordofthemind/bark"
BIN="bark"
INSTALL_DIR="${BARK_INSTALL_DIR:-$HOME/.local/bin}"

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BOLD='\033[1m'; RESET='\033[0m'

info()  { printf "${GREEN}[bark]${RESET} %s\n" "$*"; }
warn()  { printf "${YELLOW}[bark]${RESET} %s\n" "$*"; }
error() { printf "${RED}[bark] error:${RESET} %s\n" "$*" >&2; exit 1; }

# ── Detect platform ───────────────────────────────────────────────────────────
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux)  os="linux" ;;
        Darwin) os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *) error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# ── Check for required tools ──────────────────────────────────────────────────
need() {
    command -v "$1" >/dev/null 2>&1 || error "Required tool not found: $1"
}
need curl
need tar

# ── Main install ──────────────────────────────────────────────────────────────
main() {
    local platform
    platform="$(detect_platform)"

    # Show current version if already installed
    if command -v "$BIN" >/dev/null 2>&1; then
        local current
        current="$($BIN --version 2>/dev/null | head -1 || true)"
        warn "Found existing installation: ${current}"
    fi

    info "Fetching latest release for ${BOLD}${platform}${RESET} …"

    # Get latest release tag from GitHub API
    local latest_tag
    latest_tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": *"\(.*\)".*/\1/')"

    if [[ -z "$latest_tag" ]]; then
        error "Could not determine latest release tag. Check your internet connection."
    fi

    info "Latest version: ${BOLD}${latest_tag}${RESET}"

    # Build download URL
    local ext="tar.gz"
    [[ "$platform" == windows-* ]] && ext="zip"
    local filename="${BIN}-${platform}.${ext}"
    local url="https://github.com/${REPO}/releases/download/${latest_tag}/${filename}"

    # Download to temp dir
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    info "Downloading ${url} …"
    curl -fsSL --progress-bar "$url" -o "${tmp_dir}/${filename}" \
        || error "Download failed. Does release ${latest_tag} include a ${platform} binary?"

    # Extract
    info "Extracting …"
    if [[ "$ext" == "tar.gz" ]]; then
        tar -xzf "${tmp_dir}/${filename}" -C "$tmp_dir"
    else
        # zip (Windows)
        command -v unzip >/dev/null 2>&1 || error "unzip is required on Windows"
        unzip -q "${tmp_dir}/${filename}" -d "$tmp_dir"
    fi

    local binary
    binary="$(find "$tmp_dir" -name "$BIN" -o -name "${BIN}.exe" | head -1)"
    [[ -z "$binary" ]] && error "Could not find bark binary in the downloaded archive"

    # Install
    mkdir -p "$INSTALL_DIR"
    chmod +x "$binary"
    mv "$binary" "${INSTALL_DIR}/${BIN}"

    info "Installed to ${BOLD}${INSTALL_DIR}/${BIN}${RESET}"

    # Verify
    if ! "${INSTALL_DIR}/${BIN}" --version >/dev/null 2>&1; then
        error "Installation verification failed"
    fi
    local installed_ver
    installed_ver="$("${INSTALL_DIR}/${BIN}" --version)"
    info "Verified: ${installed_ver}"

    # PATH check
    if ! command -v "$BIN" >/dev/null 2>&1; then
        warn "${INSTALL_DIR} is not in your PATH."
        warn "Add this to your shell config:"
        warn ""
        warn "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        warn ""
    fi

    # Next steps
    printf "\n${BOLD}Next steps:${RESET}\n"
    printf "  ${GREEN}bark init${RESET}    — create a .bark.toml config in your project\n"
    printf "  ${GREEN}bark${RESET}         — tag all files in the current directory\n"
    printf "  ${GREEN}bark --help${RESET}  — full usage\n\n"
}

main "$@"
