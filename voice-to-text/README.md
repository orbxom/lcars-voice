# Voice to Text

A simple voice-to-text tool for Linux that records audio, transcribes it using OpenAI's Whisper, and copies the result to your clipboard.

## Features

- Press `Super+H` to start recording
- Press `Super+H` again to stop and transcribe
- Transcription is automatically copied to clipboard
- Visual feedback via modal dialog showing recording/transcribing status

## Requirements

- Ubuntu/Debian-based Linux with GNOME desktop
- Python 3.10+
- ALSA audio utilities (`arecord`)
- X11 clipboard (`xclip`)
- Zenity (for GUI dialogs)

## Installation

### Quick Install

```bash
./install.sh
```

### Manual Installation

#### 1. Install system dependencies

```bash
sudo apt update
sudo apt install -y python3-venv python3-pip alsa-utils xclip zenity ffmpeg
```

#### 2. Create Python virtual environment and install Whisper

```bash
python3 -m venv ~/voice-to-text-env
source ~/voice-to-text-env/bin/activate
pip install --upgrade pip
pip install openai-whisper
```

#### 3. Copy scripts to your bin directory

```bash
mkdir -p ~/bin
cp bin/voice-to-text ~/bin/
cp bin/voice-to-text-toggle ~/bin/
chmod +x ~/bin/voice-to-text ~/bin/voice-to-text-toggle
```

Make sure `~/bin` is in your PATH. Add this to your `~/.bashrc` if needed:

```bash
export PATH="$HOME/bin:$PATH"
```

#### 4. Set up the keyboard shortcut

**Using command line (GNOME):**

```bash
# Add the custom keybinding
gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/']"

# Configure the keybinding
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/name "'Voice to Text'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/command "'$HOME/bin/voice-to-text-toggle'"
dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/binding "'<Super>h'"
```

**Using GNOME Settings (GUI):**

1. Open Settings > Keyboard > Keyboard Shortcuts
2. Scroll down and click "Custom Shortcuts"
3. Click the "+" button to add a new shortcut
4. Name: `Voice to Text`
5. Command: `/home/YOUR_USERNAME/bin/voice-to-text-toggle`
6. Set the shortcut to `Super+H`

## Usage

### With Hotkey (Recommended)

1. Press `Super+H` to start recording - a dialog will appear showing "Recording..."
2. Speak your text
3. Press `Super+H` again to stop recording
4. Wait for transcription - dialog will show "Transcribing..."
5. The transcribed text is automatically copied to your clipboard
6. Paste with `Ctrl+V`

### Command Line

For terminal-based usage without the GUI:

```bash
voice-to-text
```

Press `Ctrl+C` when done speaking.

## Configuration

### Whisper Model

Set the `WHISPER_MODEL` environment variable to change the model:

```bash
export WHISPER_MODEL=small  # Options: tiny, base, small, medium, large
```

Larger models are more accurate but slower. The default is `base`.

### Model Comparison

| Model  | Size  | Speed    | Accuracy |
|--------|-------|----------|----------|
| tiny   | 39M   | Fastest  | Lower    |
| base   | 74M   | Fast     | Good     |
| small  | 244M  | Medium   | Better   |
| medium | 769M  | Slow     | High     |
| large  | 1.5G  | Slowest  | Highest  |

## Troubleshooting

### "Virtual environment not found"

Make sure you created the virtual environment at `~/voice-to-text-env`:

```bash
python3 -m venv ~/voice-to-text-env
source ~/voice-to-text-env/bin/activate
pip install openai-whisper
```

### "arecord not found"

Install ALSA utilities:

```bash
sudo apt install alsa-utils
```

### No audio recorded

Check your microphone is working:

```bash
arecord -l  # List recording devices
arecord -d 5 test.wav  # Test recording for 5 seconds
aplay test.wav  # Play it back
```

### Hotkey not working

Make sure the keybinding is set correctly:

```bash
dconf read /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/binding
```

Should output: `'<Super>h'`

## License

MIT
