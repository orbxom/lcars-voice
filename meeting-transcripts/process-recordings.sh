#!/bin/bash
# Master script: Download, transcribe, and fetch JIRA information for recordings
# Usage: ./process-recordings.sh <date-folder> [local-folder] [aws-profile]
#
# Examples:
#   ./process-recordings.sh 01-29-2026                    # Uses defaults from .env
#   ./process-recordings.sh 01-29-2026 ./recordings      # Custom local folder
#   ./process-recordings.sh s3://bucket/path ./recordings sandbox  # Full S3 path

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load config from .env if it exists
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
fi

# Check arguments
if [[ -z "$1" ]]; then
    echo "Usage: $0 <date-folder> [local-folder] [aws-profile]"
    echo ""
    echo "Examples:"
    echo "  $0 01-29-2026                         # Uses defaults from .env"
    echo "  $0 01-29-2026 ./output                # Custom local folder"
    echo "  $0 s3://bucket/path ./output sandbox  # Full S3 path"
    exit 1
fi

# Build S3 path - if it doesn't start with s3://, prepend bucket from config
if [[ "$1" == s3://* ]]; then
    S3_PATH="$1"
else
    S3_BUCKET="${S3_BUCKET:?S3_BUCKET is required - set in .env or provide full s3:// path}"
    S3_PATH="s3://$S3_BUCKET/$1"
fi

LOCAL_DIR="${2:-$SCRIPT_DIR/recordings}"
AWS_PROFILE="${3:-$AWS_PROFILE}"

# Resolve to absolute path
mkdir -p "$LOCAL_DIR"
RECORDINGS_DIR="$(cd "$LOCAL_DIR" && pwd)"

echo "=========================================="
echo "Processing recordings"
echo "=========================================="
echo "S3 Source: $S3_PATH"
echo "Local folder: $RECORDINGS_DIR"
if [[ -n "$AWS_PROFILE" ]]; then
    echo "AWS Profile: $AWS_PROFILE"
fi
echo ""

# Step 1: Download from S3
echo "=========================================="
echo "Step 1: Downloading from S3"
echo "=========================================="
"$SCRIPT_DIR/download-recordings.sh" "$S3_PATH" "$RECORDINGS_DIR" "$AWS_PROFILE"

echo ""

# Check for RecordingIndex.md
if [[ ! -f "$RECORDINGS_DIR/RecordingIndex.md" ]]; then
    echo "ERROR: RecordingIndex.md not found in $RECORDINGS_DIR"
    echo "This file is required to map recordings to JIRA IDs."
    exit 1
fi

# Check for MP4 files
mp4_count=$(find "$RECORDINGS_DIR" -maxdepth 1 -name "*.mp4" | wc -l)
if [[ "$mp4_count" -eq 0 ]]; then
    echo "ERROR: No MP4 files found in $RECORDINGS_DIR"
    exit 1
fi

echo "Found $mp4_count MP4 file(s) to process"
echo ""

# Step 2: Transcribe recordings
echo "=========================================="
echo "Step 2: Transcribing recordings"
echo "=========================================="
"$SCRIPT_DIR/transcribe-all.sh" "$RECORDINGS_DIR"

echo ""

# Step 3: Fetch JIRA information
echo "=========================================="
echo "Step 3: Fetching JIRA information"
echo "=========================================="
"$SCRIPT_DIR/fetch-jira-info.sh" "$RECORDINGS_DIR"

echo ""
echo "=========================================="
echo "Processing complete!"
echo "=========================================="
echo ""
echo "Output files in $RECORDINGS_DIR:"
ls -la "$RECORDINGS_DIR"/*.md 2>/dev/null | grep -v RecordingIndex || echo "No markdown files found"
