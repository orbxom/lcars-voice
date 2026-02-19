#!/bin/bash
# Download recordings from S3 bucket
# Idempotent: only downloads new/changed files (uses aws s3 sync)
# Usage: ./download-recordings.sh <s3-path> <local-folder> [aws-profile]
#
# Example: ./download-recordings.sh s3://growth-recordings/01-29-2026 ./recordings sandbox

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load config from .env if it exists
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
fi

# Check arguments
if [[ -z "$1" || -z "$2" ]]; then
    echo "Usage: $0 <s3-path> <local-folder> [aws-profile]"
    echo "Example: $0 s3://growth-recordings/01-29-2026 ./recordings sandbox"
    exit 1
fi

S3_PATH="$1"
LOCAL_DIR="$2"
AWS_PROFILE="${3:-$AWS_PROFILE}"

# Build profile flag if provided
PROFILE_FLAG=""
if [[ -n "$AWS_PROFILE" ]]; then
    PROFILE_FLAG="--profile $AWS_PROFILE"
fi

echo "Downloading recordings from S3..."
echo "================================="
echo "Source: $S3_PATH"
echo "Destination: $LOCAL_DIR"
if [[ -n "$AWS_PROFILE" ]]; then
    echo "AWS Profile: $AWS_PROFILE"
fi
echo ""

# Create local directory if it doesn't exist
mkdir -p "$LOCAL_DIR"

# Sync from S3 (idempotent - only downloads new/changed files)
echo "Syncing files..."
aws s3 sync "$S3_PATH" "$LOCAL_DIR" $PROFILE_FLAG

echo ""
echo "================================="
echo "Download complete!"
echo ""
echo "Downloaded files:"
ls -lh "$LOCAL_DIR"
