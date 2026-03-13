#!/bin/sh
set -eu

# Installer for mcp-tester and mcp-preview binaries from GitHub releases.
# Downloads the correct binary for your platform, verifies its SHA256
# checksum, and installs it to a local bin directory.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.sh | sh
#   curl -fsSL ... | sh -s -- --tool mcp-preview --version v1.18.0

REPO="paiml/rust-mcp-sdk"
DEFAULT_TOOL="mcp-tester"
DEFAULT_INSTALL_DIR="$HOME/.local/bin"

usage() {
    cat <<EOF
Install mcp-tester or mcp-preview from GitHub releases.

Usage: install.sh [OPTIONS]

Options:
  --tool TOOL        Tool to install: mcp-tester or mcp-preview (default: mcp-tester)
  --version VERSION  Version tag to install, e.g. v1.18.0 (default: latest)
  --dir DIR          Install directory (default: ~/.local/bin)
  --help             Show this help
EOF
}

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "unknown-linux-gnu" ;;
        Darwin*) echo "apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*) printf "Error: Use install.ps1 for Windows.\n" >&2; exit 1 ;;
        *)       printf "Error: Unsupported OS: %s\n" "$(uname -s)" >&2; exit 1 ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             printf "Error: Unsupported architecture: %s\n" "$(uname -m)" >&2; exit 1 ;;
    esac
}

verify_checksum() {
    checksum_file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum --check "$checksum_file" --quiet
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 --check "$checksum_file" --quiet
    else
        printf "Warning: No sha256sum or shasum found, skipping checksum verification\n" >&2
        return 0
    fi
}

# --- Parse arguments ---
TOOL="$DEFAULT_TOOL"
VERSION="latest"
INSTALL_DIR="$DEFAULT_INSTALL_DIR"

while [ $# -gt 0 ]; do
    case "$1" in
        --tool)
            TOOL="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            printf "Error: Unknown option: %s\n" "$1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

# Validate tool name
case "$TOOL" in
    mcp-tester|mcp-preview) ;;
    *) printf "Error: Unknown tool '%s'. Must be mcp-tester or mcp-preview.\n" "$TOOL" >&2; exit 1 ;;
esac

# --- Normalize version (auto-prepend v if needed) ---
if [ "$VERSION" != "latest" ]; then
    case "$VERSION" in
        v*) ;;
        *)  VERSION="v${VERSION}" ;;
    esac
fi

# --- Detect platform ---
ARCH=$(detect_arch)
OS=$(detect_os)
TARGET="${ARCH}-${OS}"
ASSET_NAME="${TOOL}-${TARGET}"

printf "Detected platform: %s\n" "$TARGET"

# --- Build download URL ---
if [ "$VERSION" = "latest" ]; then
    BASE_URL="https://github.com/${REPO}/releases/latest/download"
else
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
fi

DOWNLOAD_URL="${BASE_URL}/${ASSET_NAME}"
CHECKSUM_URL="${BASE_URL}/${ASSET_NAME}.sha256"

# --- Download to temp directory ---
TMPDIR_INSTALL=$(mktemp -d)
trap 'rm -rf "$TMPDIR_INSTALL"' EXIT

printf "Downloading %s...\n" "$ASSET_NAME"
curl -fsSL -o "${TMPDIR_INSTALL}/${ASSET_NAME}" "$DOWNLOAD_URL"

printf "Downloading checksum...\n"
curl -fsSL -o "${TMPDIR_INSTALL}/${ASSET_NAME}.sha256" "$CHECKSUM_URL"

# --- Verify checksum ---
printf "Verifying checksum...\n"
cd "$TMPDIR_INSTALL"
verify_checksum "${ASSET_NAME}.sha256"
printf "Checksum verified.\n"

# --- Install ---
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR_INSTALL}/${ASSET_NAME}" "${INSTALL_DIR}/${TOOL}"
chmod +x "${INSTALL_DIR}/${TOOL}"

printf "\n%s installed to %s/%s\n" "$TOOL" "$INSTALL_DIR" "$TOOL"

# --- Check PATH ---
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        printf "\nWarning: %s is not in your PATH.\n" "$INSTALL_DIR"
        printf "Add it with:\n"
        printf "  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
        ;;
esac
