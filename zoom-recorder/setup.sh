#!/bin/bash
# Zoom Recorder Setup Script
# Run with: sudo bash setup.sh
# Idempotent - safe to run multiple times

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "Checking dependencies..."
echo ""

# Track if we need to install anything
NEEDS_INSTALL=false

# Check Python 3
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1 | cut -d' ' -f2)
    echo -e "${GREEN}✓${NC} Python ${PYTHON_VERSION} found"
else
    echo -e "${RED}✗${NC} Python 3 not found"
    NEEDS_INSTALL=true
fi

# Check Tkinter
if python3 -c "import tkinter" 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Tkinter already installed"
else
    echo -e "${YELLOW}○${NC} Tkinter not installed - will install python3-tk"
    NEEDS_INSTALL=true
    INSTALL_TKINTER=true
fi

# Check FFmpeg
if command -v ffmpeg &> /dev/null; then
    FFMPEG_VERSION=$(ffmpeg -version 2>&1 | head -1 | cut -d' ' -f3)
    echo -e "${GREEN}✓${NC} FFmpeg ${FFMPEG_VERSION} found"
else
    echo -e "${YELLOW}○${NC} FFmpeg not found - will install"
    NEEDS_INSTALL=true
    INSTALL_FFMPEG=true
fi

# Check PipeWire
if command -v pw-cli &> /dev/null; then
    echo -e "${GREEN}✓${NC} PipeWire tools found"
else
    echo -e "${YELLOW}○${NC} PipeWire tools not found - will install pipewire-pulse"
    NEEDS_INSTALL=true
    INSTALL_PIPEWIRE=true
fi

# Check pactl (pulseaudio-utils)
if command -v pactl &> /dev/null; then
    echo -e "${GREEN}✓${NC} pactl found"
else
    echo -e "${YELLOW}○${NC} pactl not found - will install pulseaudio-utils"
    NEEDS_INSTALL=true
    INSTALL_PACTL=true
fi

echo ""

# Install missing dependencies
if [ "$NEEDS_INSTALL" = true ]; then
    echo "Installing missing dependencies..."
    apt-get update -qq

    if [ "$INSTALL_TKINTER" = true ]; then
        echo "  Installing python3-tk..."
        apt-get install -y -qq python3-tk
        echo -e "  ${GREEN}✓${NC} Tkinter installed"
    fi

    if [ "$INSTALL_FFMPEG" = true ]; then
        echo "  Installing ffmpeg..."
        apt-get install -y -qq ffmpeg
        echo -e "  ${GREEN}✓${NC} FFmpeg installed"
    fi

    if [ "$INSTALL_PIPEWIRE" = true ]; then
        echo "  Installing pipewire-pulse..."
        apt-get install -y -qq pipewire-pulse
        echo -e "  ${GREEN}✓${NC} PipeWire pulse tools installed"
    fi

    if [ "$INSTALL_PACTL" = true ]; then
        echo "  Installing pulseaudio-utils..."
        apt-get install -y -qq pulseaudio-utils
        echo -e "  ${GREEN}✓${NC} pactl installed"
    fi

    echo ""
fi

# Verify PipeWire is running (run as invoking user, not root)
echo "Checking PipeWire status..."
SUDO_USER_HOME=$(getent passwd "${SUDO_USER:-$USER}" | cut -d: -f6)

if sudo -u "${SUDO_USER:-$USER}" XDG_RUNTIME_DIR="/run/user/$(id -u "${SUDO_USER:-$USER}")" pw-cli info 0 &>/dev/null; then
    echo -e "${GREEN}✓${NC} PipeWire is running"
else
    echo -e "${RED}✗${NC} PipeWire is not running"
    echo "  Start it with: systemctl --user start pipewire pipewire-pulse"
    exit 1
fi

# Detect audio sources
echo ""
echo "Audio sources detected:"

# Get sources as the invoking user
SOURCES=$(sudo -u "${SUDO_USER:-$USER}" XDG_RUNTIME_DIR="/run/user/$(id -u "${SUDO_USER:-$USER}")" pw-cli list-objects 2>/dev/null | grep -E "node.name.*alsa" || true)

# Find mic (input)
MIC=$(sudo -u "${SUDO_USER:-$USER}" XDG_RUNTIME_DIR="/run/user/$(id -u "${SUDO_USER:-$USER}")" pactl list sources short 2>/dev/null | grep -v monitor | head -1 | cut -f2 || echo "")
if [ -n "$MIC" ]; then
    echo -e "  ${GREEN}Mic:${NC} $MIC"
else
    echo -e "  ${YELLOW}Mic:${NC} No input device detected"
fi

# Find monitor (system audio)
MONITOR=$(sudo -u "${SUDO_USER:-$USER}" XDG_RUNTIME_DIR="/run/user/$(id -u "${SUDO_USER:-$USER}")" pactl list sources short 2>/dev/null | grep monitor | head -1 | cut -f2 || echo "")
if [ -n "$MONITOR" ]; then
    echo -e "  ${GREEN}Monitor:${NC} $MONITOR"
else
    echo -e "  ${YELLOW}Monitor:${NC} No monitor source detected"
fi

echo ""
echo -e "${GREEN}Setup complete!${NC}"

# Create output directory
OUTPUT_DIR="$SUDO_USER_HOME/zoom-recordings"
if [ ! -d "$OUTPUT_DIR" ]; then
    mkdir -p "$OUTPUT_DIR"
    chown "${SUDO_USER:-$USER}:${SUDO_USER:-$USER}" "$OUTPUT_DIR"
    echo "Created output directory: $OUTPUT_DIR"
fi
