# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Repository Overview

Desktop voice recording, meeting recording, and transcription tool with an LCARS-themed UI. Built with Rust and JavaScript (Tauri v2).

## Audio Standard
- Format: 16kHz mono WAV (enforced across all recording/transcription)
- Capture: `cpal` (cross-platform; ALSA/PulseAudio on Linux)
- Transcription: `whisper-rs` (native whisper.cpp bindings, GGML models: base/small/medium/large)

## Git Workflow

- Tagging: `lcars-voice-v*` tags trigger GitHub Actions CI/CD releases

## Key Paths

- `.claude/` — Claude Code settings
- `.github/workflows/` — CI/CD for lcars-voice releases

## History

In February 2026, four standalone tools (voice-to-text, zoom-recorder, meeting-transcripts, slack-to-markdown) were consolidated into lcars-voice or removed.

In March 2026, docker-claude was extracted into its own standalone repository.
