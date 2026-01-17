# Whisper Model Selector - Design Document

## Overview

Add the ability to change the Whisper transcription model from the UI. The model selector appears in the footer bar and expands upward to show available options.

## Requirements

- Available models: base, small, medium, large
- Selection takes effect immediately
- Preference persists across app restarts
- Footer layout changes to accommodate the new control

## Footer Layout Changes

**Before:**
```
[LCARS v2.47 │ WHISPER: BASE] + blue elbow SVG
```

**After:**
```
[LCARS v2.47]  [gap]  [WHISPER: BASE ▾]  [gap]
```

Changes:
- Replace thin `│` separator with full black gap (4px)
- "WHISPER: BASE" becomes a clickable button with dropdown indicator
- Add black gap after the whisper button
- Remove the blue `elbow-bottom-right` SVG entirely

## Dropdown Design

**Trigger button:**
- Tan background (matches footer)
- Text shows current model: "WHISPER: BASE"
- Dropdown indicator (▾) on right side
- Hover: lighter tan, subtle glow

**Dropdown panel (expands upward):**
- Black background with tan/orange border (1-2px)
- Four stacked options: BASE, SMALL, MEDIUM, LARGE
- Each option styled as tan pill-button
- Current selection: orange highlight/border
- Hover state: orange background
- LCARS-style rounded corners

**Behavior:**
- Smooth slide-up animation (~150ms)
- Clicking option selects it and closes dropdown
- Clicking outside closes dropdown
- Anchored to whisper button, aligned right

## Backend & Data Flow

**Storage:** Tauri Store Plugin (`@tauri-apps/plugin-store`)
- Key-value storage for user preferences
- Auto-persists to `settings.json`
- Cleaner than SQLite for simple settings

**Frontend flow:**
1. On app init: load store, get `whisper_model` (default: "base")
2. Display current model in footer button
3. On model select: save to store, update UI immediately
4. Next transcription uses new model

**Backend integration:**
- `transcription.rs` reads model from store before transcribing
- Fall back chain: Store → `WHISPER_MODEL` env var → "base"

## File Changes

### `src/index.html`
- Restructure footer into two pill segments with gaps
- Add whisper model button with dropdown container
- Remove `elbow-bottom-right` SVG from right sidebar

### `src/styles.css`
- Footer segment styles with black gaps
- Whisper button styles (default, hover, active)
- Dropdown panel styles (positioning, border, background)
- Dropdown option styles (pill buttons, selection state)
- Slide-up animation keyframes

### `src/app.js`
- Import and initialize Tauri store on app init
- Load current model, update footer display
- Dropdown toggle logic (open/close state)
- Model selection handler (save to store, update UI)
- Click-outside-to-close event listener

### `src-tauri/Cargo.toml`
- Add `tauri-plugin-store` dependency

### `src-tauri/src/main.rs`
- Register store plugin: `.plugin(tauri_plugin_store::Builder::default().build())`

### `src-tauri/capabilities/default.json`
- Add `"store:default"` to permissions array

### `src-tauri/src/transcription.rs`
- Read whisper model from store before calling Python wrapper
- Pass model to whisper wrapper (may need wrapper modification)
