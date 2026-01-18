# Single-Command Install Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable one-command installation via `curl | bash` using GitHub Releases for app distribution and `uv` for Python/Whisper management.

**Architecture:** GitHub Actions builds AppImage on push to `release` branch. Install script downloads AppImage + whisper-wrapper.py, installs uv and system deps. Whisper wrapper uses PEP 723 inline deps so `uv run` handles Python automatically.

**Tech Stack:** GitHub Actions, tauri-action, uv, PEP 723 inline script metadata

---

## Task 1: Update whisper-wrapper.py with inline dependencies

**Files:**
- Modify: `scripts/whisper-wrapper.py`

**Step 1: Add PEP 723 inline script metadata**

Replace the first line and add metadata block. Change from:

```python
#!/usr/bin/env python3
"""Simple whisper wrapper that outputs transcription to stdout."""

import sys
```

To:

```python
#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai-whisper", "torch"]
# ///
"""Simple whisper wrapper that outputs transcription to stdout."""

import sys
```

**Step 2: Verify script is still valid Python**

Run: `python3 -m py_compile scripts/whisper-wrapper.py`
Expected: No output (success)

**Step 3: Commit**

```bash
git add scripts/whisper-wrapper.py
git commit -m "feat: add PEP 723 inline deps to whisper wrapper for uv"
```

---

## Task 2: Update transcription.rs to use uv run

**Files:**
- Modify: `src-tauri/src/transcription.rs`

**Step 1: Simplify the transcribe function signature**

Remove `venv_path` parameter since we're using `uv run` now. Change line 12 from:

```rust
pub fn transcribe(audio_path: &Path, model: &str, venv_path: &Path) -> Result<String, String> {
```

To:

```rust
pub fn transcribe(audio_path: &Path, model: &str) -> Result<String, String> {
```

**Step 2: Remove venv-related code and update command**

Replace lines 12-46 with:

```rust
pub fn transcribe(audio_path: &Path, model: &str) -> Result<String, String> {
    println!("[LCARS] transcription: transcribe() called with path = {:?}", audio_path);
    println!("[LCARS] transcription: model = {}", model);

    // Script path: first check installed location, then dev location
    let installed_script = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("lcars-voice")
        .join("whisper-wrapper.py");

    let dev_script = std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.join("scripts").join("whisper-wrapper.py"))
        });

    let script_path = if installed_script.exists() {
        installed_script
    } else {
        dev_script.unwrap_or_else(|| std::path::PathBuf::from("scripts/whisper-wrapper.py"))
    };

    println!("[LCARS] transcription: script_path = {:?}", script_path);

    println!("[LCARS] transcription: Running uv command...");
    let output = Command::new("uv")
        .args([
            "run",
            "--script",
            script_path.to_str().ok_or("Invalid script path")?,
            audio_path.to_str().ok_or("Invalid audio path")?,
            model,
        ])
        .output()
        .map_err(|e| {
            println!("[LCARS] transcription: Failed to run uv: {}", e);
            format!("Failed to run uv: {}. Is uv installed?", e)
        })?;
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Errors about `venv_path` being passed (we'll fix in next task)

**Step 4: Commit (partial, will complete after main.rs update)**

Hold commit until Task 3 is complete.

---

## Task 3: Update main.rs to remove venv_path

**Files:**
- Modify: `src-tauri/src/main.rs`

**Step 1: Remove venv_path from AppState struct (around line 42)**

Change from:

```rust
struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
    venv_path: PathBuf,
}
```

To:

```rust
struct AppState {
    db: Mutex<Database>,
    recorder: Mutex<Recorder>,
    is_recording: AtomicBool,
}
```

**Step 2: Update transcribe_audio command (around line 90-100)**

Change from:

```rust
async fn transcribe_audio(app: tauri::AppHandle, state: State<'_, AppState>, audio_path: String) -> Result<String, String> {
    let path_str = audio_path.clone();
    let venv = state.venv_path.clone();
    let model = get_current_model(&app);

    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&path_str);
        transcription::transcribe(path, &model, &venv)
    })
