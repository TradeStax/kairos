#!/usr/bin/env bash
# _common.sh — Shared build utilities for Kairos build scripts
# Source this file, do not execute it directly.
#
# Usage: source "$(dirname "$0")/_common.sh"

set -euo pipefail

# ── Color output ──────────────────────────────────────────────────────────────

_use_color() {
    [[ -z "${NO_COLOR:-}" ]] && [[ -t 1 || "${FORCE_COLOR:-}" == "1" ]]
}

if _use_color; then
    _RED=$'\033[0;31m'
    _GREEN=$'\033[0;32m'
    _YELLOW=$'\033[0;33m'
    _BLUE=$'\033[0;34m'
    _CYAN=$'\033[0;36m'
    _BOLD=$'\033[1m'
    _RESET=$'\033[0m'
else
    _RED="" _GREEN="" _YELLOW="" _BLUE="" _CYAN="" _BOLD="" _RESET=""
fi

info()    { echo "${_BLUE}info:${_RESET} $*"; }
success() { echo "${_GREEN}ok:${_RESET} $*"; }
warn()    { echo "${_YELLOW}warn:${_RESET} $*" >&2; }
error()   { echo "${_RED}error:${_RESET} $*" >&2; }
step()    { echo; echo "${_BOLD}${_CYAN}==> $*${_RESET}"; }

# ── Version detection ─────────────────────────────────────────────────────────

detect_version() {
    local cargo_toml="${REPO_ROOT}/app/Cargo.toml"
    if [[ ! -f "$cargo_toml" ]]; then
        error "Cannot find $cargo_toml"
        exit 1
    fi
    grep '^version = ' "$cargo_toml" | head -1 | sed 's/version = "\(.*\)"/\1/'
}

detect_tag() {
    if [[ -n "${CI_COMMIT_TAG:-}" ]]; then
        echo "$CI_COMMIT_TAG"
    else
        echo "v$(detect_version)"
    fi
}

# ── Repository root ──────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# ── Asset verification ────────────────────────────────────────────────────────

verify_assets() {
    local assets_dir="${REPO_ROOT}/assets"
    local missing=0

    for subdir in fonts icons sounds; do
        if [[ ! -d "$assets_dir/$subdir" ]]; then
            error "Missing assets directory: assets/$subdir"
            missing=1
        fi
    done

    if [[ $missing -ne 0 ]]; then
        error "Asset verification failed"
        exit 1
    fi
    success "Assets verified"
}

# ── Checksum generation ──────────────────────────────────────────────────────

generate_checksum() {
    local file="$1"
    local checksum_file="${file}.sha256"

    if command -v sha256sum &>/dev/null; then
        sha256sum "$file" > "$checksum_file"
    elif command -v shasum &>/dev/null; then
        shasum -a 256 "$file" > "$checksum_file"
    else
        warn "No sha256sum or shasum found — skipping checksum"
        return 0
    fi
    success "Checksum written: $(basename "$checksum_file")"
}

# ── Staging directory with cleanup ───────────────────────────────────────────

_STAGING_DIR=""

create_staging() {
    _STAGING_DIR="$(mktemp -d)"
    trap 'rm -rf "$_STAGING_DIR"' EXIT
}

# ── Prerequisite check ───────────────────────────────────────────────────────

require_command() {
    local cmd="$1"
    local hint="${2:-}"
    if ! command -v "$cmd" &>/dev/null; then
        error "'$cmd' is required but not found"
        [[ -n "$hint" ]] && info "$hint"
        exit 1
    fi
}

# ── CI detection ─────────────────────────────────────────────────────────────

is_ci() {
    [[ -n "${CI:-}" || -n "${GITLAB_CI:-}" || -n "${GITHUB_ACTIONS:-}" ]]
}

# ── Summary printer ──────────────────────────────────────────────────────────

print_summary() {
    local artifact="$1"
    local name size sha

    name="$(basename "$artifact")"

    if [[ -f "$artifact" ]]; then
        if [[ "$(uname -s)" == "Darwin" ]]; then
            size="$(stat -f%z "$artifact" 2>/dev/null || echo "?")"
        else
            size="$(stat --printf='%s' "$artifact" 2>/dev/null || echo "?")"
        fi
    else
        size="?"
    fi

    sha="unknown"
    if [[ -f "${artifact}.sha256" ]]; then
        sha="$(awk '{print $1}' "${artifact}.sha256")"
    fi

    echo
    echo "${_BOLD}── Artifact ──────────────────────────${_RESET}"
    echo "  File:   $name"
    echo "  Size:   $size bytes"
    echo "  SHA256: $sha"
    echo "${_BOLD}──────────────────────────────────────${_RESET}"
}

# ── Cargo features flag builder ──────────────────────────────────────────────

build_features_flag() {
    local features="${1:-}"
    if [[ -n "$features" ]]; then
        echo "--features=$features"
    fi
}
