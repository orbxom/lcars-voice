# LCARS Voice Interface Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Tauri desktop app with LCARS-themed UI for voice-to-text transcription using Whisper.

**Architecture:** Tauri (Rust) backend handles system tray, global hotkey, SQLite history, and spawns arecord/whisper as subprocesses. Web frontend displays LCARS UI with waveform visualization and transcription history. Communication via Tauri IPC events.

**Tech Stack:** Tauri 2.x, Rust, HTML/CSS/JS (vanilla), SQLite, arecord, openai-whisper (Python)

---

## Prerequisites

Before starting, ensure these are installed:
```bash
# Tauri dependencies
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

# Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tauri CLI
cargo install tauri-cli
```

---

## Task 1: Initialize Tauri Project

**Files:**
- Create: `lcars-voice/` (new directory)
- Create: `lcars-voice/src-tauri/Cargo.toml`
- Create: `lcars-voice/src-tauri/tauri.conf.json`
- Create: `lcars-voice/src-tauri/src/main.rs`
- Create: `lcars-voice/src/index.html`

**Step 1: Create project directory structure**

```bash
mkdir -p lcars-voice/src-tauri/src lcars-voice/src
```

**Step 2: Create Cargo.toml**

Create `lcars-voice/src-tauri/Cargo.toml`:

```toml
[package]
name = "lcars-voice"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-shell = "2"
tauri-plugin-clipboard-manager = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
```

**Step 3: Create build.rs**

Create `lcars-voice/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

**Step 4: Create tauri.conf.json**

Create `lcars-voice/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "LCARS Voice",
  "version": "0.1.0",
  "identifier": "com.lcars.voice",
  "build": {
    "frontendDist": "../src"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "LCARS Voice Interface",
        "width": 420,
        "height": 520,
        "resizable": false,
        "decorations": false,
        "transparent": false,
        "alwaysOnTop": true,
        "visible": true
      }
    ],
    "trayIcon": {
      "iconPath": "icons/tray-idle.png",
      "iconAsTemplate": true
    }
  },
  "bundle": {
    "active": true,
    "targets": ["deb", "appimage"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "linux": {
      "deb": {
        "depends": ["python3", "python3-venv", "alsa-utils", "xclip"]
      }
    }
  }
}
```

**Step 5: Create minimal main.rs**

Create `lcars-voice/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 6: Create placeholder index.html**

