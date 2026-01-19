# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LCARS Voice is a Tauri v2 desktop application for voice recording and transcription. It combines a Rust backend with a vanilla JavaScript frontend styled after Star Trek's LCARS interface. Audio is recorded via Linux `arecord`, transcribed using OpenAI's Whisper via a Python wrapper, and results are copied to the clipboard.

## Build & Run Commands

```bash
# Development (from project root)
cd src-tauri && cargo tauri dev

# Production build (creates .deb and .AppImage)
cd src-tauri && cargo tauri build

# Run Rust checks only
cd src-tauri && cargo check

# Format Rust code
cd src-tauri && cargo fmt
```

## Architecture

```
Frontend (src/)           Backend (src-tauri/src/)       External
├── app.js         ←IPC→  ├── main.rs (commands,        Python
├── index.html            │    hotkey, tray)            └── scripts/whisper-wrapper.py
└── styles.css            ├── recording.rs (arecord)
                          ├── database.rs (SQLite)
                          └── transcription.rs (Python bridge)
```

**Data flow**: User triggers recording → Rust spawns `arecord` → User stops → Rust calls Python Whisper wrapper → Result returned to frontend → Copied to clipboard + saved to SQLite history.

**Key IPC commands** (defined in `main.rs`, called via `window.__TAURI__.core.invoke()`):
- `start_recording`, `stop_recording`, `cancel_recording`
- `transcribe_audio`
- `get_history`, `search_history`
- `copy_to_clipboard`
- `get_whisper_model`, `set_whisper_model` (uses tauri-plugin-store)

**Global hotkey**: Super+Alt+H toggles recording on/off. Use `scripts/install-keybinding.sh` to set up Super+H as a system keybinding to launch the app.

## Key Paths

- Audio temp file: `/tmp/lcars-voice-recording.wav`
- History database: `~/.local/share/lcars-voice/history.db`
- Settings store: `~/.local/share/lcars-voice/settings.json` (whisper model preference)
- Whisper model: Configurable via UI dropdown (base, small, medium, large). Falls back to `WHISPER_MODEL` env var, then defaults to `base`

## Frontend Notes

- No build tooling - vanilla JS/HTML/CSS
- LCARS color palette: Orange (#FF9900), Purple (#CC99CC), Blue (#9999FF), Tan (#FFCC99)
- Tauri events to listen for: `recording-started`, `transcribing`, `transcription-complete`, `transcription-error`
- Logging uses `[LCARS]` prefix

## Linux Dependencies

Requires: `alsa-utils` (for `arecord`), `xclip`, `libnotify-bin` (for `notify-send`), Python 3 with `openai-whisper` package in virtualenv at `~/voice-to-text-env`.

## GPU Acceleration

The Python whisper wrapper (`scripts/whisper-wrapper.py`) automatically uses CUDA if available. Requires PyTorch with CUDA support in the virtualenv.

## Threading Constraints

The Tauri store (tauri-plugin-store) is not thread-safe. Always call `app.store()` from the main thread BEFORE spawning threads, then pass the result to the thread. See `get_current_model()` pattern in `main.rs`.

## Desktop Notifications

System notifications are sent for recording events:
- **Recording started**: "LCARS Voice" / "Recording started"
- **Transcription complete**: "LCARS Voice" / First ~50 chars of transcribed text
- **Transcription error**: "LCARS Voice" / "Error: {message}"

Notifications use the `notify-send` command directly rather than the notify-rust library, as the library has D-Bus threading issues when called from Tauri's spawned threads.
