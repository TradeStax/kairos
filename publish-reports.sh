#!/bin/bash
#
# Publishes reports/ to tradestax.io GCS bucket
# Usage: ./publish-reports.sh
#
# Prerequisites:
#   gcloud auth login        # One-time login (interactive)
#   gcloud auth activate-service-account --key-file=KEY.json  # Service account (non-interactive)

set -e

BUCKET="gs://tradestax.io"
SOURCE_DIR="reports"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
DEST_PATH="reports/${TIMESTAMP}"

# Check if reports directory exists and has files
if [ ! -d "$SOURCE_DIR" ]; then
    echo "Error: $SOURCE_DIR directory not found"
    exit 1
fi

# Count files to upload
FILE_COUNT=$(find "$SOURCE_DIR" -type f | wc -l)
if [ "$FILE_COUNT" -eq 0 ]; then
    echo "Error: No files found in $SOURCE_DIR"
    exit 1
fi

# Verify gsutil is authenticated
if ! gsutil ls "${BUCKET}" >/dev/null 2>&1; then
    echo "Error: Not authenticated with Google Cloud. Run:"
    echo "  gcloud auth login"
    echo ""
    echo "Or for non-interactive use, activate a service account:"
    echo "  gcloud auth activate-service-account --key-file=service-account.json"
    exit 1
fi

echo "Publishing reports to tradestax.io..."
echo "  Source: $SOURCE_DIR ($FILE_COUNT files)"
echo "  Destination: gs://${DEST_PATH}/"
echo ""

# Upload all files from reports/ preserving directory structure
gsutil -m cp -r "${SOURCE_DIR}/*" "${BUCKET}/${DEST_PATH}/"

# Set cache control for HTML files (no caching)
gsutil -m setmeta -h "Cache-Control:no-cache" "${BUCKET}/${DEST_PATH}/**/*.html" 2>/dev/null || true

# Find the main HTML file (prefer cumulative_report.html, then first .html found)
MAIN_HTML=$(gsutil ls "${BUCKET}/${DEST_PATH}/**/*.html" 2>/dev/null | head -1 | sed 's|^gs://tradestax.io/||')

echo ""
echo "✓ Published successfully!"
echo ""
if [ -n "$MAIN_HTML" ]; then
    echo "https://tradestax.io/${MAIN_HTML}"
else
    echo "View all files at:"
    echo "https://tradestax.io/${DEST_PATH}/"
fi
