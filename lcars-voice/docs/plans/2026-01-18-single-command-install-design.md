# Single-Command Install Design

## Goal

Enable installation of LCARS Voice with a single command for trusted friends/colleagues on Linux.

## Approach

Two components:

1. **GitHub Actions workflow** - Builds and publishes `.AppImage` to GitHub Releases
2. **Install script** - Users run `curl ... | bash` to set everything up

## Component 1: GitHub Actions Workflow

**File:** `.github/workflows/release.yml`

Triggers on push to `release` branch or manual dispatch. Builds Linux-only (this is a Linux app).

```yaml
name: Release
on:
  push:
    branches: [release]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-22.04
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - name: Install Linux deps
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: v__VERSION__
          releaseName: 'LCARS Voice v__VERSION__'
          releaseDraft: false
```

## Component 2: Install Script

**File:** `install.sh`

**Usage:**
```bash
curl -sSL https://raw.githubusercontent.com/USER/lcars-voice/main/install.sh | bash
```

**Steps:**
1. Install system deps (`alsa-utils`, `xclip`)
2. Install `uv` if not present
3. Download latest `.AppImage` from GitHub Releases to `~/.local/bin/lcars-voice`
4. Download `whisper-wrapper.py` to `~/.local/share/lcars-voice/`
5. Optionally prime the uv cache by running the wrapper once

## Component 3: Whisper Wrapper Changes

Update `scripts/whisper-wrapper.py` to use PEP 723 inline script metadata:

```python
#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["openai-whisper", "torch"]
# ///

import sys
import whisper
import json
# ... rest unchanged
```

This eliminates the need for a separate virtualenv. `uv run` automatically manages Python and dependencies.

## Component 4: Rust Changes

Update `src-tauri/src/transcription.rs` to invoke the wrapper via uv:

- Old: `~/voice-to-text-env/bin/python scripts/whisper-wrapper.py`
- New: `uv run ~/.local/share/lcars-voice/whisper-wrapper.py`

## Files to Create/Modify

**New:**
- `.github/workflows/release.yml`
- `install.sh`

**Modified:**
- `scripts/whisper-wrapper.py` - Add inline deps shebang
- `src-tauri/src/transcription.rs` - Change wrapper invocation

## User Experience

```bash
# Install
curl -sSL https://raw.githubusercontent.com/USER/lcars-voice/main/install.sh | bash

# Run
lcars-voice
```

First transcription may be slow as uv downloads Python + whisper + torch. Subsequent runs use cached environment.
