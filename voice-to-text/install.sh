#!/bin/bash
# Voice to Text Installer
# Installs the voice-to-text tool with Whisper transcription

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PATH="$HOME/voice-to-text-env"
BIN_DIR="$HOME/bin"

echo "=== Voice to Text Installer ==="
echo ""

# Check if running on Linux
if [[ "$(uname)" != "Linux" ]]; then
    echo "Error: This tool only works on Linux."
    exit 1
fi

# Install system dependencies
echo "[1/5] Installing system dependencies..."
if command -v apt &> /dev/null; then
    sudo apt update
    sudo apt install -y python3-venv python3-pip alsa-utils xclip zenity ffmpeg
elif command -v dnf &> /dev/null; then
    sudo dnf install -y python3-virtualenv python3-pip alsa-utils xclip zenity ffmpeg
elif command -v pacman &> /dev/null; then
    sudo pacman -S --noconfirm python python-pip alsa-utils xclip zenity ffmpeg
else
    echo "Warning: Could not detect package manager. Please install manually:"
    echo "  - python3-venv, python3-pip, alsa-utils, xclip, zenity, ffmpeg"
fi

# Create virtual environment
echo ""
echo "[2/5] Creating Python virtual environment..."
if [ -d "$VENV_PATH" ]; then
    echo "Virtual environment already exists at $VENV_PATH"
    read -p "Reinstall? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$VENV_PATH"
        python3 -m venv "$VENV_PATH"
    fi
else
    python3 -m venv "$VENV_PATH"
fi

# Install Whisper
echo ""
echo "[3/5] Installing OpenAI Whisper..."
source "$VENV_PATH/bin/activate"
pip install --upgrade pip
pip install openai-whisper
deactivate

# Copy scripts
echo ""
echo "[4/5] Installing scripts to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp "$SCRIPT_DIR/bin/voice-to-text" "$BIN_DIR/"
cp "$SCRIPT_DIR/bin/voice-to-text-toggle" "$BIN_DIR/"
chmod +x "$BIN_DIR/voice-to-text" "$BIN_DIR/voice-to-text-toggle"

# Ensure ~/bin is in PATH
if [[ ":$PATH:" != *":$HOME/bin:"* ]]; then
    echo ""
    echo "Adding ~/bin to PATH in ~/.bashrc..."
    echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
    echo "Note: Run 'source ~/.bashrc' or restart your terminal for PATH changes to take effect."
fi

# Set up GNOME keyboard shortcut
echo ""
echo "[5/5] Setting up keyboard shortcut (Super+H)..."

if command -v dconf &> /dev/null && command -v gsettings &> /dev/null; then
    # Get existing custom keybindings
    EXISTING=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings 2>/dev/null || echo "[]")

    # Check if voice-to-text keybinding already exists
    if [[ "$EXISTING" != *"voice-to-text"* ]]; then
        # Add our keybinding to the list
        if [[ "$EXISTING" == "@as []" ]] || [[ "$EXISTING" == "[]" ]]; then
            NEW_BINDINGS="['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/']"
        else
            # Remove trailing ] and add our binding
            NEW_BINDINGS="${EXISTING%]*}, '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/']"
        fi
        gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "$NEW_BINDINGS"
    fi

    # Configure the keybinding
    dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/name "'Voice to Text'"
    dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/command "'$BIN_DIR/voice-to-text-toggle'"
    dconf write /org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/voice-to-text/binding "'<Super>h'"

    echo "Keyboard shortcut configured: Super+H"
else
    echo "Warning: Could not configure keyboard shortcut automatically."
    echo "Please set up manually in Settings > Keyboard > Custom Shortcuts"
    echo "  Name: Voice to Text"
    echo "  Command: $BIN_DIR/voice-to-text-toggle"
    echo "  Shortcut: Super+H"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Usage:"
echo "  1. Press Super+H to start recording"
echo "  2. Speak your text"
echo "  3. Press Super+H again to stop and transcribe"
echo "  4. Text is copied to clipboard - paste with Ctrl+V"
echo ""
echo "For command-line usage: voice-to-text"
echo ""
echo "Set WHISPER_MODEL environment variable to change accuracy:"
echo "  export WHISPER_MODEL=small  # Options: tiny, base, small, medium, large"
