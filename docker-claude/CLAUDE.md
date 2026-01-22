# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Docker Claude Code Playground - a browser-accessible Ubuntu XFCE desktop environment with Claude Code pre-installed. Access the playground at `localhost:9202` via any web browser (KasmVNC-based, no VNC client needed).

## Common Commands

```bash
# Build the Docker image
docker compose build

# Start the playground (detached)
docker compose up -d

# Stop the playground (preserves data)
docker compose down

# Full reset (removes all persistent data)
docker compose down -v && rm -rf data/config/*

# Check container status
docker compose ps
```

## Architecture

- **Base Image**: `lscr.io/linuxserver/webtop:ubuntu-xfce` - provides browser-based desktop via KasmVNC
- **Port**: 9202 (host) → 3000 (container)
- **Persistence**: `./data/config` mounted to `/config` (container's home directory)
- **AWS Credentials**: Host `~/.aws` mounted read-only for SSO login inside container

## Pre-installed Tools

The Dockerfile layers these on top of the base image:
- Node.js LTS + pnpm
- AWS CLI v2, GitHub CLI (gh)
- Google Chrome, VSCode
- Claude Code CLI (globally installed)
- Python 3 + pip, git, curl, wget, jq

## Persistence Model

**Persists across restarts**: Everything in `/config` (home directory) - projects, dotfiles, Claude Code settings, shell history, globally installed tools.

**Does NOT persist**: System-level changes outside `/config`, running processes.

## First-Time Setup

After starting the container, open a terminal inside the desktop and run:
```bash
aws sso login
```
This authenticates with AWS (credentials are used by Claude Code's Bedrock backend).
