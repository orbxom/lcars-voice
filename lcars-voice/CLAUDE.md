# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LCARS Voice is a Tauri v2 desktop application for voice recording, meeting recording, and transcription. It combines a Rust backend with a vanilla JavaScript frontend styled after Star Trek's LCARS interface. Audio is captured via `cpal` (cross-platform), transcribed using `whisper-rs` (native whisper.cpp bindings), and results are copied to the clipboard. Meeting recordings are stored in SQLite and can be transcribed with optional speaker diarization via pyannote.

## Build & Run Commands

```bash
# Development (from project root)
cd src-tauri && cargo tauri dev

# Production build (creates .deb and .AppImage)
cd src-tauri && cargo tauri build

# Install after build
sudo dpkg -i "src-tauri/target/release/bundle/deb/LCARS Voice_0.2.0_amd64.deb"

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
├── app.test.js           ├── recording.rs (cpal audio capture + rubato resampling)
├── index.html            ├── transcription.rs (whisper-rs native inference)
└── styles.css            ├── meeting.rs (meeting session management, WAV encoding via hound)
                          ├── meeting_transcription.rs (transcription pipeline: hallucination filtering, diarization, segment merging)
                          ├── audio_sources.rs (cpal device enumeration, monitor source detection)
                          ├── model_manager.rs (GGML model download from HuggingFace)
                          └── database.rs (SQLite history + meetings)

Scripts (scripts/)
├── import-meeting.py      # Import external audio files into meeting database
├── install-desktop.sh     # Install .desktop file for app launcher
├── install-keybinding.sh  # Install GNOME keybinding for socket toggle
└── lcars-launch.sh        # Launch script for desktop entry
```

**Voice note flow**: User triggers recording -> cpal captures audio to buffer -> User stops -> audio downmixed to mono, resampled to 16KHz -> whisper-rs transcribes -> Result copied to clipboard + saved to SQLite history.

**Meeting flow**: User starts meeting recording -> cpal captures audio -> User stops -> audio encoded to 16KHz mono WAV via hound -> WAV stored as BLOB in SQLite `meetings` table -> User can later trigger transcription -> whisper-rs transcribes segments -> optional pyannote diarization assigns speakers -> hallucination filtering and segment merging -> formatted transcript saved to meeting record.

**Key IPC commands** (defined in `main.rs`, called via `window.__TAURI__.core.invoke()`):
- `start_recording`, `stop_recording`
- `get_history`, `search_history`, `add_transcription`
- `get_whisper_model`, `set_whisper_model` (uses tauri-plugin-store)
- `is_model_downloaded`, `download_model` (model management)
- `get_audio_level` (real-time RMS for waveform UI)
- `get_recording_mode`, `set_recording_mode` (Voice Note / Meeting)
- `get_waveform_data` (real-time waveform samples for oscilloscope visualization)
- `list_audio_sources` (enumerate cpal input devices with monitor detection)
- `get_elapsed_time` (elapsed recording seconds)
- `get_meeting_history` (list meeting recordings from database)
- `rename_meeting` (rename a meeting recording)
- `transcribe_meeting` (run full transcription pipeline on a stored meeting)

**Global hotkey**: Super+Alt+H toggles recording on/off.

**External toggle**: Unix socket at `$XDG_RUNTIME_DIR/lcars-voice.sock`. Send "toggle" to start/stop recording from scripts.

## Key Paths

- History database: `~/.local/share/lcars-voice/history.db`
- Settings store: `~/.local/share/lcars-voice/settings.json` (whisper model preference)
- Whisper models: `~/.local/share/lcars-voice/models/ggml-{base,small,medium,large}.bin`
- Meeting recordings: Stored as WAV BLOBs in `~/.local/share/lcars-voice/history.db` (`meetings` table)
- Unix socket: `$XDG_RUNTIME_DIR/lcars-voice.sock`
- Log files: `~/.local/share/lcars-voice/logs/lcars-voice-YYYY-MM-DD.log` (daily rotation, 14-day retention)
- Whisper model: Configurable via UI dropdown (base, small, medium, large). Falls back to `WHISPER_MODEL` env var, then defaults to `base`. Models auto-download from HuggingFace on first use.

