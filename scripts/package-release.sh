#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-}"
if [[ -z "${TARGET}" ]]; then
    echo "Usage: $0 <target-triple> [tag-or-version]" >&2
    exit 1
fi

VERSION="${2:-${GITHUB_REF_NAME:-}}"
if [[ -z "${VERSION}" ]]; then
    VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f2)
    VERSION="v${VERSION}"
fi

BIN_NAME="rust_lldb_mcp"
SRC_BIN="target/${TARGET}/release/${BIN_NAME}"
if [[ ! -f "${SRC_BIN}" ]]; then
    SRC_BIN="target/release/${BIN_NAME}"
fi

if [[ ! -f "${SRC_BIN}" ]]; then
    echo "Error: Binary not found at target/${TARGET}/release/${BIN_NAME} or target/release/${BIN_NAME}" >&2
    exit 1
fi

DIST_DIR="dist"
ARCHIVE_NAME="${BIN_NAME}-${VERSION}-${TARGET}"
STAGE_DIR="target/package-stage/${ARCHIVE_NAME}"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"
mkdir -p "${DIST_DIR}"

cp "${SRC_BIN}" "${STAGE_DIR}/"
if [[ -f "README.md" ]]; then
    cp README.md "${STAGE_DIR}/"
fi
if [[ -f "LICENSE" ]]; then
    cp LICENSE "${STAGE_DIR}/"
fi

tar -czf "${DIST_DIR}/${ARCHIVE_NAME}.tar.gz" -C "target/package-stage" "${ARCHIVE_NAME}"
rm -rf "${STAGE_DIR}"

cd "${DIST_DIR}"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${ARCHIVE_NAME}.tar.gz" > "${ARCHIVE_NAME}.tar.gz.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${ARCHIVE_NAME}.tar.gz" > "${ARCHIVE_NAME}.tar.gz.sha256"
fi

echo "Packaged ${DIST_DIR}/${ARCHIVE_NAME}.tar.gz and checksum successfully."
