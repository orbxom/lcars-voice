# Docker Claude Code Playground

Browser-accessible Ubuntu desktop with Claude Code pre-installed.

## Quick Start

```bash
# Build and start
docker compose up -d

# Open in browser
open http://localhost:9202

# Stop (preserves data)
docker compose down
```

## First-Time Setup

1. Open terminal in the desktop
2. Run `aws sso login` to authenticate with AWS
3. Start using `claude` in any directory

## Pre-installed Tools

- Claude Code CLI
- Chrome (for Playwright)
- VSCode
- Node.js + npm + pnpm
- Python 3
- AWS CLI v2
- GitHub CLI (gh)
- git, curl, wget, jq

## Reset Everything

```bash
docker compose down -v
rm -rf data/config
docker compose up -d --build
```
