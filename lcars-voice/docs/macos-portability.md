# macOS Portability Goal

## Objective

Enable LCARS Voice to build and run on macOS with the same functionality as the Linux version.

## Current State

Most of the codebase is already cross-platform thanks to Tauri and carefully chosen Rust dependencies. The main blocker is Linux-specific audio recording.

## Required Changes

### Critical (Blocks macOS Functionality)

#### 1. Replace `arecord` with Cross-Platform Audio

**File:** `src-tauri/src/recording.rs`

Current implementation uses `arecord` (ALSA), which is Linux-only.

**Solution:** Use the `cpal` crate for cross-platform audio capture.

```toml
# Add to Cargo.toml
cpal = "0.15"
```

This handles:
- Linux: ALSA/PulseAudio
- macOS: CoreAudio
- Windows: WASAPI (bonus)

#### 2. Update Tauri Bundle Configuration

**File:** `src-tauri/tauri.conf.json`

Add macOS targets and configuration:

```json
{
  "bundle": {
    "targets": ["deb", "appimage", "dmg"],
    "macos": {
      "minimumSystemVersion": "10.13"
    }
  }
}
```

#### 3. Fix Hardcoded `/tmp/` Path

**File:** `src-tauri/src/main.rs` (line 324)

```rust
// Change from:
let toggle_file = std::path::PathBuf::from("/tmp/lcars-voice-toggle");

// To:
let toggle_file = std::env::temp_dir().join("lcars-voice-toggle");
```

### Already Cross-Platform (No Changes Needed)

| Component | Implementation | Notes |
|-----------|---------------|-------|
| Clipboard | `tauri-plugin-clipboard-manager` | Handles OS differences internally |
| Database paths | `dirs::data_local_dir()` | Maps to `~/Library/Application Support/` on macOS |
| Settings store | `tauri-plugin-store` | Cross-platform |
| Global hotkey | `tauri-plugin-global-shortcut` | Super key maps to Cmd on macOS |
| Python virtualenv path | `dirs::home_dir().join("voice-to-text-env")` | Same path structure works |
| All Cargo dependencies | Various | All support macOS |

### Nice-to-Have Improvements

#### 4. Apple Silicon GPU Acceleration

**File:** `scripts/whisper-wrapper.py`

Add Metal Performance Shaders (MPS) support for Apple Silicon Macs:

```python
if torch.cuda.is_available():
    device = "cuda"
elif torch.backends.mps.is_available():
    device = "mps"
else:
    device = "cpu"
```

#### 5. macOS Keybinding Setup Script

**Current:** `scripts/install-keybinding.sh` is GNOME-specific

**Needed:** macOS equivalent or documentation for:
- System Preferences > Keyboard > Shortcuts > App Shortcuts
- Or a LaunchAgent for auto-starting

#### 6. macOS Installation Documentation

Document macOS-specific setup:
- Homebrew dependencies (if any)
- Python virtualenv setup on macOS
- PyTorch installation without CUDA

## Implementation Order

1. Add `cpal` dependency and refactor `recording.rs` with conditional compilation
2. Fix the `/tmp/` hardcoded path
3. Update `tauri.conf.json` for macOS builds
4. Test build on macOS
5. Add MPS support to whisper-wrapper.py
6. Create macOS installation docs

## Dependencies to Install on macOS

```bash
# Python environment
python3 -m venv ~/voice-to-text-env
source ~/voice-to-text-env/bin/activate
pip install openai-whisper torch

# No additional system dependencies needed for audio (CoreAudio is built-in)
```

## Testing Checklist

- [ ] App builds on macOS (`cargo tauri build`)
- [ ] Audio recording works
- [ ] Transcription completes successfully
- [ ] Clipboard copy works
- [ ] Global hotkey (Cmd+Option+H) triggers recording
- [ ] System tray icon displays correctly
- [ ] History database persists across sessions
- [ ] Model selection persists
