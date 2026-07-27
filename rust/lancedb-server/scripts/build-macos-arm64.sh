#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
TARGET="aarch64-apple-darwin"
ARTIFACT_NAME="lancedb-server-macos-arm64"
DIST_DIR="${REPO_ROOT}/dist"
PACKAGE_DIR="${DIST_DIR}/${ARTIFACT_NAME}"
ARCHIVE_PATH="${DIST_DIR}/${ARTIFACT_NAME}.tar.gz"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: this script must run on macOS" >&2
  exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "error: this script requires an Apple Silicon (arm64) Mac" >&2
  exit 1
fi

for command in cargo rustup protoc; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "error: required command not found: ${command}" >&2
    exit 1
  fi
done

cd "${REPO_ROOT}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

rustup target add "${TARGET}"
cargo test --locked -p lancedb-server --target "${TARGET}"
cargo build --locked --release -p lancedb-server --target "${TARGET}"

BINARY_PATH="${REPO_ROOT}/target/${TARGET}/release/lancedb-server"
if [[ ! -f "${BINARY_PATH}" ]]; then
  echo "error: expected binary was not produced: ${BINARY_PATH}" >&2
  exit 1
fi

if ! file "${BINARY_PATH}" | grep -q "arm64"; then
  echo "error: built binary is not arm64" >&2
  file "${BINARY_PATH}" >&2
  exit 1
fi

mkdir -p "${PACKAGE_DIR}"
install -m 755 "${BINARY_PATH}" "${PACKAGE_DIR}/${ARTIFACT_NAME}"
install -m 644 \
  "${REPO_ROOT}/rust/lancedb-server/README.macos.md" \
  "${PACKAGE_DIR}/README.md"

(
  cd "${PACKAGE_DIR}"
  shasum -a 256 "${ARTIFACT_NAME}" > SHA256SUMS
)

tar -C "${DIST_DIR}" -czf "${ARCHIVE_PATH}" "${ARTIFACT_NAME}"
shasum -a 256 "${ARCHIVE_PATH}" > "${ARCHIVE_PATH}.sha256"

echo "Created Apple Silicon package:"
echo "  ${ARCHIVE_PATH}"
file "${PACKAGE_DIR}/${ARTIFACT_NAME}"
du -h "${PACKAGE_DIR}/${ARTIFACT_NAME}"
