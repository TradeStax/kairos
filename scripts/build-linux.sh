#!/usr/bin/env bash
# build-linux.sh — Build and package Kairos for Linux
set -euo pipefail
source "$(dirname "$0")/_common.sh"

# ── Defaults ──────────────────────────────────────────────────────────────────

ARCH="x86_64"
FEATURES="heatmap"
BIN_NAME="kairos"

# ── Usage ─────────────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build and package Kairos for Linux.

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
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    *)       error "Unsupported architecture: $ARCH (use x86_64 or aarch64)"; exit 1 ;;
esac

VERSION="$(detect_version)"
ARCHIVE_NAME="kairos-${VERSION}-${TARGET}.tar.gz"

step "Building Kairos v${VERSION} for ${TARGET}"

# ── Prerequisites ─────────────────────────────────────────────────────────────

require_command cargo
require_command rustup
require_command tar

# ── Verify assets ─────────────────────────────────────────────────────────────

verify_assets

# ── Cross-compilation setup ───────────────────────────────────────────────────

if [[ "$ARCH" == "aarch64" ]]; then
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc"
    export CC_aarch64_unknown_linux_gnu="aarch64-linux-gnu-gcc"
    export CXX_aarch64_unknown_linux_gnu="aarch64-linux-gnu-g++"
fi

# ── Build ─────────────────────────────────────────────────────────────────────

step "Compiling release binary"
rustup target add "$TARGET" 2>/dev/null || true

CARGO_ARGS=(build --release --target="$TARGET")
FEATURES_FLAG="$(build_features_flag "$FEATURES")"
[[ -n "$FEATURES_FLAG" ]] && CARGO_ARGS+=("$FEATURES_FLAG")

cargo "${CARGO_ARGS[@]}"

BINARY="target/${TARGET}/release/${BIN_NAME}"
if [[ ! -f "$BINARY" ]]; then
    error "Build succeeded but binary not found at $BINARY"
    exit 1
fi
success "Binary built: $BINARY"

# ── Package ───────────────────────────────────────────────────────────────────

step "Creating archive: $ARCHIVE_NAME"
STAGING="$(create_staging)"

mkdir -p "$STAGING/bin"
cp "$BINARY" "$STAGING/bin/${BIN_NAME}"
cp -r "${REPO_ROOT}/assets" "$STAGING/assets"

OUTPUT_DIR="${REPO_ROOT}/target/release"
mkdir -p "$OUTPUT_DIR"
ARCHIVE="${OUTPUT_DIR}/${ARCHIVE_NAME}"

tar -czf "$ARCHIVE" -C "$STAGING" .

success "Archive created"

# ── Checksum ──────────────────────────────────────────────────────────────────

generate_checksum "$ARCHIVE"

# ── Summary ───────────────────────────────────────────────────────────────────

print_summary "$ARCHIVE"
