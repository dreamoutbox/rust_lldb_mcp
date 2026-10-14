#!/usr/bin/env bash
set -euo pipefail

REPO="dreamoutbox/rust_lldb_mcp"
BIN_NAME="rust_lldb_mcp"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"

# Detect operating system
OS="$(uname -s)"
case "${OS}" in
    Linux)
        OS_TYPE="unknown-linux-gnu"
        ;;
    Darwin)
        OS_TYPE="apple-darwin"
        ;;
    *)
        echo "Error: Unsupported operating system '${OS}'." >&2
        exit 1
        ;;
esac

# Detect CPU architecture
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64|amd64)
        ARCH_TYPE="x86_64"
        ;;
    arm64|aarch64)
        ARCH_TYPE="aarch64"
        ;;
    *)
        echo "Error: Unsupported architecture '${ARCH}'." >&2
        exit 1
        ;;
esac

TARGET="${ARCH_TYPE}-${OS_TYPE}"

# Determine version
VERSION="${1:-${VERSION:-}}"
if [[ -z "${VERSION}" ]]; then
    # Try resolving via GitHub redirect first (avoids GitHub API rate limits)
    RESOLVED_URL=$(curl -sIL -o /dev/null -w "%{url_effective}" "https://github.com/${REPO}/releases/latest" 2>/dev/null || true)
    if [[ "${RESOLVED_URL}" =~ /releases/tag/([^/]+)$ ]]; then
        VERSION="${BASH_REMATCH[1]}"
    else
        # Fallback to GitHub API
        VERSION=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n 1 | cut -d '"' -f 4 || true)
    fi
fi

if [[ -z "${VERSION}" ]]; then
    echo "Error: Unable to determine latest release version for ${REPO}." >&2
    echo "Please specify a version explicitly: $0 <version>" >&2
    exit 1
fi

ARCHIVE_NAME="${BIN_NAME}-${VERSION}-${TARGET}"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}.tar.gz"

echo "Installing ${BIN_NAME} ${VERSION} for ${TARGET}..."
echo "Downloading from: ${DOWNLOAD_URL}"

TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

if ! curl --proto '=https' --tlsv1.2 -fsSL "${DOWNLOAD_URL}" -o "${TMP_DIR}/${ARCHIVE_NAME}.tar.gz"; then
    echo "Error: Failed to download release archive." >&2
    echo "Verify that a release exists for target '${TARGET}' at:" >&2
    echo "  https://github.com/${REPO}/releases/tag/${VERSION}" >&2
    exit 1
fi

tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}.tar.gz" -C "${TMP_DIR}"

# Locate extracted binary
BIN_PATH=$(find "${TMP_DIR}" -type f -name "${BIN_NAME}" | head -n 1)
if [[ -z "${BIN_PATH}" || ! -f "${BIN_PATH}" ]]; then
    echo "Error: Could not locate '${BIN_NAME}' inside the downloaded archive." >&2
    exit 1
fi

mkdir -p "${INSTALL_DIR}"
install -m 755 "${BIN_PATH}" "${INSTALL_DIR}/${BIN_NAME}"

echo "Successfully installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
        ;;
    *)
        echo ""
        echo "Notice: '${INSTALL_DIR}' is not in your PATH."
        echo "Add it to your shell configuration:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
