# Azure Deployment for Claude Playground

Deploy the Docker Claude Code Playground to Azure Container Apps.

## Prerequisites

- [Azure CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli) installed
- [Terraform](https://www.terraform.io/downloads) >= 1.0 installed
- [Docker](https://www.docker.com/get-started) installed
- An Azure subscription with permissions to create resources

## Quick Start

### 1. Login to Azure

```bash
az login
```

### 2. Configure Terraform

```bash
cd azure/terraform
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your unique names
```

### 3. Deploy Infrastructure

```bash
./azure/cli/claude-playground init
./azure/cli/claude-playground apply
```

### 4. Build and Push Docker Image

```bash
./azure/cli/claude-playground build-push
```

### 5. Start a Playground Instance

```bash
./azure/cli/claude-playground start
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `start [--user NAME]` | Start a new container instance |
| `stop <name>` | Stop a running instance |
| `list` | List all running instances |
| `open <name>` | Open instance URL in browser |
| `build-push` | Build and push Docker image to ACR |
| `init` | Initialize Terraform |
| `apply` | Apply Terraform configuration |
| `destroy` | Destroy all Azure resources |

## Authentication

Instances are protected by Azure AD authentication. Users in your organization can sign in with their work accounts.

## Costs

| Resource | Cost |
|----------|------|
| Container Apps (2 vCPU, 4GB) | ~$0.06/hour per instance |
| Azure Files | ~$0.06/GB/month |
| Container Registry (Basic) | ~$5/month |

**When idle:** ~$5-10/month (storage + registry only)

## Persistence

User data persists in Azure Files at `/config`. This includes:
- Claude Code settings and conversation history
- Shell history and dotfiles
- Projects and installed tools

## First Time Inside Container

After starting an instance, open a terminal and run:

```bash
claude login
```

This authenticates Claude Code with your Pro/Max account. Credentials persist across restarts.

## Troubleshooting

### "Not logged into Azure"

Run `az login` to authenticate with Azure.

### "Could not get terraform outputs"

Run `./azure/cli/claude-playground apply` to deploy infrastructure first.

### Container won't start

Check the Azure portal for container logs, or run:
```bash
az containerapp logs show --name <app-name> --resource-group claude-playground-rg
```
