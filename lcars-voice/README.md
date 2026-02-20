# LCARS Voice

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
- **Stale socket**: A previous crash left `/run/user/1000/lcars-voice.sock` behind. The script handles this automatically, but you can manually clean up:
  ```bash
  rm -f /run/user/1000/lcars-voice.sock
  ```
- **Stale processes**: Old instances blocking the new one:
  ```bash
  pkill -f "target/release/lcars-voice"
  rm -f /run/user/1000/lcars-voice.sock
  ```

### Re-registering the keybinding

If the keybinding is missing from GNOME entirely:
```bash
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/binding "'<Super><Alt>h'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/command "'/home/zknowles/personal/claude-tools/lcars-voice/lcars-voice-toggle.sh'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/lcars-voice/name "'LCARS Voice Toggle'"
```

Then ensure it's in the active list:
```bash
# Check current list
dconf read /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
# Add lcars-voice path if missing
```
