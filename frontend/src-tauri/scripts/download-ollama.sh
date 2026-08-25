#!/usr/bin/env bash
#
# Download a reviewed, version-pinned Ollama sidecar for Tauri.
#
# SECURITY: this script deliberately refuses mutable `latest` URLs and refuses
# to extract an archive without an independently obtained SHA-256 digest.
# Obtain OLLAMA_VERSION and OLLAMA_ARCHIVE_SHA256 from a reviewed upstream
# release/checksum source, record them in the release notes, then run:
#
#   OLLAMA_VERSION=<version> OLLAMA_ARCHIVE_SHA256=<sha256> \
#     ./download-ollama.sh [target-triple]
#
# Do not commit the resulting binary without recording its source version and
# checksum in a reviewed release manifest.

set -euo pipefail

: "${OLLAMA_VERSION:?Set a reviewed immutable Ollama release version}"
: "${OLLAMA_ARCHIVE_SHA256:?Set the reviewed SHA-256 for the exact archive}"

BINARIES_DIR="$(cd "$(dirname "$0")/../binaries" 2>/dev/null && pwd || echo "$(dirname "$0")/../binaries")"
mkdir -p "$BINARIES_DIR"

if [ "${1:-}" != "" ]; then
    TARGET="$1"
else
    ARCH="$(uname -m)"
    OS="$(uname -s)"
    case "$OS" in
        Darwin)
            case "$ARCH" in
                arm64) TARGET="aarch64-apple-darwin" ;;
                x86_64) TARGET="x86_64-apple-darwin" ;;
                *) echo "Unsupported arch: $ARCH" >&2; exit 1 ;;
            esac
            ;;
        Linux)
            case "$ARCH" in
                x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
                aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
                *) echo "Unsupported arch: $ARCH" >&2; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*|Windows_NT) TARGET="x86_64-pc-windows-msvc" ;;
        *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
    esac
fi

SUFFIX=""
case "$TARGET" in
    *windows*) SUFFIX=".exe" ;;
esac
OUT_FILE="$BINARIES_DIR/ollama-${TARGET}${SUFFIX}"

case "$TARGET" in
    *apple-darwin) ASSET="ollama-darwin.tgz"; ARCHIVE_TYPE="tgz" ;;
    x86_64-unknown-linux-gnu) ASSET="ollama-linux-amd64.tar.zst"; ARCHIVE_TYPE="zst" ;;
    aarch64-unknown-linux-gnu) ASSET="ollama-linux-arm64.tar.zst"; ARCHIVE_TYPE="zst" ;;
    x86_64-pc-windows-msvc) ASSET="ollama-windows-amd64.zip"; ARCHIVE_TYPE="zip" ;;
    *) echo "No Ollama asset mapping for target: $TARGET" >&2; exit 1 ;;
esac

ASSET_URL="https://github.com/ollama/ollama/releases/download/v${OLLAMA_VERSION}/${ASSET}"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
ARCHIVE_FILE="$TMPDIR/ollama-archive"

printf 'Downloading reviewed release v%s for %s\n' "$OLLAMA_VERSION" "$TARGET"
curl --fail --location --proto '=https' --tlsv1.2 --progress-bar "$ASSET_URL" -o "$ARCHIVE_FILE"

actual_sha="$(sha256sum "$ARCHIVE_FILE" | awk '{print $1}')"
if [ "$actual_sha" != "$OLLAMA_ARCHIVE_SHA256" ]; then
    echo "Checksum mismatch for $ASSET" >&2
    echo "expected: $OLLAMA_ARCHIVE_SHA256" >&2
    echo "actual:   $actual_sha" >&2
    exit 1
fi

case "$ARCHIVE_TYPE" in
    tgz) tar xzf "$ARCHIVE_FILE" -C "$TMPDIR" ;;
    zst)
        command -v zstd >/dev/null || { echo "zstd is required" >&2; exit 1; }
        zstd -d "$ARCHIVE_FILE" -o "$TMPDIR/ollama.tar" --quiet
        tar xf "$TMPDIR/ollama.tar" -C "$TMPDIR"
        ;;
    zip) unzip -q "$ARCHIVE_FILE" -d "$TMPDIR" ;;
esac

OLLAMA_BIN=""
for candidate in "$TMPDIR/bin/ollama" "$TMPDIR/ollama" "$TMPDIR/ollama.exe"; do
    if [ -f "$candidate" ]; then
        OLLAMA_BIN="$candidate"
        break
    fi
done
[ -n "$OLLAMA_BIN" ] || { echo "Ollama binary not found in verified archive" >&2; exit 1; }

install -m 0755 "$OLLAMA_BIN" "$OUT_FILE"
printf 'Saved verified sidecar: %s\n' "$OUT_FILE"
printf 'Archive SHA-256: %s\n' "$actual_sha"
