#!/bin/bash
# Slack-to-Markdown Setup Script
# Run with: bash setup.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Setting up slack-to-markdown..."
echo ""

# Check Python 3
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1 | cut -d' ' -f2)
    echo -e "${GREEN}✓${NC} Python ${PYTHON_VERSION} found"
else
    echo -e "${RED}✗${NC} Python 3 not found. Install Python 3.10+."
    exit 1
fi

# Create virtualenv if needed
VENV_DIR="${SCRIPT_DIR}/.venv"
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment..."
    python3 -m venv "$VENV_DIR"
    echo -e "${GREEN}✓${NC} Virtual environment created"
else
    echo -e "${GREEN}✓${NC} Virtual environment exists"
fi

# Install dependencies
echo "Installing dependencies..."
"${VENV_DIR}/bin/pip" install -q -r "${SCRIPT_DIR}/requirements.txt"
echo -e "${GREEN}✓${NC} Dependencies installed"

# Check .env
if [ -f "${SCRIPT_DIR}/.env" ]; then
    echo -e "${GREEN}✓${NC} .env file found"
else
    echo -e "${YELLOW}○${NC} .env file not found"
    echo "  Copy .env.example to .env and add your Slack user token:"
    echo "    cp ${SCRIPT_DIR}/.env.example ${SCRIPT_DIR}/.env"
    echo "  Then edit .env with your token."
fi

echo ""
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "Usage:"
echo "  cd ${SCRIPT_DIR}"
echo "  .venv/bin/python -m src <slack-thread-url>"
