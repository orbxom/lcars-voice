#!/bin/bash
# Transcribe all recordings and rename them to match JIRA numbers
# Reads mapping from RecordingIndex.md
# Idempotent: overwrites existing transcripts and renamed files
# Usage: ./transcribe-all.sh [recordings-folder]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RECORDINGS_DIR="${1:-$SCRIPT_DIR/recordings}"
WHISPER_SCRIPT="$SCRIPT_DIR/whisper-wrapper.py"
PYTHON_ENV="$HOME/voice-to-text-env/bin/python"

# Load config from .env if it exists
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
fi

WHISPER_MODEL="${WHISPER_MODEL:-large-v3}"

# Parse RecordingIndex.md to build mapping
# Format: One JIRA ID per line (or multiple joined with dashes), mapped to MP4s in timestamp order
parse_recording_index() {
    local index_file="$RECORDINGS_DIR/RecordingIndex.md"

    if [[ ! -f "$index_file" ]]; then
        echo "ERROR: RecordingIndex.md not found in $RECORDINGS_DIR" >&2
        exit 1
    fi

    # Get sorted list of MP4 files by name (which sorts by timestamp)
    mapfile -t mp4_files < <(find "$RECORDINGS_DIR" -maxdepth 1 -name "*.mp4" -printf "%f\n" | sort)

    # Parse JIRA IDs from index file - simple format: one entry per non-comment, non-empty line
    mapfile -t jira_entries < <(grep -v '^#' "$index_file" | grep -v '^$' | grep -E '^[A-Z]+-[0-9]')

    # Verify counts match
    if [[ ${#mp4_files[@]} -ne ${#jira_entries[@]} ]]; then
        echo "WARNING: MP4 count (${#mp4_files[@]}) doesn't match JIRA entries (${#jira_entries[@]})" >&2
        echo "MP4 files: ${mp4_files[*]}" >&2
        echo "JIRA entries: ${jira_entries[*]}" >&2
    fi

    # Output mapping
    for i in "${!mp4_files[@]}"; do
        if [[ $i -lt ${#jira_entries[@]} ]]; then
            echo "${mp4_files[$i]%.mp4}|${jira_entries[$i]}"
        fi
    done
}

echo "Starting transcription process..."
echo "================================="
echo "Using Whisper model: $WHISPER_MODEL"
echo ""

# Build mapping from RecordingIndex.md
echo "Parsing RecordingIndex.md..."
mapfile -t mappings < <(parse_recording_index)

if [[ ${#mappings[@]} -eq 0 ]]; then
    echo "ERROR: No mappings found"
    exit 1
fi

echo "Found ${#mappings[@]} recording(s) to process"
echo ""

for mapping in "${mappings[@]}"; do
    IFS='|' read -r original_name jira_id <<< "$mapping"

    original_file="$RECORDINGS_DIR/$original_name.mp4"
    new_file="$RECORDINGS_DIR/$jira_id.mp4"
    transcript_file="$RECORDINGS_DIR/$jira_id.md"

    echo ""
    echo "Processing: $original_name -> $jira_id"

    # Find the source file (could be original name or already renamed)
    if [[ -f "$original_file" ]]; then
        source_file="$original_file"
    elif [[ -f "$new_file" ]]; then
        source_file="$new_file"
    else
        echo "  ERROR: Cannot find source file for $jira_id"
        continue
    fi

    # Transcribe
    echo "  Transcribing with $WHISPER_MODEL model..."
    result=$("$PYTHON_ENV" "$WHISPER_SCRIPT" "$source_file" "$WHISPER_MODEL" 2>&1)

    # Extract JSON (last line that starts with {)
    json_output=$(echo "$result" | grep '^{' | tail -1)

    if [[ -z "$json_output" ]]; then
        echo "  ERROR: No JSON output from whisper"
        echo "  Output was: $result"
        continue
    fi

    # Extract text from JSON
    transcript=$(echo "$json_output" | python3 -c "import sys, json; print(json.load(sys.stdin).get('text', ''))")

    if [[ -z "$transcript" ]]; then
        echo "  ERROR: Empty transcript"
        continue
    fi

    # Write markdown file
    echo "  Writing transcript to $jira_id.md..."
    cat > "$transcript_file" << EOF
# $jira_id - Meeting Transcript

**Source:** $original_name.mp4
**Date:** $(echo "$original_name" | cut -d' ' -f1)

---

$transcript
EOF

    # Rename recording if not already renamed
    if [[ "$source_file" != "$new_file" ]]; then
        echo "  Renaming to $jira_id.mp4..."
        mv "$source_file" "$new_file"
    fi

    echo "  Done!"
done

echo ""
echo "================================="
echo "Transcription complete!"
echo ""
echo "Generated files:"
ls -la "$RECORDINGS_DIR"/*.md 2>/dev/null || echo "No markdown files found"