## Frontend Notes

- No build tooling - vanilla JS/HTML/CSS
- Antonio font self-hosted in `src/fonts/`
- LCARS color palette: Orange (#FF9900), Purple (#CC99CC), Blue (#9999FF), Tan (#FFCC99)
- Tauri events: `recording-started`, `transcribing`, `transcription-complete`, `transcription-error`, `model-download-progress`, `meeting-saved`, `meeting-transcription-progress`, `meeting-transcription-complete`
- Logging: Uses `log` + `fern` crates for dual output (stderr with `[LCARS]` prefix + daily log file). Levels: error/warn/info/debug.

## Dependencies

Rust crates (no external runtime dependencies):
- `cpal` -- cross-platform audio capture (ALSA/PulseAudio on Linux, CoreAudio on macOS, WASAPI on Windows)
- `rubato` -- audio resampling to 16KHz for Whisper
- `whisper-rs` -- native whisper.cpp bindings (bundles whisper.cpp, requires C++ compiler at build time)
- `hound` -- WAV file encoding/decoding (meeting audio)
- `log` -- facade for structured logging (error/warn/info/debug macros)
- `fern` -- logging dispatcher for dual output (stderr + daily log file)
- `rusqlite` -- SQLite for transcription history and meeting storage
- `reqwest` -- HTTP client for model download
- `chrono` -- date/time handling for meeting sessions
- `tokio` -- async runtime for socket listener and async commands
- `serde` + `serde_json` -- serialization for IPC and settings
- `dirs` -- platform-specific directory paths (XDG on Linux)
- `tauri-plugin-global-shortcut` -- global hotkey registration (Super+Alt+H)
- `tauri-plugin-clipboard-manager` -- clipboard write access
- `tauri-plugin-store` -- persistent JSON settings storage
- `tauri-plugin-notification` -- registered for capabilities (notifications sent via `notify-send` instead)

System: `xclip` (clipboard on Linux)

## Feature Flags

- `cuda` (default: enabled) -- GPU-accelerated Whisper inference via `whisper-rs/cuda`. Requires CUDA toolkit at build time. Disable with `cargo tauri build --no-default-features` for CPU-only builds.

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
- Meeting transcription runs on a `tokio::task::spawn_blocking` thread (whisper-rs inference + optional pyannote subprocess)
- Unix socket toggle listener runs via `tauri::async_runtime::spawn`

## Desktop Notifications

Uses `notify-send` (shelled out via `std::process::Command`) for native desktop notifications. The Tauri notification plugin (`tauri-plugin-notification`) is still registered but not used for sending — its `notify-rust` backend silently drops notifications on some Linux setups.

Notifications are sent on a background thread to avoid blocking the main/hotkey thread.

- **Recording started**: "LCARS Voice" / "Recording started" (or "Meeting recording started")
- **Transcribing**: "LCARS Voice" / "Recording stopped, transcribing..."
- **Transcription complete**: "LCARS Voice" / First ~50 chars of transcribed text
- **Transcription error**: "LCARS Voice" / "Error: {message}"
- **Meeting saved**: "LCARS Voice" / "Meeting saved to {path}"

System dependency: `notify-send` (from `libnotify-bin`, typically pre-installed on Linux desktops).

## Speaker Diarization (Optional)

Meeting transcriptions can include speaker labels via pyannote. Requires:
- Python 3 with `pyannote.audio` installed. Auto-detects `~/voice-to-text-env/bin/python` if it exists, or set `PYTHON_ENV` to a custom path.
- `HF_TOKEN` environment variable set with a HuggingFace token that has access to `pyannote/speaker-diarization-3.1`

If not available, transcription proceeds without speaker labels.

## Post-Build Workflow

After running `cargo tauri build`, always show the user the install command:
```
sudo dpkg -i "src-tauri/target/release/bundle/deb/LCARS Voice_0.2.0_amd64.deb"
```
Note: Update the version in the path if `Cargo.toml` version changes.