```

To:

```rust
async fn transcribe_audio(app: tauri::AppHandle, _state: State<'_, AppState>, audio_path: String) -> Result<String, String> {
    let path_str = audio_path.clone();
    let model = get_current_model(&app);

    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&path_str);
        transcription::transcribe(path, &model)
    })
```

**Step 3: Update stop_recording_and_transcribe command (around line 144-153)**

Change:

```rust
    let venv = state.venv_path.clone();
    let model = get_current_model(&app);
    let path_clone = audio_path.clone();

    std::thread::spawn(move || {
        println!("[LCARS] thread: Transcription thread started from command");
        let result = transcription::transcribe(&path_clone, &model, &venv);
```

To:

```rust
    let model = get_current_model(&app);
    let path_clone = audio_path.clone();

    std::thread::spawn(move || {
        println!("[LCARS] thread: Transcription thread started from command");
        let result = transcription::transcribe(&path_clone, &model);
```

**Step 4: Remove venv_path initialization (around line 184-193)**

Change from:

```rust
    let venv_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-to-text-env");
    println!("[LCARS] main: venv_path = {:?}", venv_path);

    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
        venv_path,
    };
```

To:

```rust
    let app_state = AppState {
        db: Mutex::new(db),
        recorder: Mutex::new(recorder),
        is_recording: AtomicBool::new(false),
    };
```

**Step 5: Update tray toggle transcribe call (around line 247-251)**

Change:

```rust
                                    let result = transcription::transcribe(
                                        &path,
                                        &model,
                                        &state.venv_path,
                                    );
```

To:

```rust
                                    let result = transcription::transcribe(
                                        &path,
                                        &model,
                                    );
```

**Step 6: Update file watcher transcribe call (around line 358)**

Change:

```rust
                                        let result = transcription::transcribe(&path, &model, &state.venv_path);
```

To:

```rust
                                        let result = transcription::transcribe(&path, &model);
```

**Step 7: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Success (no errors)

**Step 8: Commit**

```bash
git add src-tauri/src/transcription.rs src-tauri/src/main.rs
git commit -m "refactor: use uv run instead of virtualenv for whisper"
```

---

## Task 4: Update tauri.conf.json bundle deps

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**Step 1: Remove python deps, keep only runtime deps**

Change lines 38-40 from:

```json
      "deb": {
        "depends": ["python3", "python3-venv", "alsa-utils", "xclip"]
      }
```

To:

```json
      "deb": {
        "depends": ["alsa-utils", "xclip"]
      }
```

**Step 2: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "chore: remove python deps from deb (handled by uv now)"
```

---

## Task 5: Create GitHub Actions release workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Create the workflow file**

```yaml
name: Release

on:
  push:
    branches: [release]
  workflow_dispatch:

jobs:
  build-linux:
    runs-on: ubuntu-22.04
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Build and release
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: v__VERSION__
          releaseName: 'LCARS Voice v__VERSION__'
          releaseBody: |
            ## Installation

            ```bash
            curl -sSL https://raw.githubusercontent.com/orbxom/claude-tools/main/lcars-voice/install.sh | bash
            ```

            Or download the AppImage below and run manually.
          releaseDraft: false
          prerelease: false
```

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub Actions release workflow"
```

---

## Task 6: Create install script

**Files:**
- Create: `install.sh`

**Step 1: Create the install script**

```bash
#!/bin/bash
set -e

echo "=== LCARS Voice Installer ==="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

REPO="orbxom/claude-tools"
APP_NAME="lcars-voice"
INSTALL_DIR="$HOME/.local/bin"
DATA_DIR="$HOME/.local/share/lcars-voice"

# Ensure directories exist
mkdir -p "$INSTALL_DIR"
mkdir -p "$DATA_DIR"

# Step 1: Install system dependencies
echo -e "${YELLOW}[1/4] Installing system dependencies...${NC}"
if command -v apt-get &> /dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq alsa-utils xclip
    echo -e "${GREEN}  System deps installed${NC}"
else
    echo -e "${RED}  Warning: apt-get not found. Please install alsa-utils and xclip manually.${NC}"
fi

# Step 2: Install uv if not present
echo -e "${YELLOW}[2/4] Setting up uv (Python manager)...${NC}"
if ! command -v uv &> /dev/null; then
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.local/bin:$PATH"
    echo -e "${GREEN}  uv installed${NC}"
else
    echo -e "${GREEN}  uv already installed${NC}"
fi

# Step 3: Download latest AppImage from GitHub Releases
echo -e "${YELLOW}[3/4] Downloading LCARS Voice...${NC}"

# Get latest release info
RELEASE_INFO=$(curl -s "https://api.github.com/repos/$REPO/releases/latest")
APPIMAGE_URL=$(echo "$RELEASE_INFO" | grep -o '"browser_download_url": *"[^"]*\.AppImage"' | head -1 | cut -d'"' -f4)

if [ -z "$APPIMAGE_URL" ]; then
    echo -e "${RED}  Error: Could not find AppImage in latest release${NC}"
    echo "  Please check: https://github.com/$REPO/releases"
    exit 1
fi

curl -L -o "$INSTALL_DIR/$APP_NAME" "$APPIMAGE_URL"
chmod +x "$INSTALL_DIR/$APP_NAME"
echo -e "${GREEN}  Downloaded to $INSTALL_DIR/$APP_NAME${NC}"

# Step 4: Download whisper-wrapper.py
echo -e "${YELLOW}[4/4] Setting up Whisper wrapper...${NC}"
WRAPPER_URL="https://raw.githubusercontent.com/$REPO/main/lcars-voice/scripts/whisper-wrapper.py"
curl -sSL -o "$DATA_DIR/whisper-wrapper.py" "$WRAPPER_URL"
echo -e "${GREEN}  Wrapper installed to $DATA_DIR/whisper-wrapper.py${NC}"

# Add to PATH if needed
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}Note: Add $INSTALL_DIR to your PATH:${NC}"
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
    echo "  source ~/.bashrc"
fi

echo ""
echo -e "${GREEN}=== Installation complete! ===${NC}"
echo ""
echo "Run with: $APP_NAME"
echo ""
echo -e "${YELLOW}First transcription will download Whisper model (~150MB for base).${NC}"
echo "Subsequent runs will be fast."
```

**Step 2: Make it executable locally**

Run: `chmod +x install.sh`

**Step 3: Commit**

```bash
git add install.sh
git commit -m "feat: add single-command install script"
```

---

## Task 7: Test the changes locally

**Step 1: Ensure uv is installed**

Run: `command -v uv || curl -LsSf https://astral.sh/uv/install.sh | sh`

**Step 2: Build and run the app**

Run: `cd src-tauri && cargo tauri dev`

**Step 3: Test transcription**

- Click record, speak, click stop
- First run will be slow (uv downloads Python + whisper + torch)
- Verify transcription appears

**Step 4: Commit any fixes if needed**

---

## Task 8: Push and create release

**Step 1: Push to main**

```bash
git push origin main
```

**Step 2: Create and push release branch**

```bash
git checkout -b release
git push origin release
```

**Step 3: Verify GitHub Action runs**

Check: https://github.com/orbxom/claude-tools/actions

**Step 4: Verify release is created**

Check: https://github.com/orbxom/claude-tools/releases

---

## Summary

After completing all tasks:

1. Users can install with: `curl -sSL https://raw.githubusercontent.com/orbxom/claude-tools/main/lcars-voice/install.sh | bash`
2. App runs with: `lcars-voice`
3. No manual Python/virtualenv setup required
4. First transcription downloads Whisper automatically via uv
