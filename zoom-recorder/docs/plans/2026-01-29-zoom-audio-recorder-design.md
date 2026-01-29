# Zoom Audio Recorder Design

Record Zoom calls on Linux with JIRA timestamp marking for transcript processing.

## Overview

A lightweight Python/Tkinter tool that:
- Records both sides of Zoom audio (mic + system) via FFmpeg
- Provides a small GUI for controlling recording and marking timestamps
- Associates timestamps with JIRA ticket numbers
- Outputs WAV files optimized for Whisper transcription

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    zoom-recorder                        │
├─────────────────────────────────────────────────────────┤
│  GUI (Tkinter)           │  Recorder (FFmpeg)           │
│  ┌───────────────────┐   │  ┌───────────────────────┐   │
│  │ [Start Recording] │   │  │ Captures mic +        │   │
│  │ [Stop Recording]  │   │  │ system audio via      │   │
│  │ ────────────────  │   │  │ PipeWire, mixed       │   │
│  │ JIRA: [GT-____]   │   │  │ in real-time          │   │
│  │ [Mark Timestamp]  │   │  │                       │   │
│  └───────────────────┘   │  │ Output: 16kHz mono WAV│   │
│                          │  └───────────────────────┘   │
├─────────────────────────────────────────────────────────┤
│  Output: ~/zoom-recordings/2026-01-29-143022/           │
│    ├── audio.wav                                        │
│    ├── timestamps.json                                  │
│    └── metadata.json                                    │
└─────────────────────────────────────────────────────────┘
```

### Components

- **GUI window** - Small, always-on-top window with start/stop and timestamp marking
- **Recorder process** - FFmpeg subprocess capturing and mixing audio sources
- **Timestamp manager** - Tracks marks with elapsed time and JIRA ticket
- **Output directory** - Created per recording with timestamp-based naming

### Data Flow

1. User clicks Start → spawns FFmpeg subprocess → creates output directory
2. User enters JIRA ticket, clicks Mark → records `{time: "00:05:23", ticket: "GT-1234"}`
3. User clicks Stop → terminates FFmpeg → writes timestamps.json and metadata.json

## GUI Design

### Window Specs

- Small fixed-size window (~300x200 pixels)
- Always-on-top so it's accessible during calls
- Minimal, stays out of the way

### Layout

```
┌─────────────────────────────────────┐
│ ● Zoom Recorder              ─ □ x │
├─────────────────────────────────────┤
│                                     │
│   [  Start Recording  ]             │
│                                     │
│   ⏱ 00:12:34    ● REC               │
│                                     │
│   ┌─────────────────────────┐       │
│   │ GT-1234                 │       │
│   └─────────────────────────┘       │
│   [  Mark Timestamp  ]              │
│                                     │
│   Last: 00:10:15 → GT-1230          │
│                                     │
└─────────────────────────────────────┘
```

### Behavior

- **Start Recording** button toggles to **Stop Recording** when active
- **Timer** shows elapsed recording time, updates every second
- **REC indicator** shows red dot when recording
- **JIRA field** accepts ticket numbers, remembers last entry
- **Mark Timestamp** captures current time + JIRA ticket, clears the field
- **Last mark display** shows confirmation of the last timestamp marked
- **Keyboard shortcut** - Enter in the JIRA field triggers Mark Timestamp

### State Handling

- Mark button disabled when not recording
- Confirm before stopping if recording is active
- JIRA field is optional - can mark without a ticket number

## Output Format

### Directory Structure

```
~/zoom-recordings/
└── 2026-01-29-143022/
    ├── audio.wav
    ├── timestamps.json
    └── metadata.json
```

### timestamps.json

```json
{
  "marks": [
    {"time": "00:02:15", "seconds": 135, "ticket": "GT-1234", "note": null},
    {"time": "00:08:42", "seconds": 522, "ticket": "GT-1235", "note": null},
    {"time": "00:15:00", "seconds": 900, "ticket": null, "note": null}
  ]
}
```

- `time` - Human-readable timestamp
- `seconds` - Raw seconds for programmatic use
- `ticket` - JIRA ticket or null if none entered
- `note` - Reserved for future use (free-text notes)

### metadata.json

```json
{
  "started_at": "2026-01-29T14:30:22+11:00",
  "ended_at": "2026-01-29T15:15:45+11:00",
  "duration_seconds": 2723,
  "sample_rate": 16000,
  "channels": 1,
  "format": "wav"
}
```

### Directory Naming

`YYYY-MM-DD-HHMMSS` based on recording start time. Simple, sortable, unique.

### Output Location

Default: `~/zoom-recordings/`

## Audio Capture

### FFmpeg with Simultaneous Capture

FFmpeg captures multiple PipeWire sources at once and mixes them in real-time:

```bash
ffmpeg -f pulse -i <mic-source> \
       -f pulse -i <monitor-source> \
       -filter_complex amix=inputs=2:duration=longest \
       -ar 16000 -ac 1 \
       audio.wav
```

This mixes both streams with proper timing as they're captured - no post-merge, no sync issues.

### Audio Sources

- **Microphone** - User's default input device
- **Monitor source** - PipeWire monitor of default output (captures Zoom audio)

### Output Format

- WAV format (native Whisper support)
- 16kHz sample rate (optimal for speech)
- Mono channel (speech doesn't need stereo)
- 16-bit signed integer samples

## Error Handling

### Startup Checks

- Verify FFmpeg is installed
- Verify PipeWire is running
- Detect available audio sources (mic + monitor)
- Fail with clear error message if any check fails

### During Recording

- If FFmpeg process dies unexpectedly → show error in GUI, save any partial audio
- If audio source disconnects → FFmpeg continues with silence

### User Errors

- Click Stop with no recording active → button is disabled, can't happen
- Close window while recording → prompt "Recording in progress. Stop and save?"
- No JIRA ticket entered when marking → allowed, saves with `ticket: null`

### File System

- Output directory already exists → append `-1`, `-2`, etc.
- Disk full → FFmpeg will error, catch and display

### Not Included (v1)

- No auto-recovery of crashed recordings
- No pause/resume
- No audio level monitoring

## Setup Script

Idempotent script to verify and install all dependencies.

### Behavior

```bash
#!/bin/bash
# setup.sh - Run with sudo

# Check/install:
# 1. Python 3 (usually pre-installed)
# 2. Tkinter (python3-tk package)
# 3. FFmpeg
# 4. PipeWire (verify running, not install)

# For each dependency:
# - Check if present
# - If missing, install via apt
# - If present, print "✓ already installed"

# Final step:
# - Verify PipeWire is running
# - List detected audio sources
```

### Output Format

```
Checking dependencies...
✓ Python 3.11.4 found
✓ Tkinter already installed
✓ FFmpeg 6.0 found
✓ PipeWire running

Audio sources detected:
  - Mic: alsa_input.usb-Blue_Yeti-00.analog-stereo
  - Monitor: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor

Setup complete!
```

## Technical Choices

- **Python + Tkinter** - No extra GUI dependencies, standard library
- **FFmpeg** - Reliable audio capture and mixing
- **PipeWire** - Modern Linux audio backend (already on system)
- **WAV 16kHz mono** - Optimal for Whisper transcription

## Dependencies

- Python 3 (standard library only - Tkinter included)
- FFmpeg
- PipeWire (already running)

## Not Included in v1

- Pause/resume
- Audio level meters
- Configurable output path
- Global keyboard hotkeys (just Enter in JIRA field)
