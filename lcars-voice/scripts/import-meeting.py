#!/usr/bin/env python3
"""Import an external audio file into the lcars-voice meeting database."""

import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path


def get_duration_ms(filepath: str) -> int:
    result = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-of", "default=noprint_wrappers=1:nokey=1", filepath],
        capture_output=True, text=True, check=True,
    )
    return int(float(result.stdout.strip()) * 1000)


def convert_to_wav(input_path: str, output_path: str) -> None:
    subprocess.run(
        ["ffmpeg", "-y", "-i", input_path,
         "-ar", "16000", "-ac", "1", "-sample_fmt", "s16",
         "-f", "wav", output_path],
        check=True,
    )


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <audio-file>", file=sys.stderr)
        sys.exit(1)

    input_path = sys.argv[1]
    if not os.path.isfile(input_path):
        print(f"Error: file not found: {input_path}", file=sys.stderr)
        sys.exit(1)

    db_path = Path.home() / ".local" / "share" / "lcars-voice" / "history.db"
    if not db_path.exists():
        print(f"Error: database not found: {db_path}", file=sys.stderr)
        sys.exit(1)

    input_name = Path(input_path).stem
    filename = f"{input_name}.wav"

    print(f"Getting duration of {input_path}...")
    duration_ms = get_duration_ms(input_path)
    print(f"Duration: {duration_ms}ms ({duration_ms / 1000 / 60:.1f} min)")

    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
        tmp_path = tmp.name

    try:
        print(f"Converting to 16kHz mono WAV...")
        convert_to_wav(input_path, tmp_path)

        wav_bytes = open(tmp_path, "rb").read()
        size_bytes = len(wav_bytes)
        print(f"WAV size: {size_bytes / 1024 / 1024:.1f} MB")
    finally:
        os.unlink(tmp_path)

    print(f"Inserting into database...")
    conn = sqlite3.connect(str(db_path))
    cursor = conn.execute(
        "INSERT INTO meetings (filename, audio_data, duration_ms, size_bytes) "
        "VALUES (?, ?, ?, ?)",
        (filename, wav_bytes, duration_ms, size_bytes),
    )
    meeting_id = cursor.lastrowid
    conn.commit()
    conn.close()

    print(f"Imported as meeting ID {meeting_id}: {filename}")


if __name__ == "__main__":
    main()
