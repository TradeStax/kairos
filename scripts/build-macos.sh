#!/usr/bin/env bash
# build-macos.sh — Build and package Kairos for macOS
set -euo pipefail
source "$(dirname "$0")/_common.sh"

# ── Defaults ──────────────────────────────────────────────────────────────────

ARCH="universal"
FEATURES="heatmap"
BIN_NAME="kairos"
export MACOSX_DEPLOYMENT_TARGET="11.0"

# ── Usage ─────────────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build and package Kairos for macOS.

Options:
  --arch ARCH        Target architecture: x86_64, aarch64, universal (default)
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

# ── Resolve target(s) ────────────────────────────────────────────────────────

TARGETS=()
case "$ARCH" in
    x86_64)    TARGETS=("x86_64-apple-darwin");  LABEL="x86_64-apple-darwin" ;;
    aarch64)   TARGETS=("aarch64-apple-darwin");  LABEL="aarch64-apple-darwin" ;;
    universal) TARGETS=("x86_64-apple-darwin" "aarch64-apple-darwin"); LABEL="universal-apple-darwin" ;;
    *)         error "Unsupported architecture: $ARCH (use x86_64, aarch64, or universal)"; exit 1 ;;
esac

VERSION="$(detect_version)"
ARCHIVE_NAME="kairos-${VERSION}-${LABEL}.tar.gz"

step "Building Kairos v${VERSION} for ${LABEL}"

# ── Prerequisites ─────────────────────────────────────────────────────────────

require_command cargo
require_command rustup
if [[ "$ARCH" == "universal" ]]; then
    require_command lipo "Install Xcode command line tools: xcode-select --install"
fi

# ── Verify assets ─────────────────────────────────────────────────────────────

verify_assets

# ── Build ─────────────────────────────────────────────────────────────────────

FEATURES_FLAG="$(build_features_flag "$FEATURES")"

for target in "${TARGETS[@]}"; do
    step "Compiling for ${target}"
    rustup target add "$target" 2>/dev/null || true

    CARGO_ARGS=(build --release --target="$target")
    [[ -n "$FEATURES_FLAG" ]] && CARGO_ARGS+=("$FEATURES_FLAG")

    cargo "${CARGO_ARGS[@]}"

    binary="target/${target}/release/${BIN_NAME}"
    if [[ ! -f "$binary" ]]; then
        error "Build succeeded but binary not found at $binary"
        exit 1
    fi
    success "Binary built: $binary"
done

# ── Universal binary (lipo) ──────────────────────────────────────────────────

OUTPUT_DIR="${REPO_ROOT}/target/release"
mkdir -p "$OUTPUT_DIR"
FINAL_BIN="${OUTPUT_DIR}/${BIN_NAME}"

if [[ "$ARCH" == "universal" ]]; then
    step "Creating universal binary"
    lipo \
        "target/x86_64-apple-darwin/release/${BIN_NAME}" \
        "target/aarch64-apple-darwin/release/${BIN_NAME}" \
        -create -output "$FINAL_BIN"
    success "Universal binary created"
else
    cp "target/${TARGETS[0]}/release/${BIN_NAME}" "$FINAL_BIN"
fi

# ── Package ───────────────────────────────────────────────────────────────────

step "Creating archive: $ARCHIVE_NAME"
create_staging
STAGING="$_STAGING_DIR"

cp "$FINAL_BIN" "$STAGING/${BIN_NAME}"
cp -r "${REPO_ROOT}/assets" "$STAGING/assets"

ARCHIVE="${OUTPUT_DIR}/${ARCHIVE_NAME}"
tar -czf "$ARCHIVE" -C "$STAGING" .

success "Archive created"

# ── Checksum ──────────────────────────────────────────────────────────────────

generate_checksum "$ARCHIVE"

# ── Summary ───────────────────────────────────────────────────────────────────

print_summary "$ARCHIVE"
