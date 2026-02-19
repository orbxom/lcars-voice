#!/bin/bash
set -e

# Process local zoom-recorder output into per-ticket transcript files.
#
# Usage: process-local-recordings.sh [date] [output-dir]
#   date:       Optional date filter (YYYY-MM-DD). Defaults to today.
#   output-dir: Where to write .md files. Defaults to $SCRIPT_DIR/recordings
#
# Finds recording directories in RECORDINGS_SOURCE (default ~/zoom-recordings/)
# matching the given date, transcribes each with whisper, and segments the
# transcript by JIRA ticket marks from timestamps.json.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load .env if present
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    source "$SCRIPT_DIR/.env"
fi

# Configuration
RECORDINGS_SOURCE="${RECORDINGS_SOURCE:-$HOME/zoom-recordings}"
WHISPER_MODEL="${WHISPER_MODEL:-large-v3}"
PYTHON_ENV="${PYTHON_ENV:-$HOME/voice-to-text-env/bin/python}"
WHISPER_SCRIPT="$SCRIPT_DIR/whisper-wrapper.py"
DIARIZE_SCRIPT="$SCRIPT_DIR/diarize.py"
SEGMENT_SCRIPT="$SCRIPT_DIR/segment-transcript.py"

# Arguments
DATE_FILTER="${1:-$(date +%Y-%m-%d)}"
OUTPUT_DIR="${2:-$SCRIPT_DIR/recordings}"

echo "Processing local recordings"
echo "  Source:  $RECORDINGS_SOURCE"
echo "  Date:    $DATE_FILTER"
echo "  Output:  $OUTPUT_DIR"
echo "  Model:   $WHISPER_MODEL"
echo ""

# Verify tools exist
if [[ ! -f "$PYTHON_ENV" ]]; then
    echo "Error: Python environment not found at $PYTHON_ENV" >&2
    echo "Run setup or set PYTHON_ENV in .env" >&2
    exit 1
fi

if ! command -v ffmpeg &>/dev/null; then
    echo "Error: ffmpeg not found" >&2
    exit 1
fi

# Find recording directories matching the date
# zoom-recorder names dirs as YYYY-MM-DD-HHMMSS
matching_dirs=()
for dir in "$RECORDINGS_SOURCE"/"$DATE_FILTER"-*/; do
    if [[ -d "$dir" && -f "$dir/audio.wav" ]]; then
        matching_dirs+=("$dir")
    fi
done

if [[ ${#matching_dirs[@]} -eq 0 ]]; then
    echo "No recordings found for $DATE_FILTER in $RECORDINGS_SOURCE"
    exit 0
fi

echo "Found ${#matching_dirs[@]} recording(s) for $DATE_FILTER"
echo ""

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Process each recording
for recording_dir in "${matching_dirs[@]}"; do
    dir_name=$(basename "$recording_dir")
    audio_file="$recording_dir/audio.wav"
    timestamps_file="$recording_dir/timestamps.json"

    echo "Processing: $dir_name"

    # Check for timestamps.json
    if [[ ! -f "$timestamps_file" ]]; then
        echo "  Warning: No timestamps.json found, skipping" >&2
        continue
    fi

    # Transcribe with whisper
    echo "  Transcribing with whisper ($WHISPER_MODEL)..."
    whisper_output_file=$(mktemp /tmp/whisper-output-XXXXXX.json)

    whisper_log="$recording_dir/whisper.log"
    json_output=$("$PYTHON_ENV" "$WHISPER_SCRIPT" "$audio_file" "$WHISPER_MODEL" 2>"$whisper_log") || true

    if [[ -z "$json_output" ]]; then
        echo "  Error: No JSON output from whisper (see $whisper_log)" >&2
        rm -f "$whisper_output_file"
        continue
    fi

    echo "$json_output" > "$whisper_output_file"

    # Diarize — add speaker labels
    echo "  Running speaker diarization..."
    diarized_output_file=$(mktemp /tmp/diarized-output-XXXXXX.json)
    diarize_log="$recording_dir/diarize.log"
    if "$PYTHON_ENV" "$DIARIZE_SCRIPT" "$audio_file" "$whisper_output_file" ${HF_TOKEN:+--hf-token "$HF_TOKEN"} > "$diarized_output_file" 2>"$diarize_log"; then
        segment_input="$diarized_output_file"
    else
        echo "  Warning: Diarization failed, proceeding without speaker labels (see $diarize_log)" >&2
        segment_input="$whisper_output_file"
    fi

    # Extract date from directory name (YYYY-MM-DD from YYYY-MM-DD-HHMMSS)
    recording_date=$(echo "$dir_name" | cut -d'-' -f1-3)

    # Segment transcript by JIRA ticket marks
    echo "  Segmenting by ticket marks..."
    "$PYTHON_ENV" "$SEGMENT_SCRIPT" \
        "$segment_input" \
        "$timestamps_file" \
        "$OUTPUT_DIR" \
        --source "$dir_name/audio.wav" \
        --date "$recording_date"

    # Clean up temp files
    rm -f "$whisper_output_file" "$diarized_output_file"

    echo "  Done"
    echo ""
done

echo "All recordings processed. Output in: $OUTPUT_DIR"
