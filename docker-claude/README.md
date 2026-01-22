# Docker Claude Code Playground

Browser-accessible Ubuntu XFCE desktop with Claude Code pre-installed. Access via any web browser - no VNC client needed.

## Quick Start

```bash
# Build and start
docker compose build
docker compose up -d

# Stop (preserves data)
docker compose down

# Full reset
docker compose down -v && rm -rf data/config/*
```

## Access

| Location | URL | Notes |
|----------|-----|-------|
| Local | `http://localhost:9202` | HTTP works locally |
| Remote (Tailscale) | `https://<hostname>:9203` | HTTPS required, accept self-signed cert |

## Pre-installed Tools

| Category | Tools |
|----------|-------|
| Core | Claude Code, Chrome, VSCode |
| Node.js | Node.js LTS, npm, pnpm |
| Python | Python 3, pip |
| CLI | AWS CLI v2, gh (GitHub), git, curl, wget, jq |

## First-Time Setup

1. Open terminal in the desktop
2. Run `aws sso login` to authenticate with AWS
3. `cd ~/clawd-qa` (or create your own project folder)
4. Run `claude`

## Persistence

Everything in `/config` (the home directory) persists across restarts:
- Projects and files
- Claude Code settings (`~/.claude/`, `project/.claude/`)
- Shell history, dotfiles
- App configs (VSCode, Chrome)

**Does NOT persist:** System packages installed with `apt`. Add those to the Dockerfile.

## Playwright MCP

Works with visible browser (not headless). The container has `SYS_ADMIN` capability and `seccomp=unconfined` to allow Chrome's sandbox.

## Configuration

**Environment:** Edit `data/config/clawd-qa/.claude/settings.local.json`

**Container resources:** Edit `docker-compose.yml` - defaults to 4 CPU / 8GB RAM

**Timezone:** Change `TZ=America/New_York` in `docker-compose.yml`

## Ports

| Host | Container | Purpose |
|------|-----------|---------|
| 9202 | 3000 | HTTP (local access) |
| 9203 | 3001 | HTTPS (remote/Tailscale) |

## Azure Deployment

For running in Azure (on-demand remote instances), see [azure/README.md](azure/README.md).

Quick start:
```bash
az login
cd azure/terraform && cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars
./azure/cli/claude-playground init
./azure/cli/claude-playground apply
./azure/cli/claude-playground build-push
./azure/cli/claude-playground start
```
