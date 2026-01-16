# LCARS Voice Interface - Design Document

## Overview

Transform the existing voice-to-text bash scripts into a polished desktop application with an authentic Star Trek LCARS aesthetic. The app provides voice-to-text transcription via OpenAI Whisper with a system tray icon, global hotkey, and optional floating window.

## Goals

- Keep existing hotkey workflow (Super+H toggle)
- Add system tray icon showing recording state
- Create LCARS-styled floating window with waveform, record button, and searchable history
- Package for easy installation (.deb)
- Persistent transcription history

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri App (Rust)                     │
│  - System tray icon                                     │
│  - Window management                                    │
│  - Global hotkey listener (Super+H)                     │
│  - Spawns/manages recording subprocess                  │
│  - SQLite database for history                          │
└──────────────────────┬──────────────────────────────────┘
                       │ IPC
┌──────────────────────▼──────────────────────────────────┐
│                  Web UI (HTML/CSS/JS)                   │
│  - LCARS-styled floating window                         │
│  - Real-time waveform canvas                            │
│  - History list with search                             │
│  - Record button                                        │
└─────────────────────────────────────────────────────────┘
                       │ Shell/subprocess
┌──────────────────────▼──────────────────────────────────┐
│               Python/Whisper (existing)                 │
│  - Audio recording (arecord)                            │
│  - Transcription (whisper)                              │
│  - Returns text to Tauri via stdout                     │
└─────────────────────────────────────────────────────────┘
```

**Key principle:** The Rust backend owns all state. The web UI is purely for display. Existing Python/Whisper code runs as a subprocess.

## Technology Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| App framework | Tauri 2.x | Small bundle (~10-15MB), uses system webview, Rust backend |
| Frontend | HTML/CSS/JS (vanilla) | Full styling control for LCARS, no framework overhead |
| Database | SQLite | Fast search, FTS5 support, crash-safe |
| Audio | arecord (existing) | Already works, no changes needed |
| Transcription | Whisper (existing) | Already works, called as subprocess |
| Packaging | .deb + AppImage | Native Debian install + portable fallback |

## UI Design

### Window Specifications

- **Size:** 420x520px (fixed, non-resizable)
- **Style:** Frameless (no OS title bar)
- **Behavior:** Always on top (toggleable), draggable by header

### LCARS Visual Language

**Color palette:**
- Orange: `#FF9900` (primary actions, recording)
- Purple: `#CC99CC` (secondary elements)
- Blue: `#9999FF` (information, borders)
- Tan: `#FFCC99` (accents, dividers)
- Background: `#000000` (true black)

**Typography:**
- Display: Antonio (free, mimics Swiss 911)
- Mono: Orbitron (for stardates, timestamps)

**Signature elements:**
- Compound-curve SVG elbows (authentic, not border-radius fakes)
- Decorative sidebar blocks with random numbers
- Animated scan-lines during recording
- Pulsing status indicators

### Layout

```
┌──────────────────────────────────────────────────────┐
│ [ELBOW]  ████████████████  VOICE INTERFACE  [ELBOW]  │
│ [SIDEBAR]                                  [SIDEBAR] │
│ │  47  │  STATUS: READY          ●                   │
│ │ 215  │  ┌─────────────────────────────┐            │
│ │  08  │  │      WAVEFORM CANVAS        │            │
│ │      │  └─────────────────────────────┘            │
│ │ 1701 │  [ ● RECORD ]    SUPER+H                    │
│ │  93  │  ━━━ TRANSCRIPTION LOG ━━━━━━━              │
│ │      │  🔍 Search...                               │
│ │      │  ├─ "Set up the meeting..."  ⧉             │
│ │      │  ├─ "Remember to call..."    ⧉             │
│ │      │  └─ "The quick brown..."     ⧉             │
│ [ELBOW]  ████  LCARS v2.47 | WHISPER: BASE  [ELBOW]  │
└──────────────────────────────────────────────────────┘
```

### UI States

| State | Status Indicator | Waveform | Record Button | Tray Icon |
|-------|------------------|----------|---------------|-----------|
| Idle | Green dot, "READY" | Idle sine wave | "RECORD" (orange) | Monochrome |
| Recording | Red pulse, "RECORDING" | Live audio | "STOP" (red) | Red dot overlay |
| Transcribing | Orange pulse, "TRANSCRIBING" | Frozen | "PROCESSING" (disabled) | Orange spinner |

## Data Storage

**Location:** `~/.local/share/lcars-voice/history.db`

**Schema:**
```sql
CREATE TABLE transcriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text TEXT NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    duration_ms INTEGER,
    model TEXT DEFAULT 'base'
);

CREATE VIRTUAL TABLE transcriptions_fts USING fts5(text, content=transcriptions);
```

**Retention:** Keep last 100 entries (configurable)

## System Tray

**Icon states:**
- Idle: Monochrome LCARS-style icon
- Recording: Red dot overlay, subtle pulse
- Transcribing: Orange activity indicator

**Interactions:**
- Left-click: Toggle window visibility
- No right-click menu (minimal approach)

## Global Hotkey

**Shortcut:** Super+H (configurable)

**Behavior:**
- Registered via Tauri `global_shortcut` API
- Works regardless of window state
- Toggle: Idle → Recording → Transcribing → Idle

**State machine:**
```
[Idle] ──Super+H──▶ [Recording] ──Super+H──▶ [Transcribing] ──auto──▶ [Idle]
                                                    │
                                            copy to clipboard
```

## Packaging

### Primary: Debian Package (.deb)

**Contents:**
```
/usr/bin/lcars-voice
/usr/share/applications/lcars-voice.desktop
/usr/share/icons/hicolor/*/apps/lcars-voice.png
/usr/share/lcars-voice/whisper-wrapper.py
```

**Dependencies:**
- python3 (>= 3.10)
- python3-venv
- alsa-utils
- xclip

**Post-install:** Creates Python venv at `~/.local/share/lcars-voice/venv`, installs whisper

### Secondary: AppImage

Single portable file for non-Debian distributions.

## File Structure

```
lcars-voice/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs           # Entry point, tray, hotkey
│       ├── recording.rs      # Audio recording management
│       ├── transcription.rs  # Whisper subprocess
│       └── database.rs       # SQLite history
├── src/
│   ├── index.html
│   ├── styles.css            # LCARS styling
│   └── app.js                # UI logic, IPC
├── scripts/
│   └── whisper-wrapper.py    # Simplified whisper interface
└── packaging/
    ├── lcars-voice.desktop
    └── icons/
```

## Implementation Notes

### Whisper Integration

Keep it simple: shell out to whisper CLI rather than embedding Python.

```rust
// In transcription.rs
let output = Command::new("python3")
    .args(["-c", "import whisper; ..."])
    .output()?;
```

### Audio Recording

Use arecord directly, stream to temp file:

```rust
let mut child = Command::new("arecord")
    .args(["-f", "S16_LE", "-r", "16000", "-c", "1", &temp_path])
    .spawn()?;
```

### IPC Events

Tauri events between Rust and JS:

- `recording-started` → UI shows recording state
- `recording-stopped` → UI shows transcribing state
- `transcription-complete { text }` → UI updates history
- `transcription-error { message }` → UI shows error

## Future Considerations (Not in v1)

- Multiple language support
- Custom hotkey configuration
- Theme variations (different LCARS eras)
- Audio playback of recordings
- Cloud sync of history

## Prototype

A working HTML/CSS/JS prototype exists at:
`voice-to-text/ui-prototype/index.html`

Open in browser to preview the LCARS interface design.
