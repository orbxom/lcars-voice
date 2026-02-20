# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LCARS Voice is a Tauri v2 desktop application for voice recording and transcription. It combines a Rust backend with a vanilla JavaScript frontend styled after Star Trek's LCARS interface. Audio is captured via `cpal` (cross-platform), transcribed using `whisper-rs` (native whisper.cpp bindings), and results are copied to the clipboard.

## Build & Run Commands

```bash
# Development (from project root)
cd src-tauri && cargo tauri dev

# Production build (creates .deb and .AppImage)
cd src-tauri && cargo tauri build

# Run Rust tests
cd src-tauri && cargo test

# Run Rust checks only
cd src-tauri && cargo check

# Format Rust code
cd src-tauri && cargo fmt
```

## Architecture

```
Frontend (src/)           Backend (src-tauri/src/)
├── app.js         <-IPC->  ├── main.rs (commands, hotkey, tray, socket toggle)
├── index.html            ├── recording.rs (cpal audio capture + rubato resampling)
└── styles.css            ├── transcription.rs (whisper-rs native inference)
                          ├── model_manager.rs (GGML model download from HuggingFace)
                          └── database.rs (SQLite history)
```

**Data flow**: User triggers recording -> cpal captures audio to buffer -> User stops -> audio downmixed to mono, resampled to 16KHz -> whisper-rs transcribes -> Result copied to clipboard + saved to SQLite history.

**Key IPC commands** (defined in `main.rs`, called via `window.__TAURI__.core.invoke()`):
- `start_recording`, `stop_recording`
- `get_history`, `search_history`, `add_transcription`
- `get_whisper_model`, `set_whisper_model` (uses tauri-plugin-store)
- `is_model_downloaded`, `download_model` (model management)
- `get_audio_level` (real-time RMS for waveform UI)

**Global hotkey**: Super+Alt+H toggles recording on/off.

**External toggle**: Unix socket at `$XDG_RUNTIME_DIR/lcars-voice.sock`. Send "toggle" to start/stop recording from scripts.

## Key Paths

- History database: `~/.local/share/lcars-voice/history.db`
- Settings store: `~/.local/share/lcars-voice/settings.json` (whisper model preference)
- Whisper models: `~/.local/share/lcars-voice/models/ggml-{base,small,medium,large}.bin`
- Unix socket: `$XDG_RUNTIME_DIR/lcars-voice.sock`
- Whisper model: Configurable via UI dropdown (base, small, medium, large). Falls back to `WHISPER_MODEL` env var, then defaults to `base`. Models auto-download from HuggingFace on first use.

## Frontend Notes

- No build tooling - vanilla JS/HTML/CSS
- Antonio font self-hosted in `src/fonts/`
- LCARS color palette: Orange (#FF9900), Purple (#CC99CC), Blue (#9999FF), Tan (#FFCC99)
- Tauri events: `recording-started`, `transcribing`, `transcription-complete`, `transcription-error`, `model-download-progress`
- Logging uses `[LCARS]` prefix

## Dependencies

Rust crates (no external runtime dependencies):
- `cpal` -- cross-platform audio capture (ALSA/PulseAudio on Linux, CoreAudio on macOS, WASAPI on Windows)
- `rubato` -- audio resampling to 16KHz for Whisper
- `whisper-rs` -- native whisper.cpp bindings (bundles whisper.cpp, requires C++ compiler at build time)
- `tauri-plugin-notification` -- native desktop notifications
- `reqwest` -- HTTP client for model download
- `rusqlite` -- SQLite for transcription history

System: `xclip` (clipboard on Linux)

## Model Management

GGML model files are downloaded from HuggingFace on first use and cached locally. The `model_manager.rs` module handles download with progress events. Model sizes:
- base: ~75 MB
- small: ~500 MB
- medium: ~1.5 GB
- large: ~3.1 GB

## Threading

- Audio capture runs on cpal's callback thread, writing to `Arc<Mutex<Vec<f32>>>`
- Transcription runs on a `std::thread::spawn` blocking thread (whisper-rs state is not Send)
- WhisperContext is lazy-loaded and cached in `Arc<Mutex<Option<WhisperContext>>>`
- Unix socket toggle listener runs via `tauri::async_runtime::spawn`

## Desktop Notifications

Uses `tauri-plugin-notification` for native desktop notifications:
- **Recording started**: "LCARS Voice" / "Recording started"
- **Transcription complete**: "LCARS Voice" / First ~50 chars of transcribed text
- **Transcription error**: "LCARS Voice" / "Error: {message}"
