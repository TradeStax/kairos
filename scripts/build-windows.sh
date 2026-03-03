#!/usr/bin/env bash
# build-windows.sh — Build and package Kairos for Windows
set -euo pipefail
source "$(dirname "$0")/_common.sh"

# ── Defaults ──────────────────────────────────────────────────────────────────

ARCH="x86_64"
FEATURES="heatmap"
EXE_NAME="kairos.exe"

# ── Usage ─────────────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build and package Kairos for Windows.

Options:
  --arch ARCH        Target architecture: x86_64 (default), aarch64
  --features FEAT    Cargo features to enable (default: heatmap)
  --help             Show this help message
EOF
    exit 0
}

# ── Argument parsing ──────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)    ARCH="$2"; shift 2 ;;
        --features) FEATURES="$2"; shift 2 ;;
        --help)    usage ;;
        *)         error "Unknown argument: $1"; usage ;;
    esac
done

# ── Resolve target ────────────────────────────────────────────────────────────

case "$ARCH" in
    x86_64)  TARGET="x86_64-pc-windows-msvc" ;;
    aarch64) TARGET="aarch64-pc-windows-msvc" ;;
    *)       error "Unsupported architecture: $ARCH (use x86_64 or aarch64)"; exit 1 ;;
esac

VERSION="$(detect_version)"
ARCHIVE_NAME="kairos-${VERSION}-${TARGET}.zip"

step "Building Kairos v${VERSION} for ${TARGET}"

# ── Prerequisites ─────────────────────────────────────────────────────────────

require_command cargo
require_command rustup

# ── Verify assets ─────────────────────────────────────────────────────────────

verify_assets

# ── Build ─────────────────────────────────────────────────────────────────────

step "Compiling release binary"
rustup target add "$TARGET" 2>/dev/null || true

CARGO_ARGS=(build --release --target="$TARGET")
FEATURES_FLAG="$(build_features_flag "$FEATURES")"
[[ -n "$FEATURES_FLAG" ]] && CARGO_ARGS+=("$FEATURES_FLAG")

cargo "${CARGO_ARGS[@]}"

BINARY="target/${TARGET}/release/${EXE_NAME}"
if [[ ! -f "$BINARY" ]]; then
    error "Build succeeded but binary not found at $BINARY"
    exit 1
fi
success "Binary built: $BINARY"

# ── Package ───────────────────────────────────────────────────────────────────

step "Creating archive: $ARCHIVE_NAME"
STAGING="$(create_staging)"

cp "$BINARY" "$STAGING/${EXE_NAME}"
cp -r "${REPO_ROOT}/assets" "$STAGING/assets"

OUTPUT_DIR="${REPO_ROOT}/target/release"
mkdir -p "$OUTPUT_DIR"
ARCHIVE="${OUTPUT_DIR}/${ARCHIVE_NAME}"

# Zip tool fallback chain: 7z > zip > PowerShell
if command -v 7z &>/dev/null; then
    (cd "$STAGING" && 7z a -tzip "$ARCHIVE" . >/dev/null)
elif command -v zip &>/dev/null; then
    (cd "$STAGING" && zip -r "$ARCHIVE" . >/dev/null)
elif command -v powershell &>/dev/null; then
    # Convert to Windows-style paths for PowerShell
    local_staging="$(cygpath -w "$STAGING" 2>/dev/null || echo "$STAGING")"
    local_archive="$(cygpath -w "$ARCHIVE" 2>/dev/null || echo "$ARCHIVE")"
    powershell -Command "Compress-Archive -Path '${local_staging}\\*' -DestinationPath '${local_archive}' -Force"
elif command -v powershell.exe &>/dev/null; then
    local_staging="$(cygpath -w "$STAGING" 2>/dev/null || echo "$STAGING")"
    local_archive="$(cygpath -w "$ARCHIVE" 2>/dev/null || echo "$ARCHIVE")"
    powershell.exe -Command "Compress-Archive -Path '${local_staging}\\*' -DestinationPath '${local_archive}' -Force"
else
    error "No zip tool found (tried: 7z, zip, powershell)"
    exit 1
fi

success "Archive created"

# ── Checksum ──────────────────────────────────────────────────────────────────

generate_checksum "$ARCHIVE"

# ── Summary ───────────────────────────────────────────────────────────────────

print_summary "$ARCHIVE"
