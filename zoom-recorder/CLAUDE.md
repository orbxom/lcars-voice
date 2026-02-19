# CLAUDE.md

This file provides guidance to Claude Code when working with code in this project.

## Project Overview

Tkinter GUI application for recording Zoom calls on Linux with JIRA timestamp marking. Captures both microphone and system audio via PipeWire/FFmpeg, saves as WAV, and stores JIRA ticket timestamps for downstream transcript processing by [meeting-transcripts](../meeting-transcripts/).

## Running

```bash
# Run the app
python3 -m src

# Run tests
python3 -m pytest tests/ -v

# First-time setup (installs system deps)
./setup.sh
```

## Architecture

```
src/
├── __main__.py      Entry point
├── gui.py           Tkinter GUI (300x220, always-on-top)
├── audio.py         Audio source detection via pactl
├── recorder.py      FFmpeg subprocess (mic + monitor → 16kHz mono WAV)
├── timestamps.py    JIRA ticket mark tracking
└── metadata.py      Recording metadata writer
```

**Data flow:** User clicks Start → `audio.detect_sources()` finds mic + monitor → `Recorder.start()` spawns FFmpeg with amix filter → User marks JIRA tickets during call → User clicks Stop → `Recorder.stop()` terminates FFmpeg → timestamps.json + metadata.json written alongside audio.wav.

## Output

Each recording session creates a timestamped directory in `~/zoom-recordings/`:

```
~/zoom-recordings/2026-02-19-093015/
├── audio.wav          16kHz mono WAV
├── timestamps.json    JIRA ticket marks with time positions
└── metadata.json      Recording metadata (duration, format, etc.)
```

This output is consumed by `meeting-transcripts/process-local-recordings.sh`.

## Key Conventions

- Audio: Always 16kHz, mono, WAV format
- FFmpeg uses PipeWire pulse sources (mic + monitor mixed via amix filter)
- `Recorder.stop()` sends SIGTERM first, falls back to SIGKILL after timeout
- GUI uses Enter key in JIRA field to trigger timestamp marking
- All tests mock subprocess calls — no real audio capture in tests

## Dependencies

System: `python3-tk`, `ffmpeg`, `pipewire-pulse`, `pulseaudio-utils` (for `pactl`). No virtualenv needed — uses system Python.
