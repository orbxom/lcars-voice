# LCARS Voice

Desktop voice recording, meeting recording, and transcription app with a Star Trek LCARS-themed interface. Built with Rust and Tauri v2.

## Features

- **Voice Notes** — Record and instantly transcribe voice notes to clipboard using Whisper AI
- **Meeting Recording** — Record meetings with system audio capture and microphone
- **Meeting Transcription** — Transcribe recorded meetings with optional speaker diarization
- **Offline** — All transcription runs locally via whisper.cpp (no cloud APIs)
- **GPU Acceleration** — CUDA support for fast transcription on NVIDIA GPUs
- **Global Hotkey** — Super+Alt+H to toggle recording from anywhere
- **Model Selection** — Choose from Whisper base, small, medium, or large models

## Installation

### One-line install (Linux)

```bash
curl -sSL https://raw.githubusercontent.com/orbxom/lcars-voice/main/lcars-voice/install.sh | bash
```

This auto-detects your GPU and installs the appropriate variant (CPU or CUDA).

### Build from source

```bash
cd lcars-voice/src-tauri
cargo tauri dev      # development
cargo tauri build    # production (.deb and .AppImage)
```

Requires: Rust, Node.js, and system dependencies:
```bash
sudo apt install xclip libnotify-bin libwebkit2gtk-4.1-dev libayatana-appindicator3-dev
```

For CUDA builds, install the CUDA toolkit. For CPU-only: `cargo tauri build --no-default-features`.

## Usage

- **Super+Alt+H** — Toggle voice recording (records, transcribes, copies to clipboard)
- Switch between Voice Note and Meeting modes in the UI
- Select Whisper model size in the dropdown (base → large, larger = more accurate but slower)
- Meeting recordings are saved to a local SQLite database and can be transcribed later

## Requirements

- Linux (GNOME recommended for global hotkey support)
- Optional: NVIDIA GPU + CUDA for faster transcription
- Optional: Python 3 + pyannote.audio for speaker diarization in meetings

## Troubleshooting

### Super+Alt+H keybinding does nothing

The keybinding is a GNOME custom keybinding that runs `lcars-voice-toggle.sh`. There are two layers that can fail:

#### 1. GNOME custom keybindings stopped working entirely

**Symptom**: Super+Alt+H does nothing AND other custom keybindings (e.g. Super+Shift+S for Flameshot) also don't work.

**Cause**: `gsd-media-keys` (the GNOME daemon that handles custom keybindings) has crashed or become unresponsive.

**Fix**:
```bash
# Kill and restart gsd-media-keys
kill -9 $(pgrep gsd-media-keys)
sleep 2
/usr/libexec/gsd-media-keys &>/dev/null &
```

Test with any custom keybinding (e.g. Flameshot) to confirm they're working again.

#### 2. Toggle script fails to start the app

**Symptom**: Custom keybindings work (Flameshot fires) but LCARS Voice doesn't appear.

**Diagnosis**: Check the log file:
```bash
cat /tmp/lcars-toggle.log
```

**Common causes**:
- **Stale release binary**: The toggle script runs the pre-built binary at `src-tauri/target/release/lcars-voice`. If the source code has changed since the last build, rebuild:
  ```bash
  cd src-tauri && cargo tauri build
  ```
- **Stale socket**: A previous crash left `$XDG_RUNTIME_DIR/lcars-voice.sock` behind. The script handles this automatically, but you can manually clean up:
  ```bash
  rm -f "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"
  ```
- **Stale processes**: Old instances blocking the new one:
  ```bash
  pkill -f "target/release/lcars-voice"
  rm -f "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/lcars-voice.sock"
  ```

### Re-registering the keybinding

If the keybinding is missing from GNOME entirely:
```bash
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/binding "'<Super><Alt>h'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/command "'$HOME/.local/bin/lcars-voice-toggle'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/name "'LCARS Voice Toggle'"
```

Then ensure it's in the active list:
```bash
# Check current list
dconf read /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
# Add lcars-voice path if missing
```
