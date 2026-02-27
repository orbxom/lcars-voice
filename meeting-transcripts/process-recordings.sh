#!/bin/bash
set -e

# Process recordings into transcripts.
#
# Usage: process-recordings.sh [date] [output-dir]
#   date:       Optional date filter (YYYY-MM-DD). Defaults to today.
#   output-dir: Where to write .md files. Defaults to $SCRIPT_DIR/recordings
#
# Pipeline:
#   1. process-local-recordings.sh - transcribe, diarize, and segment
#
# Note: fetch-jira-info.sh is now a standalone optional tool.
# Run it separately if you need JIRA metadata enrichment.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DATE="${1:-$(date +%Y-%m-%d)}"
OUTPUT_DIR="${2:-$SCRIPT_DIR/recordings}"

echo "=== Recording Pipeline ==="
echo "Date:   $DATE"
echo "Output: $OUTPUT_DIR"
echo ""

# Step 1: Transcribe and segment
echo "--- Transcribe and segment recordings ---"
"$SCRIPT_DIR/process-local-recordings.sh" "$DATE" "$OUTPUT_DIR"
echo ""

echo "=== Pipeline complete ==="
echo "Output files in: $OUTPUT_DIR"
