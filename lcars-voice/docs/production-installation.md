# Production Installation Guide

## Icon Replacement (Completed)

The app icon has been updated from the default Tauri gear to a custom LCARS orange circle:
- Source: `src-tauri/app-icon.svg`
- Generated icons in `src-tauri/icons/`
- Tray icons: `tray-idle.png` (orange), `tray-recording.png` (red)

**Note**: On Linux, the dock/taskbar icon only appears correctly after installing the .deb package (Linux uses .desktop file matching via WM_CLASS).

## Hotkey Setup

| Hotkey | Purpose | Configuration |
|--------|---------|---------------|
| **Super+Alt+H** | Toggle recording | Built into app via `tauri-plugin-global-shortcut` |
| **Super+H** | Launch app | System keybinding via GNOME dconf |

### Super+Alt+H (In-App)
Works automatically in both dev and production. No setup required.

### Super+H (System Keybinding)
Requires running the install script:
```bash
./scripts/install-keybinding.sh
killall gsd-media-keys && /usr/libexec/gsd-media-keys &
```

## Building for Production

```bash
cd src-tauri
cargo tauri build
```

This creates:
- `target/release/bundle/deb/lcars-voice_*.deb`
- `target/release/bundle/appimage/lcars-voice_*.AppImage`

## Installing

```bash
sudo dpkg -i target/release/bundle/deb/lcars-voice_*.deb
```

The .deb installs:
- Binary: `/usr/bin/lcars-voice`
- Desktop file: `/usr/share/applications/LCARS Voice.desktop`
- Icons: `/usr/share/icons/hicolor/*/apps/lcars-voice.png`

## Post-Installation

### Update Launch Script for Production
The `scripts/lcars-launch.sh` script needs to detect the installed binary:

```bash
#!/bin/bash
# Launch LCARS Voice - supports both dev and installed versions

if command -v lcars-voice &> /dev/null; then
    # Production: use installed binary
    lcars-voice
elif [ -f "$(dirname "$0")/../src-tauri/Cargo.toml" ]; then
    # Dev: run from source
    cd "$(dirname "$0")/.." && cargo tauri dev
else
    notify-send "LCARS Voice" "Application not found"
    exit 1
fi
```

### Re-run Keybinding Setup
After installing, run the keybinding script again to ensure Super+H launches the installed binary:
```bash
./scripts/install-keybinding.sh
```

## Verification Checklist

- [ ] Build completes without errors
- [ ] .deb package installs successfully
- [ ] App launches from application menu
- [ ] Dock/taskbar shows orange circle icon (not gear)
- [ ] Super+H launches the app
- [ ] Super+Alt+H toggles recording
- [ ] Tray icon shows orange circle (idle) / red circle (recording)
- [ ] Transcription works and copies to clipboard

## Known Issues

1. **Whisper script path**: The whisper-wrapper.py path resolution assumes dev directory structure. For production, ensure the script is accessible or embed it as a resource.

2. **Python virtualenv**: Users must have `~/voice-to-text-env` with `openai-whisper` installed.

3. **System dependencies**: Requires `alsa-utils`, `xclip`, `libnotify-bin`.
