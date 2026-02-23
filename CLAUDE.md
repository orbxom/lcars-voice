# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Repository Overview

Monorepo of personal productivity tools focused on meeting recording, transcription, voice-to-text, and Slack integration. Each tool is independent with its own language, dependencies, and CLAUDE.md.

## Tools

| Tool | Language | Purpose |
|------|----------|---------|
| [lcars-voice](lcars-voice/) | Rust + JS (Tauri v2) | Desktop voice recording & transcription with LCARS UI |
| [voice-to-text](voice-to-text/) | Bash + Python | Lightweight keyboard-driven voice-to-text for clipboard |
| [zoom-recorder](zoom-recorder/) | Python (Tkinter) | Record Zoom meetings with inline JIRA ticket marking |
| [meeting-transcripts](meeting-transcripts/) | Python + Bash | Process recordings into per-JIRA-ticket markdown transcripts |
| [slack-to-markdown](slack-to-markdown/) | Python | Convert Slack thread URLs into markdown files with attachments |
| [docker-claude](docker-claude/) | Docker + Terraform | Browser-accessible remote dev environment with Claude Code |

Each tool has its own `CLAUDE.md` with architecture, build commands, and conventions.

## Shared Conventions

### Shell Scripts
- Error handling: `set -euo pipefail`
- Logging: `info()`, `error()`, `warn()` color functions
- Idempotent setup: `setup.sh` checks for existing tools before installing
- Script directory: `SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"`
- Config: `.env` files with `.env.example` templates

### Python Tools
- Testing: `pytest` with mocked subprocess/API calls
- Virtual environments: `.venv/` (per-tool) or `~/voice-to-text-env/` (shared)
- Entry point: `python3 -m src` or direct script execution
- No shared Python packages across tools

### Audio Standard
- Format: 16kHz mono WAV (enforced across all recording/transcription tools)
- Capture: PipeWire/PulseAudio on Linux
- Transcription: OpenAI Whisper (various model sizes)

## Git Workflow

- Monorepo with independent tools — no shared build system
- Tagging: `lcars-voice-v*` tags trigger GitHub Actions CI/CD releases
- Design docs: `docs/plans/YYYY-MM-DD-<topic>-{design,implementation}.md`

## Key Paths

- `docs/plans/` — Design and implementation planning documents
- `.claude/` — Claude Code settings
- `.github/workflows/` — CI/CD for lcars-voice releases
