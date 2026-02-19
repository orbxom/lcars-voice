#!/bin/bash
set -e

# Process zoom recordings into per-ticket transcripts enriched with JIRA info.
#
# Usage: process-recordings.sh [date] [output-dir]
#   date:       Optional date filter (YYYY-MM-DD). Defaults to today.
#   output-dir: Where to write .md files. Defaults to $SCRIPT_DIR/recordings
#
# Pipeline:
#   1. process-local-recordings.sh - transcribe and segment by ticket
#   2. fetch-jira-info.sh - enrich with JIRA metadata

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DATE="${1:-$(date +%Y-%m-%d)}"
OUTPUT_DIR="${2:-$SCRIPT_DIR/recordings}"

echo "=== Zoom Recording Pipeline ==="
echo "Date:   $DATE"
echo "Output: $OUTPUT_DIR"
echo ""

# Step 1: Transcribe and segment
echo "--- Step 1: Transcribe and segment recordings ---"
"$SCRIPT_DIR/process-local-recordings.sh" "$DATE" "$OUTPUT_DIR"
echo ""

# Step 2: Fetch JIRA info
echo "--- Step 2: Enrich with JIRA metadata ---"
"$SCRIPT_DIR/fetch-jira-info.sh" "$OUTPUT_DIR"
echo ""

echo "=== Pipeline complete ==="
echo "Output files in: $OUTPUT_DIR"
