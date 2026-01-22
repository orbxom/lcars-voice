# Docker Claude Code Playground - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a Docker container with browser-accessible Ubuntu desktop for running Claude Code as a playground.

**Architecture:** Extend linuxserver/webtop base image with custom Dockerfile that installs dev tools. Docker Compose handles port mapping, volumes, and resource limits. Access via browser at localhost:9202.

**Tech Stack:** Docker, linuxserver/webtop:ubuntu-xfce, KasmVNC, Node.js, Chrome, VSCode, AWS CLI

---

### Task 1: Create Dockerfile

**Files:**
- Create: `Dockerfile`

**Step 1: Create the Dockerfile with base image and apt packages**

```dockerfile
FROM lscr.io/linuxserver/webtop:ubuntu-xfce

# Install base development tools
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    jq \
    unzip \
    python3 \
    python3-pip \
    ca-certificates \
    gnupg \
    && rm -rf /var/lib/apt/lists/*
```

**Step 2: Verify Dockerfile syntax**

Run: `docker build --check .` or just proceed to next step (syntax check happens on build)

---

### Task 2: Add Node.js installation to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add NodeSource repository and install Node.js LTS**

Append to Dockerfile after the apt-get install block:

```dockerfile
# Install Node.js LTS via NodeSource
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Enable corepack for pnpm
RUN corepack enable && corepack prepare pnpm@latest --activate
```

---

### Task 3: Add AWS CLI v2 to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add AWS CLI v2 installation**

Append to Dockerfile:

```dockerfile
# Install AWS CLI v2
RUN curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip awscliv2.zip \
    && ./aws/install \
    && rm -rf awscliv2.zip aws/
```

---

### Task 4: Add GitHub CLI to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add gh CLI installation**

Append to Dockerfile:

```dockerfile
# Install GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update \
    && apt-get install -y gh \
    && rm -rf /var/lib/apt/lists/*
```

---

### Task 5: Add Google Chrome to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add Chrome installation**

Append to Dockerfile:

```dockerfile
# Install Google Chrome
RUN wget -q -O - https://dl.google.com/linux/linux_signing_key.pub | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] http://dl.google.com/linux/chrome/deb/ stable main" | tee /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update \
    && apt-get install -y google-chrome-stable \
    && rm -rf /var/lib/apt/lists/*
```

---

### Task 6: Add VSCode to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add VSCode installation**

Append to Dockerfile:

```dockerfile
# Install VSCode
RUN wget -qO- https://packages.microsoft.com/keys/microsoft.asc | gpg --dearmor -o /usr/share/keyrings/packages.microsoft.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/packages.microsoft.gpg] https://packages.microsoft.com/repos/code stable main" | tee /etc/apt/sources.list.d/vscode.list \
    && apt-get update \
    && apt-get install -y code \
    && rm -rf /var/lib/apt/lists/*
```

---

### Task 7: Add Claude Code to Dockerfile

**Files:**
- Modify: `Dockerfile`

**Step 1: Add Claude Code installation**

Append to Dockerfile:

```dockerfile
# Install Claude Code globally
RUN npm install -g @anthropic-ai/claude-code
```

---

### Task 8: Create docker-compose.yml

**Files:**
- Create: `docker-compose.yml`

**Step 1: Create the compose file with all configuration**

```yaml
services:
  claude-desktop:
    build: .
    container_name: claude-playground
    ports:
      - "9202:3000"
    volumes:
      - ./data/config:/config
      - ~/.aws:/config/.aws:ro
    environment:
      - PUID=1000
      - PGID=1000
      - TZ=America/New_York
    shm_size: "4gb"
    deploy:
      resources:
        limits:
          cpus: "4"
          memory: 8G
        reservations:
          cpus: "2"
          memory: 4G
    restart: unless-stopped
```

---

### Task 9: Create data directory structure

**Files:**
- Create: `data/.gitkeep`

**Step 1: Create the data directory with gitkeep**

Run:
```bash
mkdir -p data
touch data/.gitkeep
```

---

### Task 10: Add .gitignore

**Files:**
- Create: `.gitignore`

**Step 1: Create gitignore to exclude persistent data**

```
# Persistent container data (created at runtime)
data/config/

# Keep the data directory structure
!data/.gitkeep
```

---

### Task 11: Create README.md

**Files:**
- Create: `README.md`

**Step 1: Create quick reference README**

```markdown
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
```

---

### Task 12: Build the Docker image

**Files:**
- None (uses existing Dockerfile)

**Step 1: Build the image**

Run:
```bash
docker compose build
```

Expected: Build completes successfully (may take several minutes on first build)

---

### Task 13: Start the container and verify

**Step 1: Start the container**

Run:
```bash
docker compose up -d
```

Expected: Container starts successfully

**Step 2: Check container is running**

Run:
```bash
docker compose ps
```

Expected: Shows `claude-playground` as `running`

**Step 3: Open browser and verify desktop loads**

Run:
```bash
echo "Open http://localhost:9202 in your browser"
```

Expected: XFCE desktop appears in browser

---

### Task 14: Verify tools inside container

**Step 1: Open terminal in desktop and verify tools**

Inside container terminal, run:
```bash
node --version
npm --version
pnpm --version
python3 --version
aws --version
gh --version
git --version
claude --version
google-chrome --version
code --version
```

Expected: All commands return version numbers without errors

---

### Task 15: Commit all files

**Step 1: Stage and commit**

Run:
```bash
git add Dockerfile docker-compose.yml data/.gitkeep .gitignore README.md
git commit -m "feat: add Docker Claude Code playground

- Dockerfile with Ubuntu XFCE desktop and dev tools
- docker-compose.yml with resource limits and volume mounts
- Pre-installed: Claude Code, Chrome, VSCode, Node.js, AWS CLI, gh

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Verification Checklist

After completing all tasks, verify:

- [ ] `docker compose up -d` starts container
- [ ] Browser shows XFCE desktop at localhost:9202
- [ ] Terminal opens and Claude Code runs
- [ ] Chrome launches
- [ ] VSCode launches
- [ ] `aws sso login` works
- [ ] Files persist after `docker compose down` and `up`
