# Azure Deployment Design

## Overview

Deploy the Docker Claude Code Playground to Azure for on-demand remote access. Multiple instances can run simultaneously, with persistent storage per user. Authentication via Azure AD restricts access to your organization.

## Goals

- Free up local system resources by running in Azure
- Spin up 2-3 instances on demand
- Share with coworkers (same infrastructure, separate instances)
- Pay only when running (on-demand, not always-on)
- CLI-based start/stop/connect workflow

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Your Machine                                                    │
│  ┌─────────────┐         ┌──────────────────────────────────┐   │
│  │ CLI tool    │         │ Browser                          │   │
│  │ start/stop  │         │ https://claude-xyz.azurecontainer│   │
│  └─────────────┘         │ apps.io (Azure AD login)         │   │
│                          └──────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                                      │ HTTPS + Azure AD
┌─────────────────────────────────────┼───────────────────────────┐
│  Azure                              ▼                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Azure Container Apps Environment                         │   │
│  │  ┌─────────────────────┐      ┌─────────────────────┐    │   │
│  │  │  Container App      │      │  Container App      │    │   │
│  │  │  (your instance)    │      │  (instance 2)       │    │   │
│  │  │  ┌───────────────┐  │      │  ┌───────────────┐  │    │   │
│  │  │  │ KasmVNC :3000 │  │      │  │ KasmVNC :3000 │  │    │   │
│  │  │  │ Claude Code   │  │      │  │ Claude Code   │  │    │   │
│  │  │  └───────────────┘  │      └──┴───────────────┘──┘    │   │
│  │  └──────────┬──────────┘                 │               │   │
│  └─────────────┼────────────────────────────┼───────────────┘   │
│                │                            │                    │
│  ┌─────────────▼────────────────────────────▼───────────────┐   │
│  │  Azure Files (SMB share)                                  │   │
│  │  /users/zknowles/  ←─ your persistent home                │   │
│  │  /users/coworker1/ ←─ future coworker                     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Azure Container Registry - stores the Docker image       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Azure AD - authenticates users from your organization    │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Compute | Azure Container Apps | Serverless containers, consumption-based pricing, built-in auth |
| Storage | Azure Files (SMB) | Native integration with Container Apps, persistent across restarts |
| Authentication | Azure AD (Easy Auth) | Built-in, no custom code, org-level access control |
| Access | Public HTTPS URL | Stable URLs, no VPN or tunneling required |
| Infrastructure | Terraform | Version controlled, reproducible, shareable |
| Claude Code auth | Pro/Max plan (`claude login`) | No AWS dependencies |

## Project Structure

```
docker-claude/
├── Dockerfile                 # Shared - works for both local and Azure
├── docker-compose.yml         # Local development (unchanged)
├── data/                      # Local persistent data (unchanged)
├── azure/
│   ├── terraform/
│   │   ├── main.tf           # Resource group, VNet, Container Apps env
│   │   ├── variables.tf      # Configurable values (region, sizing)
│   │   ├── outputs.tf        # App URLs, resource IDs
│   │   ├── container-app.tf  # Container App configuration
│   │   ├── storage.tf        # Azure Files share
│   │   └── auth.tf           # Azure AD app registration
│   ├── cli/
│   │   └── claude-playground # Bash script: start, stop, list, open
│   └── README.md             # Azure-specific setup instructions
├── README.md                  # Main readme (local usage, links to azure/)
└── docs/
    └── plans/
```

## Azure Resources (Terraform)

**Networking:**
- Resource Group (logical container for all resources)
- Virtual Network with subnet for Container Apps

**Compute:**
- Azure Container Apps Environment (consumption plan)
- Container App(s) with HTTPS ingress + Azure AD auth

**Storage:**
- Azure Container Registry (Basic tier)
- Azure Storage Account + File Share

**Authentication:**
- Azure AD App Registration
- Service Principal for CLI

## CLI Commands

```bash
# Start a new instance
./azure/cli/claude-playground start [--user zknowles]
# Output: Instance started: https://claude-zknowles-abc123.azurecontainerapps.io

# List running instances
./azure/cli/claude-playground list

# Open instance in browser
./azure/cli/claude-playground open claude-zknowles-abc123

# Stop an instance
./azure/cli/claude-playground stop claude-zknowles-abc123
```

## Usage Workflow

**One-time setup:**
```bash
# 1. Login to Azure CLI
az login

# 2. Deploy infrastructure
cd azure/terraform && terraform init && terraform apply

# 3. Build and push Docker image
./azure/cli/claude-playground build-push
```

**Daily usage:**
```bash
# Start instance, open URL, sign in with Azure AD
./azure/cli/claude-playground start

# First time inside container: authenticate Claude Code
claude login

# When done
./azure/cli/claude-playground stop <instance-name>
```

**Sharing with coworkers:**
```bash
# Coworker clones repo, runs terraform (or uses shared state)
# Then starts their own instance with their own persistent storage
./azure/cli/claude-playground start --user coworker1
```

## Cost Estimates

**When running:**
| Resource | Cost |
|----------|------|
| Container Apps (2 vCPU, 4GB) | ~$0.06/hour per instance |
| Azure Files | ~$0.06/GB/month (hot tier) |
| Container Registry (Basic) | ~$5/month |

**When idle (no instances):**
| Resource | Cost |
|----------|------|
| Azure Files | ~$0.06/GB stored |
| Container Registry | ~$5/month |
| **Total idle** | **~$5-10/month** |

## Persistence

**Persists across restarts (Azure Files):**
- Everything in `/config` (home directory)
- Claude Code settings and conversation history
- Installed tools, shell history, dotfiles, projects

**Does not persist:**
- System-level changes outside `/config`
- Running processes

## First-Time Container Setup

After starting a container instance:
```bash
# Authenticate Claude Code with your Pro/Max account
claude login
```

This only needs to be done once per user - credentials persist in Azure Files.