Create `lcars-voice/src/index.html`:

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>LCARS Voice</title>
  <style>
    body { background: #000; color: #FF9900; font-family: sans-serif; }
  </style>
</head>
<body>
  <h1>LCARS Voice Interface</h1>
  <p>Tauri is working!</p>
</body>
</html>
```

**Step 7: Create placeholder icons**

```bash
mkdir -p lcars-voice/src-tauri/icons
# Create a simple placeholder icon (32x32 orange square)
convert -size 32x32 xc:'#FF9900' lcars-voice/src-tauri/icons/32x32.png
convert -size 128x128 xc:'#FF9900' lcars-voice/src-tauri/icons/128x128.png
cp lcars-voice/src-tauri/icons/32x32.png lcars-voice/src-tauri/icons/tray-idle.png
cp lcars-voice/src-tauri/icons/128x128.png lcars-voice/src-tauri/icons/icon.icns
cp lcars-voice/src-tauri/icons/128x128.png lcars-voice/src-tauri/icons/icon.ico
```

Note: If `convert` (ImageMagick) isn't available, create any 32x32 and 128x128 PNG files manually.

**Step 8: Verify project builds**

```bash
cd lcars-voice && cargo tauri build --debug
```

Expected: Build succeeds, creates debug binary

**Step 9: Run development server**

```bash
cd lcars-voice && cargo tauri dev
```

Expected: Window opens showing "LCARS Voice Interface" heading

**Step 10: Commit**

```bash
git add lcars-voice/
git commit -m "feat: initialize Tauri project structure"
```

---

## Task 2: Implement System Tray

**Files:**
- Modify: `lcars-voice/src-tauri/src/main.rs`
- Create: `lcars-voice/src-tauri/icons/tray-recording.png`

**Step 1: Create recording tray icon**

```bash
convert -size 32x32 xc:'#CC4444' lcars-voice/src-tauri/icons/tray-recording.png
```

**Step 2: Add tray to main.rs**

Replace `lcars-voice/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Load tray icons
            let idle_icon = Image::from_path("icons/tray-idle.png")
                .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/tray-idle.png")).unwrap());

            // Build tray
            let _tray = TrayIconBuilder::new()
                .icon(idle_icon)
                .tooltip("LCARS Voice - Ready")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Test tray functionality**

```bash
cd lcars-voice && cargo tauri dev
```

Expected: Tray icon appears. Left-click toggles window visibility.

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add system tray with window toggle"
```

---

## Task 3: Implement Global Hotkey

**Files:**
- Modify: `lcars-voice/src-tauri/src/main.rs`
- Modify: `lcars-voice/src-tauri/Cargo.toml` (already has plugin)

**Step 1: Add global shortcut plugin to main.rs**

Update `lcars-voice/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

fn main() {
    let is_recording = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(move |app| {
            let recording_state = is_recording.clone();
            let app_handle = app.handle().clone();

            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                let was_recording = recording_state.fetch_xor(true, Ordering::SeqCst);
                if was_recording {
                    // Was recording, now stopping
                    let _ = app_handle.emit("recording-stopped", ());
                } else {
                    // Was idle, now starting
                    let _ = app_handle.emit("recording-started", ());
                }
            })?;

            // Tray setup
            let idle_icon = Image::from_bytes(include_bytes!("../icons/tray-idle.png"))?;

            let _tray = TrayIconBuilder::new()
                .icon(idle_icon)
                .tooltip("LCARS Voice - Ready")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 2: Update index.html to listen for events**

Update `lcars-voice/src/index.html`:

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>LCARS Voice</title>
  <style>
    body { background: #000; color: #FF9900; font-family: sans-serif; padding: 20px; }
    .recording { color: #CC4444; }
  </style>
</head>
<body>
  <h1>LCARS Voice Interface</h1>
  <p id="status">Status: Ready</p>
  <script>
    const { listen } = window.__TAURI__.event;

    listen('recording-started', () => {
      document.getElementById('status').textContent = 'Status: RECORDING';
      document.getElementById('status').classList.add('recording');
    });

    listen('recording-stopped', () => {
      document.getElementById('status').textContent = 'Status: Processing...';
      document.getElementById('status').classList.remove('recording');
    });
  </script>
</body>
</html>
```

**Step 3: Test hotkey**

```bash
cd lcars-voice && cargo tauri dev
```

Expected: Press Super+H, status changes to "RECORDING". Press again, status shows "Processing...".

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add global Super+H hotkey for recording toggle"
```

---

## Task 4: Implement SQLite History Database

**Files:**
- Create: `lcars-voice/src-tauri/src/database.rs`
- Modify: `lcars-voice/src-tauri/src/main.rs`

**Step 1: Create database module**

Create `lcars-voice/src-tauri/src/database.rs`:

```rust
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub id: i64,
    pub text: String,
    pub timestamp: String,
    pub duration_ms: Option<i64>,
    pub model: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS transcriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER,
                model TEXT DEFAULT 'base'
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    fn get_db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lcars-voice")
            .join("history.db")
    }

    pub fn add_transcription(
        &self,
        text: &str,
        duration_ms: Option<i64>,
        model: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO transcriptions (text, duration_ms, model) VALUES (?1, ?2, ?3)",
            params![text, duration_ms, model],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_history(&self, limit: usize) -> Result<Vec<Transcription>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, timestamp, duration_ms, model
             FROM transcriptions
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok(Transcription {
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                duration_ms: row.get(3)?,
                model: row.get(4)?,
            })
        })?;

        rows.collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Transcription>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, text, timestamp, duration_ms, model
             FROM transcriptions
             WHERE text LIKE ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(Transcription {
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                duration_ms: row.get(3)?,
                model: row.get(4)?,
            })
        })?;

        rows.collect()
    }
}
```

**Step 2: Add module to main.rs and create Tauri commands**

Update `lcars-voice/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod database;

use database::{Database, Transcription};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

struct AppState {
    db: Mutex<Database>,
    is_recording: AtomicBool,
}

#[tauri::command]
fn get_history(state: State<AppState>, limit: Option<usize>) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_history(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
fn search_history(state: State<AppState>, query: String, limit: Option<usize>) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.search(&query, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_transcription(state: State<AppState>, text: String, duration_ms: Option<i64>, model: Option<String>) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_transcription(&text, duration_ms, &model.unwrap_or_else(|| "base".to_string()))
        .map_err(|e| e.to_string())
}

fn main() {
    let db = Database::new().expect("Failed to initialize database");
    let app_state = AppState {
        db: Mutex::new(db),
        is_recording: AtomicBool::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![get_history, search_history, add_transcription])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                let state: State<AppState> = app_handle.state();
                let was_recording = state.is_recording.fetch_xor(true, Ordering::SeqCst);
                if was_recording {
                    let _ = app_handle.emit("recording-stopped", ());
                } else {
                    let _ = app_handle.emit("recording-started", ());
                }
            })?;

            // Tray setup
            let idle_icon = Image::from_bytes(include_bytes!("../icons/tray-idle.png"))?;

            let _tray = TrayIconBuilder::new()
                .icon(idle_icon)
                .tooltip("LCARS Voice - Ready")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Test database commands**

```bash
cd lcars-voice && cargo tauri dev
```

In browser console (F12):
```javascript
await window.__TAURI__.core.invoke('add_transcription', { text: 'Test entry', durationMs: 1000 });
await window.__TAURI__.core.invoke('get_history', { limit: 10 });
```

Expected: Returns array with the test entry.

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add SQLite database for transcription history"
```

---

## Task 5: Implement Audio Recording

**Files:**
- Create: `lcars-voice/src-tauri/src/recording.rs`
- Modify: `lcars-voice/src-tauri/src/main.rs`

**Step 1: Create recording module**

Create `lcars-voice/src-tauri/src/recording.rs`:

```rust
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub struct Recorder {
    process: Option<Child>,
    output_path: PathBuf,
}

impl Recorder {
    pub fn new() -> Self {
        let output_path = std::env::temp_dir().join("lcars-voice-recording.wav");
        Self {
            process: None,
            output_path,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.process.is_some() {
            return Err("Already recording".to_string());
        }

        // Remove old recording if exists
        let _ = std::fs::remove_file(&self.output_path);

        let child = Command::new("arecord")
            .args([
                "-f", "S16_LE",
                "-r", "16000",
                "-c", "1",
                self.output_path.to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start arecord: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<PathBuf, String> {
        if let Some(mut child) = self.process.take() {
            // Send SIGTERM to stop recording gracefully
            let _ = child.kill();
            let _ = child.wait();

            // Small delay to ensure file is flushed
            std::thread::sleep(std::time::Duration::from_millis(100));

            if self.output_path.exists() {
                Ok(self.output_path.clone())
            } else {
                Err("Recording file not found".to_string())
            }
        } else {
            Err("Not recording".to_string())
        }
    }

    pub fn is_recording(&self) -> bool {
        self.process.is_some()
    }
}
```

**Step 2: Integrate recorder into main.rs**

Update the `AppState` and setup in `lcars-voice/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod database;
mod recording;

use database::{Database, Transcription};
use recording::Recorder;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
}

#[tauri::command]
fn get_history(state: State<AppState>, limit: Option<usize>) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_history(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
fn search_history(state: State<AppState>, query: String, limit: Option<usize>) -> Result<Vec<Transcription>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.search(&query, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_transcription(state: State<AppState>, text: String, duration_ms: Option<i64>, model: Option<String>) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_transcription(&text, duration_ms, &model.unwrap_or_else(|| "base".to_string()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn start_recording(state: State<AppState>) -> Result<(), String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    recorder.start()
}

#[tauri::command]
fn stop_recording(state: State<AppState>) -> Result<String, String> {
    let mut recorder = state.recorder.lock().map_err(|e| e.to_string())?;
    let path = recorder.stop()?;
    Ok(path.to_string_lossy().to_string())
}

fn main() {
    let db = Database::new().expect("Failed to initialize database");
    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(Recorder::new()),
        is_recording: AtomicBool::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_history,
            search_history,
            add_transcription,
            start_recording,
            stop_recording
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Register Super+H hotkey
            let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyH);
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                let state: State<AppState> = app_handle.state();
                let was_recording = state.is_recording.fetch_xor(true, Ordering::SeqCst);

                if was_recording {
                    // Stop recording
                    if let Ok(mut recorder) = state.recorder.lock() {
                        if let Ok(audio_path) = recorder.stop() {
                            let _ = app_handle.emit("recording-stopped", audio_path.to_string_lossy().to_string());
                        }
                    }
                } else {
                    // Start recording
                    if let Ok(mut recorder) = state.recorder.lock() {
                        if recorder.start().is_ok() {
                            let _ = app_handle.emit("recording-started", ());
                        }
                    }
                }
            })?;

            // Tray setup
            let idle_icon = Image::from_bytes(include_bytes!("../icons/tray-idle.png"))?;

            let _tray = TrayIconBuilder::new()
                .icon(idle_icon)
                .tooltip("LCARS Voice - Ready")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Test recording**

```bash
cd lcars-voice && cargo tauri dev
```

Press Super+H to start, speak, press Super+H again. Check `/tmp/lcars-voice-recording.wav` exists.

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add audio recording via arecord"
```

---

## Task 6: Implement Whisper Transcription

**Files:**
- Create: `lcars-voice/src-tauri/src/transcription.rs`
- Create: `lcars-voice/scripts/whisper-wrapper.py`
- Modify: `lcars-voice/src-tauri/src/main.rs`

**Step 1: Create whisper wrapper script**

Create `lcars-voice/scripts/whisper-wrapper.py`:

```python
#!/usr/bin/env python3
"""Simple whisper wrapper that outputs transcription to stdout."""

import sys
import whisper
import json

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "No audio file provided"}))
        sys.exit(1)

    audio_path = sys.argv[1]
    model_name = sys.argv[2] if len(sys.argv) > 2 else "base"

    try:
        model = whisper.load_model(model_name)
        result = model.transcribe(audio_path, language="en")
        print(json.dumps({
            "text": result["text"].strip(),
            "language": result.get("language", "en")
        }))
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
```

**Step 2: Create transcription module**

Create `lcars-voice/src-tauri/src/transcription.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: Option<String>,
    pub language: Option<String>,
    pub error: Option<String>,
}

pub fn transcribe(audio_path: &Path, model: &str, venv_path: &Path) -> Result<String, String> {
    let python_path = venv_path.join("bin").join("python3");

    // Get the whisper wrapper script path
    let script_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|p| p.join("scripts").join("whisper-wrapper.py"))
        .unwrap_or_else(|| Path::new("scripts/whisper-wrapper.py").to_path_buf());

    let output = Command::new(&python_path)
        .args([
            script_path.to_str().unwrap(),
            audio_path.to_str().unwrap(),
            model,
        ])
        .output()
        .map_err(|e| format!("Failed to run whisper: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: TranscriptionResult = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse whisper output: {} - raw: {}", e, stdout))?;

    if let Some(error) = result.error {
        return Err(error);
    }

    result.text.ok_or_else(|| "No transcription text".to_string())
}
```

**Step 3: Integrate transcription into main.rs**

Add to imports and commands in `lcars-voice/src-tauri/src/main.rs`:

```rust
mod transcription;

// Add venv_path to AppState
struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    venv_path: PathBuf,
    model: String,
}

#[tauri::command]
async fn transcribe_audio(state: State<'_, AppState>, audio_path: String) -> Result<String, String> {
    let path = std::path::Path::new(&audio_path);
    let venv = state.venv_path.clone();
    let model = state.model.clone();

    transcription::transcribe(path, &model, &venv)
}
```

Update the main function initialization:

```rust
fn main() {
    let db = Database::new().expect("Failed to initialize database");
    let venv_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-to-text-env");

    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(Recorder::new()),
        is_recording: AtomicBool::new(false),
        venv_path,
        model: std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string()),
    };
    // ... rest of setup
}
```

Add `transcribe_audio` to invoke_handler.

**Step 4: Test transcription**

```bash
cd lcars-voice && cargo tauri dev
```

Record something with Super+H, then in console:
```javascript
await window.__TAURI__.core.invoke('transcribe_audio', { audioPath: '/tmp/lcars-voice-recording.wav' });
```

Expected: Returns transcribed text.

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add Whisper transcription integration"
```

---

## Task 7: Wire Up Complete Recording Flow

**Files:**
- Modify: `lcars-voice/src-tauri/src/main.rs`

**Step 1: Update hotkey handler for full flow**

In the hotkey handler, after stopping recording, automatically transcribe and copy to clipboard:

```rust
// In the hotkey callback, replace the stop recording section:
if was_recording {
    // Stop recording and transcribe
    let app_clone = app_handle.clone();
    std::thread::spawn(move || {
        let state: State<AppState> = app_clone.state();

        // Stop recording
        let audio_path = {
            let mut recorder = state.recorder.lock().unwrap();
            recorder.stop()
        };

        if let Ok(path) = audio_path {
            let _ = app_clone.emit("transcribing", ());

            // Transcribe
            let result = transcription::transcribe(
                &path,
                &state.model,
                &state.venv_path,
            );

            match result {
                Ok(text) => {
                    // Add to history
                    if let Ok(db) = state.db.lock() {
                        let _ = db.add_transcription(&text, None, &state.model);
                    }

                    // Copy to clipboard (via frontend)
                    let _ = app_clone.emit("transcription-complete", text);
                }
                Err(e) => {
                    let _ = app_clone.emit("transcription-error", e);
                }
            }
        }
    });
} else {
    // Start recording
    if let Ok(mut recorder) = state.recorder.lock() {
        if recorder.start().is_ok() {
            let _ = app_handle.emit("recording-started", ());
        }
    }
}
```

**Step 2: Update frontend to handle clipboard**

Update `lcars-voice/src/index.html` to copy to clipboard on transcription complete:

```html
<script>
  const { listen } = window.__TAURI__.event;
  const { writeText } = window.__TAURI__.clipboard;

  listen('recording-started', () => {
    document.getElementById('status').textContent = 'Status: RECORDING';
    document.getElementById('status').classList.add('recording');
  });

  listen('transcribing', () => {
    document.getElementById('status').textContent = 'Status: Transcribing...';
    document.getElementById('status').classList.remove('recording');
  });

  listen('transcription-complete', async (event) => {
    const text = event.payload;
    await writeText(text);
    document.getElementById('status').textContent = 'Status: Copied to clipboard!';
    setTimeout(() => {
      document.getElementById('status').textContent = 'Status: Ready';
    }, 2000);
  });

  listen('transcription-error', (event) => {
    document.getElementById('status').textContent = 'Error: ' + event.payload;
  });
</script>
```

**Step 3: Add clipboard plugin to tauri.conf.json**

Add to plugins array in `tauri.conf.json`:
```json
"plugins": {
  "clipboard-manager": {}
}
```

**Step 4: Test full flow**

```bash
cd lcars-voice && cargo tauri dev
```

Press Super+H, speak, press Super+H. Text should appear in clipboard.

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: wire up complete recording -> transcribe -> clipboard flow"
```

---

## Task 8: Implement LCARS UI

**Files:**
- Copy: `voice-to-text/ui-prototype/*` to `lcars-voice/src/`
- Modify: `lcars-voice/src/app.js` for Tauri integration

**Step 1: Copy prototype files**

```bash
cp voice-to-text/ui-prototype/index.html lcars-voice/src/
cp voice-to-text/ui-prototype/styles.css lcars-voice/src/
cp voice-to-text/ui-prototype/app.js lcars-voice/src/
```

**Step 2: Update app.js for Tauri IPC**

Replace the simulated functions with actual Tauri calls. Key changes:

```javascript
// Replace localStorage with Tauri commands
async loadHistory() {
  try {
    const history = await window.__TAURI__.core.invoke('get_history', { limit: 100 });
    this.history = history;
  } catch (e) {
    console.error('Failed to load history:', e);
    this.history = [];
  }
}

async filterHistory(query) {
  if (query) {
    this.history = await window.__TAURI__.core.invoke('search_history', { query, limit: 100 });
  } else {
    await this.loadHistory();
  }
  this.renderHistory();
}

// Replace simulated recording with listening to Tauri events
bindTauriEvents() {
  const { listen } = window.__TAURI__.event;

  listen('recording-started', () => {
    this.isRecording = true;
    this.updateUI('recording');
    this.startWaveformAnimation();
  });

  listen('transcribing', () => {
    this.isRecording = false;
    this.updateUI('transcribing');
  });

  listen('transcription-complete', async (event) => {
    await this.loadHistory();
    this.renderHistory();
    this.updateUI('ready');
    this.flashStatus('COPIED TO CLIPBOARD');
  });

  listen('transcription-error', (event) => {
    this.updateUI('ready');
    this.flashStatus('ERROR: ' + event.payload);
  });
}

// Replace simulated transcription with actual Tauri command
async toggleRecording() {
  if (this.isTranscribing) return;

  if (this.isRecording) {
    await window.__TAURI__.core.invoke('stop_recording');
  } else {
    await window.__TAURI__.core.invoke('start_recording');
  }
}
```

**Step 3: Test UI**

```bash
cd lcars-voice && cargo tauri dev
```

Expected: Full LCARS UI appears, recording works, history populates.

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: integrate LCARS UI with Tauri backend"
```

---

## Task 9: Create App Icons

**Files:**
- Create: `lcars-voice/src-tauri/icons/*.png`

**Step 1: Create LCARS-style icons**

Create a simple LCARS-themed icon (orange rounded rectangle on black):

```bash
# Using ImageMagick - create basic icons
cd lcars-voice/src-tauri/icons

# Main app icon
convert -size 128x128 xc:black \
  -fill '#FF9900' -draw "roundrectangle 20,20 108,108 15,15" \
  -fill black -draw "roundrectangle 35,45 95,85 8,8" \
  icon-128.png

convert icon-128.png -resize 32x32 32x32.png
convert icon-128.png -resize 128x128 128x128.png
convert icon-128.png -resize 256x256 128x128@2x.png
cp 128x128.png icon.icns
cp 128x128.png icon.ico

# Tray icons
convert -size 22x22 xc:transparent \
  -fill '#FF9900' -draw "roundrectangle 2,2 20,20 4,4" \
  tray-idle.png

convert -size 22x22 xc:transparent \
  -fill '#CC4444' -draw "roundrectangle 2,2 20,20 4,4" \
  tray-recording.png
```

**Step 2: Commit**

```bash
git add -A && git commit -m "feat: add LCARS-style app icons"
```

---

## Task 10: Build and Package

**Files:**
- Modify: `lcars-voice/src-tauri/tauri.conf.json`

**Step 1: Update bundle configuration**

Ensure `tauri.conf.json` has correct bundle settings (already done in Task 1).

**Step 2: Build release**

```bash
cd lcars-voice && cargo tauri build
```

Expected: Creates `.deb` and `.AppImage` in `target/release/bundle/`

**Step 3: Test .deb installation**

```bash
sudo dpkg -i target/release/bundle/deb/lcars-voice_0.1.0_amd64.deb
```

**Step 4: Test AppImage**

```bash
chmod +x target/release/bundle/appimage/lcars-voice_0.1.0_amd64.AppImage
./target/release/bundle/appimage/lcars-voice_0.1.0_amd64.AppImage
```

**Step 5: Commit**

```bash
git add -A && git commit -m "chore: finalize build configuration"
```

---

## Summary

After completing all tasks, you will have:

1. A Tauri app with LCARS-themed UI
2. System tray icon with window toggle
3. Global Super+H hotkey for recording
4. SQLite-backed transcription history
5. Whisper integration for transcription
6. Clipboard integration
7. .deb and AppImage packages

The app maintains full backwards compatibility with the original hotkey workflow while adding a polished visual interface.
