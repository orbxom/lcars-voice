# Docker Claude Code Playground - Design

## Overview

A Docker-based Ubuntu environment with visual desktop access for running Claude Code as a self-contained playground. Access the desktop through any web browser - no VNC client needed.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Your Browser (localhost:9202)                  │
└─────────────────────┬───────────────────────────┘
                      │ HTTP/WebSocket
┌─────────────────────▼───────────────────────────┐
│  Docker Container                               │
│  ┌────────────────────────────────────────────┐ │
│  │  KasmVNC (web-based remote desktop)        │ │
│  └────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────┐ │
│  │  XFCE Desktop                              │ │
│  │  - Terminal(s) with Claude Code            │ │
│  │  - Chrome for Playwright                   │ │
│  │  - VSCode                                  │ │
│  │  - Your dev tools                          │ │
│  └────────────────────────────────────────────┘ │
│  Volumes:                                       │
│  - /config (persistent home)                    │
│  - ~/.aws (mounted read-only)                   │
└─────────────────────────────────────────────────┘
```

**Base Image:** `lscr.io/linuxserver/webtop:ubuntu-xfce`

**Access:** `http://localhost:9202`

## Volume Mounts & Persistence

| Host | Container | Purpose |
|------|-----------|---------|
| `./data/config` | `/config` | Persistent home directory |
| `~/.aws` (read-only) | `/config/.aws` | AWS credentials for SSO |

**Persists across restarts:**
- Everything in `/config` (home directory)
- Claude Code settings and conversation history
- Installed tools (`apt install`, `npm install -g`)
- Shell history, dotfiles, projects

**Does not persist:**
- System-level changes outside `/config`
- Running processes

## Pre-installed Tools

**Core:**
- Chrome (for Playwright MCP and browsing)
- Claude Code CLI
- AWS CLI v2 (for `aws sso login`)
- VSCode

**Development:**
- Node.js (LTS) with npm
- pnpm
- Python 3 + pip
- git
- gh CLI (GitHub)
- curl, wget, jq, unzip

**Desktop:**
- XFCE terminal
- Basic text editor

## Resource Allocation

| Resource | Limit | Reservation |
|----------|-------|-------------|
| CPU | 4 cores | 2 cores |
| Memory | 8 GB | 4 GB |
| Shared Memory | 4 GB | - |

Generous allocation for multiple Claude Code instances, Chrome with Playwright, and VSCode running simultaneously.

## File Structure

```
docker-claude/
├── Dockerfile           # Custom image with all tools
├── docker-compose.yml   # Container configuration
├── data/
│   └── config/          # Persistent home (created on first run)
└── README.md            # Quick reference
```

## Usage

```bash
# Start the playground
docker compose up -d

# Access desktop
open http://localhost:9202

# Stop (preserves data)
docker compose down

# Full reset (destroys data)
docker compose down -v
rm -rf data/
```

**First-time setup inside container:**
1. Open terminal
2. Run `aws sso login` to authenticate
3. Start using `claude` in any directory

## Implementation Notes

- `shm_size: 4gb` prevents Chrome/Playwright crashes
- `PUID/PGID=1000` matches typical Linux user for permission compatibility
- `:ro` on AWS mount prevents container from modifying host credentials
- Local-only access (no authentication) for PoC; add auth for remote/AWS deployment
