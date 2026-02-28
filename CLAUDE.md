# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Repository Overview

Monorepo of personal productivity tools focused on voice recording, meeting recording, and transcription. Each tool has its own language, dependencies, and CLAUDE.md.

## Tools

| Tool | Language | Purpose |
|------|----------|---------|
| [lcars-voice](lcars-voice/) | Rust + JS (Tauri v2) | Desktop voice recording, meeting recording & transcription with LCARS UI |
| [docker-claude](docker-claude/) | Docker + Terraform | Browser-accessible remote dev environment with Claude Code |

## Shared Conventions

### Audio Standard
- Format: 16kHz mono WAV (enforced across all recording/transcription)
- Capture: `cpal` (cross-platform; ALSA/PulseAudio on Linux)
- Transcription: `whisper-rs` (native whisper.cpp bindings, GGML models: base/small/medium/large)

## Git Workflow

- Monorepo with two independent tools — no shared build system
- Tagging: `lcars-voice-v*` tags trigger GitHub Actions CI/CD releases

## Key Paths

- `.claude/` — Claude Code settings
- `.github/workflows/` — CI/CD for lcars-voice releases

## History

In February 2026, four standalone tools (voice-to-text, zoom-recorder, meeting-transcripts, slack-to-markdown) were consolidated into lcars-voice or removed.
