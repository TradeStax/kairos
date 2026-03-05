#!/usr/bin/env bash
set -euo pipefail

# Generate update-manifest.json from release artifacts and checksum files.
# Intended for use in CI after all platform builds complete.
#
# Required environment:
#   CI_COMMIT_TAG         — Git tag (e.g., v0.9.8)
#   CI_API_V4_URL         — GitLab API base URL
#   CI_PROJECT_ID         — GitLab project ID

TAG="${CI_COMMIT_TAG:-v0.0.0}"
VERSION="${TAG#v}"
RELEASE_DIR="target/release"
BASE_URL="${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/packages/generic/kairos/${TAG}"

declare -A CHECKSUMS SIZES

for sha_file in "${RELEASE_DIR}"/kairos-*.sha256; do
    [ -f "$sha_file" ] || continue
    checksum=$(awk '{print $1}' "$sha_file")
    archive=$(awk '{print $2}' "$sha_file" | sed 's/^\*//')

    if [[ "$archive" == *"x86_64-pc-windows-msvc"* ]]; then
        platform="x86_64-pc-windows-msvc"
    elif [[ "$archive" == *"universal-apple-darwin"* ]]; then
        platform="universal-apple-darwin"
    elif [[ "$archive" == *"x86_64-unknown-linux-gnu"* ]]; then
        platform="x86_64-unknown-linux-gnu"
    elif [[ "$archive" == *"aarch64-unknown-linux-gnu"* ]]; then
        platform="aarch64-unknown-linux-gnu"
    else
        continue
    fi

    archive_path="${RELEASE_DIR}/${archive}"
    size=0
    if [ -f "$archive_path" ]; then
        size=$(stat -c%s "$archive_path" 2>/dev/null \
            || stat -f%z "$archive_path" 2>/dev/null \
            || echo 0)
    fi

    CHECKSUMS[$platform]="$checksum"
    SIZES[$platform]="$size"
done

# Build platforms JSON
platforms_json=""
first=true
for platform in "${!CHECKSUMS[@]}"; do
    ext="tar.gz"
    [[ "$platform" == *"windows"* ]] && ext="zip"

    if [ "$first" = true ]; then
        first=false
    else
        platforms_json+=","
    fi

    platforms_json+="\"${platform}\": {\"url\": \"${BASE_URL}/kairos-${TAG}-${platform}.${ext}\", \"sha256\": \"${CHECKSUMS[$platform]}\", \"size\": ${SIZES[$platform]}}"
done

cat > "${RELEASE_DIR}/update-manifest.json" <<EOF
{
  "version": "${VERSION}",
  "release_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "minimum_version": null,
  "release_notes": "See release page for full details.",
  "platforms": {${platforms_json}}
}
EOF

echo "Generated update-manifest.json:"
cat "${RELEASE_DIR}/update-manifest.json"
