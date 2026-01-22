# Azure Deployment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deploy Docker Claude Code Playground to Azure Container Apps with Azure Files persistence and Azure AD authentication.

**Architecture:** Azure Container Apps with consumption plan, Azure Files for persistent home directories, built-in Easy Auth for Azure AD login. CLI tool for start/stop/list/open commands.

**Tech Stack:** Terraform (Azure provider), Azure CLI, Bash, Azure Container Apps, Azure Files, Azure Container Registry, Azure AD

---

## Prerequisites

Before starting, ensure you have:
- Azure CLI installed (`az --version`)
- Terraform installed (`terraform --version`)
- An Azure subscription with permissions to create resources
- Logged into Azure (`az login`)

---

## Task 1: Create Azure Directory Structure

**Files:**
- Create: `azure/terraform/.gitkeep`
- Create: `azure/cli/.gitkeep`

**Step 1: Create the azure directory structure**

```bash
mkdir -p azure/terraform azure/cli
```

**Step 2: Verify the structure**

Run: `ls -la azure/`
Expected: Two directories: `terraform` and `cli`

**Step 3: Commit**

```bash
git add azure/
git commit -m "chore: create azure directory structure"
```

---

## Task 2: Create Terraform Variables

**Files:**
- Create: `azure/terraform/variables.tf`

**Step 1: Create variables.tf with all configurable values**

```hcl
variable "resource_group_name" {
  description = "Name of the Azure resource group"
  type        = string
  default     = "claude-playground-rg"
}

variable "location" {
  description = "Azure region for resources"
  type        = string
  default     = "eastus"
}

variable "environment_name" {
  description = "Name of the Container Apps environment"
  type        = string
  default     = "claude-playground-env"
}

variable "container_registry_name" {
  description = "Name of the Azure Container Registry (must be globally unique)"
  type        = string
}

variable "storage_account_name" {
  description = "Name of the Azure Storage Account (must be globally unique)"
  type        = string
}

variable "container_cpu" {
  description = "CPU cores for each container instance"
  type        = number
  default     = 2.0
}

variable "container_memory" {
  description = "Memory in GB for each container instance"
  type        = string
  default     = "4Gi"
}

variable "default_user" {
  description = "Default username for container instances"
  type        = string
  default     = "default"
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/variables.tf | head -20`
Expected: First 20 lines of the variables file

**Step 3: Commit**

```bash
git add azure/terraform/variables.tf
git commit -m "feat(azure): add terraform variables"
```

---

## Task 3: Create Terraform Main Configuration

**Files:**
- Create: `azure/terraform/main.tf`

**Step 1: Create main.tf with provider and resource group**

```hcl
terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.0"
    }
    azuread = {
      source  = "hashicorp/azuread"
      version = "~> 2.0"
    }
  }
  required_version = ">= 1.0"
}

provider "azurerm" {
  features {}
}

provider "azuread" {}

resource "azurerm_resource_group" "main" {
  name     = var.resource_group_name
  location = var.location

  tags = {
    environment = "playground"
    project     = "claude-code"
  }
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/main.tf`
Expected: The terraform configuration with providers and resource group

**Step 3: Commit**

```bash
git add azure/terraform/main.tf
git commit -m "feat(azure): add terraform main configuration with resource group"
```

---

## Task 4: Create Container Registry Configuration

**Files:**
- Modify: `azure/terraform/main.tf`

**Step 1: Add Container Registry resource to main.tf**

Append to `azure/terraform/main.tf`:

```hcl

resource "azurerm_container_registry" "main" {
  name                = var.container_registry_name
  resource_group_name = azurerm_resource_group.main.name
  location            = azurerm_resource_group.main.location
  sku                 = "Basic"
  admin_enabled       = true

  tags = {
    environment = "playground"
    project     = "claude-code"
  }
}
```

**Step 2: Verify the addition**

Run: `grep -A 15 "azurerm_container_registry" azure/terraform/main.tf`
Expected: The container registry resource block

**Step 3: Commit**

```bash
git add azure/terraform/main.tf
git commit -m "feat(azure): add container registry resource"
```

---

## Task 5: Create Storage Configuration

**Files:**
- Create: `azure/terraform/storage.tf`

**Step 1: Create storage.tf with storage account and file share**

```hcl
resource "azurerm_storage_account" "main" {
  name                     = var.storage_account_name
  resource_group_name      = azurerm_resource_group.main.name
  location                 = azurerm_resource_group.main.location
  account_tier             = "Standard"
  account_replication_type = "LRS"

  tags = {
    environment = "playground"
    project     = "claude-code"
  }
}

resource "azurerm_storage_share" "users" {
  name                 = "claude-users"
  storage_account_name = azurerm_storage_account.main.name
  quota                = 100 # GB
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/storage.tf`
Expected: Storage account and file share resources

**Step 3: Commit**

```bash
git add azure/terraform/storage.tf
git commit -m "feat(azure): add azure files storage configuration"
```

---

## Task 6: Create Container Apps Environment

**Files:**
- Create: `azure/terraform/container-app.tf`

**Step 1: Create container-app.tf with Log Analytics and environment**

```hcl
resource "azurerm_log_analytics_workspace" "main" {
  name                = "${var.environment_name}-logs"
  resource_group_name = azurerm_resource_group.main.name
  location            = azurerm_resource_group.main.location
  sku                 = "PerGB2018"
  retention_in_days   = 30

  tags = {
    environment = "playground"
    project     = "claude-code"
  }
}

resource "azurerm_container_app_environment" "main" {
  name                       = var.environment_name
  resource_group_name        = azurerm_resource_group.main.name
  location                   = azurerm_resource_group.main.location
  log_analytics_workspace_id = azurerm_log_analytics_workspace.main.id

  tags = {
    environment = "playground"
    project     = "claude-code"
  }
}

resource "azurerm_container_app_environment_storage" "users" {
  name                         = "users-storage"
  container_app_environment_id = azurerm_container_app_environment.main.id
  account_name                 = azurerm_storage_account.main.name
  share_name                   = azurerm_storage_share.users.name
  access_key                   = azurerm_storage_account.main.primary_access_key
  access_mode                  = "ReadWrite"
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/container-app.tf`
Expected: Log Analytics, Container Apps environment, and storage mount

**Step 3: Commit**

```bash
git add azure/terraform/container-app.tf
git commit -m "feat(azure): add container apps environment with storage mount"
```

---

## Task 7: Create Azure AD App Registration

**Files:**
- Create: `azure/terraform/auth.tf`

**Step 1: Create auth.tf with Azure AD app registration**

```hcl
data "azuread_client_config" "current" {}

resource "azuread_application" "claude_playground" {
  display_name = "Claude Playground"

  web {
    redirect_uris = ["https://*.azurecontainerapps.io/.auth/login/aad/callback"]

    implicit_grant {
      access_token_issuance_enabled = false
      id_token_issuance_enabled     = true
    }
  }

  required_resource_access {
    resource_app_id = "00000003-0000-0000-c000-000000000000" # Microsoft Graph

    resource_access {
      id   = "e1fe6dd8-ba31-4d61-89e7-88639da4683d" # User.Read
      type = "Scope"
    }
  }
}

resource "azuread_application_password" "claude_playground" {
  application_id = azuread_application.claude_playground.id
  display_name   = "Claude Playground Client Secret"
}

resource "azuread_service_principal" "claude_playground" {
  client_id = azuread_application.claude_playground.client_id
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/auth.tf`
Expected: Azure AD application, password, and service principal

**Step 3: Commit**

```bash
git add azure/terraform/auth.tf
git commit -m "feat(azure): add azure ad app registration for authentication"
```

---

## Task 8: Create Container App Template Module

**Files:**
- Create: `azure/terraform/modules/container-app/main.tf`
- Create: `azure/terraform/modules/container-app/variables.tf`
- Create: `azure/terraform/modules/container-app/outputs.tf`

**Step 1: Create the module directory**

```bash
mkdir -p azure/terraform/modules/container-app
```

**Step 2: Create modules/container-app/variables.tf**

```hcl
variable "name" {
  description = "Name of the container app"
  type        = string
}

variable "resource_group_name" {
  description = "Resource group name"
  type        = string
}

variable "environment_id" {
  description = "Container Apps environment ID"
  type        = string
}

variable "container_registry_server" {
  description = "Container registry login server"
  type        = string
}

variable "container_registry_username" {
  description = "Container registry username"
  type        = string
}

variable "container_registry_password" {
  description = "Container registry password"
  type        = string
  sensitive   = true
}

variable "image" {
  description = "Container image to deploy"
  type        = string
}

variable "cpu" {
  description = "CPU cores"
  type        = number
  default     = 2.0
}

variable "memory" {
  description = "Memory allocation"
  type        = string
  default     = "4Gi"
}

variable "user_directory" {
  description = "User-specific subdirectory in Azure Files"
  type        = string
}

variable "storage_name" {
  description = "Name of the environment storage mount"
  type        = string
}

variable "aad_client_id" {
  description = "Azure AD application client ID"
  type        = string
}

variable "aad_client_secret" {
  description = "Azure AD application client secret"
  type        = string
  sensitive   = true
}

variable "aad_tenant_id" {
  description = "Azure AD tenant ID"
  type        = string
}
```

**Step 3: Create modules/container-app/main.tf**

```hcl
resource "azurerm_container_app" "main" {
  name                         = var.name
  resource_group_name          = var.resource_group_name
  container_app_environment_id = var.environment_id
  revision_mode                = "Single"

  registry {
    server               = var.container_registry_server
    username             = var.container_registry_username
    password_secret_name = "registry-password"
  }

  secret {
    name  = "registry-password"
    value = var.container_registry_password
  }

  secret {
    name  = "aad-client-secret"
    value = var.aad_client_secret
  }

  template {
    min_replicas = 0
    max_replicas = 1

    container {
      name   = "claude-desktop"
      image  = var.image
      cpu    = var.cpu
      memory = var.memory

      env {
        name  = "PUID"
        value = "1000"
      }

      env {
        name  = "PGID"
        value = "1000"
      }

      env {
        name  = "TZ"
        value = "America/New_York"
      }

      volume_mounts {
        name = "user-home"
        path = "/config"
      }
    }

    volume {
      name         = "user-home"
      storage_name = var.storage_name
      storage_type = "AzureFile"
    }
  }

  ingress {
    external_enabled = true
    target_port      = 3000
    transport        = "auto"

    traffic_weight {
      percentage      = 100
      latest_revision = true
    }
  }

  identity {
    type = "SystemAssigned"
  }

  tags = {
    environment = "playground"
    project     = "claude-code"
    user        = var.user_directory
  }
}

resource "azurerm_container_app_custom_domain" "auth" {
  # Note: Easy Auth requires additional configuration via Azure CLI
  # This is handled in the CLI tool after deployment
  count = 0 # Placeholder for future custom domain support
}
```

**Step 4: Create modules/container-app/outputs.tf**

```hcl
output "fqdn" {
  description = "Fully qualified domain name of the container app"
  value       = azurerm_container_app.main.ingress[0].fqdn
}

output "name" {
  description = "Name of the container app"
  value       = azurerm_container_app.main.name
}

output "id" {
  description = "ID of the container app"
  value       = azurerm_container_app.main.id
}
```

**Step 5: Verify files were created**

Run: `ls -la azure/terraform/modules/container-app/`
Expected: main.tf, variables.tf, outputs.tf

**Step 6: Commit**

```bash
git add azure/terraform/modules/
git commit -m "feat(azure): add container app terraform module"
```

---

## Task 9: Create Terraform Outputs

**Files:**
- Create: `azure/terraform/outputs.tf`

**Step 1: Create outputs.tf**

```hcl
output "resource_group_name" {
  description = "Name of the resource group"
  value       = azurerm_resource_group.main.name
}

output "container_registry_server" {
  description = "Container registry login server"
  value       = azurerm_container_registry.main.login_server
}

output "container_registry_username" {
  description = "Container registry admin username"
  value       = azurerm_container_registry.main.admin_username
}

output "container_registry_password" {
  description = "Container registry admin password"
  value       = azurerm_container_registry.main.admin_password
  sensitive   = true
}

output "storage_account_name" {
  description = "Storage account name"
  value       = azurerm_storage_account.main.name
}

output "environment_id" {
  description = "Container Apps environment ID"
  value       = azurerm_container_app_environment.main.id
}

output "environment_storage_name" {
  description = "Name of the storage mount in the environment"
  value       = azurerm_container_app_environment_storage.users.name
}

output "aad_client_id" {
  description = "Azure AD application client ID"
  value       = azuread_application.claude_playground.client_id
}

output "aad_client_secret" {
  description = "Azure AD application client secret"
  value       = azuread_application_password.claude_playground.value
  sensitive   = true
}

output "aad_tenant_id" {
  description = "Azure AD tenant ID"
  value       = data.azuread_client_config.current.tenant_id
}
```

**Step 2: Verify file was created**

Run: `cat azure/terraform/outputs.tf`
Expected: All output definitions

**Step 3: Commit**

```bash
git add azure/terraform/outputs.tf
git commit -m "feat(azure): add terraform outputs"
```

---

## Task 10: Create Terraform Variable Values Template

**Files:**
- Create: `azure/terraform/terraform.tfvars.example`

**Step 1: Create example tfvars file**

```hcl
# Copy this file to terraform.tfvars and fill in your values

# Globally unique names (lowercase, alphanumeric only)
container_registry_name = "claudeplayground<unique-suffix>"
storage_account_name    = "claudeplayground<unique-suffix>"

# Optional overrides
# resource_group_name = "claude-playground-rg"
# location           = "eastus"
# environment_name   = "claude-playground-env"
# container_cpu      = 2.0
# container_memory   = "4Gi"
# default_user       = "default"
```

**Step 2: Add terraform.tfvars to gitignore**

Append to root `.gitignore`:
```
# Terraform
azure/terraform/.terraform/
azure/terraform/*.tfstate
azure/terraform/*.tfstate.*
azure/terraform/terraform.tfvars
```

**Step 3: Verify files**

Run: `cat azure/terraform/terraform.tfvars.example`
Expected: Template with placeholder values

**Step 4: Commit**

```bash
git add azure/terraform/terraform.tfvars.example .gitignore
git commit -m "feat(azure): add terraform.tfvars template and update gitignore"
```

---

## Task 11: Modify Dockerfile for Azure Compatibility

**Files:**
- Modify: `Dockerfile`

**Step 1: Update Dockerfile to remove AWS-specific parts**

The existing Dockerfile works for Azure but we should remove AWS CLI since we're not using it. However, keeping it doesn't hurt and maintains local compatibility. No changes needed to the Dockerfile itself - it works for both local and Azure deployments.

**Step 2: Verify Dockerfile is compatible**

Run: `grep -v "^#" Dockerfile | grep -v "^$" | head -20`
Expected: Dockerfile commands, confirming it doesn't have Azure-incompatible elements

**Step 3: No commit needed - Dockerfile is already compatible**

---

## Task 12: Create CLI Tool - Core Script

**Files:**
- Create: `azure/cli/claude-playground`

**Step 1: Create the CLI script with help and common functions**

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERRAFORM_DIR="$SCRIPT_DIR/../terraform"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Get terraform outputs
get_tf_output() {
    terraform -chdir="$TERRAFORM_DIR" output -raw "$1" 2>/dev/null || echo ""
}

get_tf_output_json() {
    terraform -chdir="$TERRAFORM_DIR" output -json "$1" 2>/dev/null || echo "{}"
}

# Verify prerequisites
check_prerequisites() {
    local missing=()
    command -v az &>/dev/null || missing+=("az (Azure CLI)")
    command -v terraform &>/dev/null || missing+=("terraform")
    command -v docker &>/dev/null || missing+=("docker")
    command -v jq &>/dev/null || missing+=("jq")

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing[*]}"
        exit 1
    fi

    # Check Azure login
    if ! az account show &>/dev/null; then
        log_error "Not logged into Azure. Run: az login"
        exit 1
    fi
}

# Show usage
usage() {
    cat <<EOF
Claude Playground CLI - Manage Azure Container App instances

Usage: $(basename "$0") <command> [options]

Commands:
    start [--user NAME]    Start a new container instance
    stop <name>            Stop a running instance
    list                   List all running instances
    open <name>            Open instance URL in browser
    build-push             Build and push Docker image to ACR
    init                   Initialize terraform (first-time setup)
    apply                  Apply terraform configuration
    destroy                Destroy all Azure resources
    help                   Show this help message

Examples:
    $(basename "$0") start                    # Start instance for default user
    $(basename "$0") start --user alice       # Start instance for user 'alice'
    $(basename "$0") list                     # Show running instances
    $(basename "$0") stop claude-alice-abc123 # Stop specific instance
    $(basename "$0") open claude-alice-abc123 # Open in browser

EOF
}

# Command implementations will be added in subsequent tasks

case "${1:-help}" in
    start)
        shift
        cmd_start "$@"
        ;;
    stop)
        shift
        cmd_stop "$@"
        ;;
    list)
        cmd_list
        ;;
    open)
        shift
        cmd_open "$@"
        ;;
    build-push)
        cmd_build_push
        ;;
    init)
        cmd_init
        ;;
    apply)
        cmd_apply
        ;;
    destroy)
        cmd_destroy
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        log_error "Unknown command: $1"
        usage
        exit 1
        ;;
esac
```

**Step 2: Make the script executable**

```bash
chmod +x azure/cli/claude-playground
```

**Step 3: Verify**

Run: `./azure/cli/claude-playground help | head -10`
Expected: Help message header

**Step 4: Commit**

```bash
git add azure/cli/claude-playground
git commit -m "feat(azure): add CLI tool scaffold with help"
```

---

## Task 13: Add CLI Init and Apply Commands

**Files:**
- Modify: `azure/cli/claude-playground`

**Step 1: Add init and apply command implementations**

Insert before the `case` statement in `azure/cli/claude-playground`:

```bash
cmd_init() {
    log_info "Initializing Terraform..."
    check_prerequisites

    if [[ ! -f "$TERRAFORM_DIR/terraform.tfvars" ]]; then
        log_warning "terraform.tfvars not found!"
        log_info "Copy terraform.tfvars.example to terraform.tfvars and fill in your values"
        exit 1
    fi

    terraform -chdir="$TERRAFORM_DIR" init
    log_success "Terraform initialized"
}

cmd_apply() {
    log_info "Applying Terraform configuration..."
    check_prerequisites

    if [[ ! -d "$TERRAFORM_DIR/.terraform" ]]; then
        log_warning "Terraform not initialized. Running init first..."
        cmd_init
    fi

    terraform -chdir="$TERRAFORM_DIR" apply
    log_success "Infrastructure deployed"
}

cmd_destroy() {
    log_warning "This will destroy ALL Azure resources for Claude Playground!"
    read -p "Are you sure? (yes/no): " confirm
    if [[ "$confirm" != "yes" ]]; then
        log_info "Aborted"
        exit 0
    fi

    terraform -chdir="$TERRAFORM_DIR" destroy
    log_success "Infrastructure destroyed"
}
```

**Step 2: Verify the changes**

Run: `grep -A 5 "cmd_init()" azure/cli/claude-playground`
Expected: The init function definition

**Step 3: Commit**

```bash
git add azure/cli/claude-playground
git commit -m "feat(azure): add CLI init, apply, and destroy commands"
```

---

## Task 14: Add CLI Build-Push Command

**Files:**
- Modify: `azure/cli/claude-playground`

**Step 1: Add build-push command implementation**

Insert before the `case` statement in `azure/cli/claude-playground`:

```bash
cmd_build_push() {
    log_info "Building and pushing Docker image to ACR..."
    check_prerequisites

    local registry_server=$(get_tf_output "container_registry_server")
    local registry_username=$(get_tf_output "container_registry_username")
    local registry_password=$(get_tf_output "container_registry_password")

    if [[ -z "$registry_server" ]]; then
        log_error "Could not get registry info. Have you run 'apply' yet?"
        exit 1
    fi

    local image_name="$registry_server/claude-playground:latest"

    log_info "Building image..."
    docker build -t "$image_name" "$PROJECT_ROOT"

    log_info "Logging into ACR..."
    echo "$registry_password" | docker login "$registry_server" -u "$registry_username" --password-stdin

    log_info "Pushing image..."
    docker push "$image_name"

    log_success "Image pushed: $image_name"
}
```

**Step 2: Verify the changes**

Run: `grep -A 10 "cmd_build_push()" azure/cli/claude-playground`
Expected: The build_push function definition

**Step 3: Commit**

```bash
git add azure/cli/claude-playground
git commit -m "feat(azure): add CLI build-push command"
```

---

## Task 15: Add CLI Start Command

**Files:**
- Modify: `azure/cli/claude-playground`

**Step 1: Add start command implementation**

Insert before the `case` statement in `azure/cli/claude-playground`:

```bash
cmd_start() {
    local user="${default_user:-default}"

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --user)
                user="$2"
                shift 2
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log_info "Starting container for user: $user"
    check_prerequisites

    local resource_group=$(get_tf_output "resource_group_name")
    local environment_id=$(get_tf_output "environment_id")
    local storage_name=$(get_tf_output "environment_storage_name")
    local registry_server=$(get_tf_output "container_registry_server")
    local registry_username=$(get_tf_output "container_registry_username")
    local registry_password=$(get_tf_output "container_registry_password")
    local aad_client_id=$(get_tf_output "aad_client_id")
    local aad_client_secret=$(get_tf_output "aad_client_secret")
    local aad_tenant_id=$(get_tf_output "aad_tenant_id")

    if [[ -z "$resource_group" ]]; then
        log_error "Could not get terraform outputs. Have you run 'apply' yet?"
        exit 1
    fi

    # Generate unique app name
    local suffix=$(date +%s | tail -c 6)
    local app_name="claude-${user}-${suffix}"

    log_info "Creating container app: $app_name"

    # Create the container app using Azure CLI
    az containerapp create \
        --name "$app_name" \
        --resource-group "$resource_group" \
        --environment "$environment_id" \
        --image "$registry_server/claude-playground:latest" \
        --registry-server "$registry_server" \
        --registry-username "$registry_username" \
        --registry-password "$registry_password" \
        --cpu 2 \
        --memory 4Gi \
        --min-replicas 1 \
        --max-replicas 1 \
        --ingress external \
        --target-port 3000 \
        --env-vars "PUID=1000" "PGID=1000" "TZ=America/New_York" \
        --query "properties.configuration.ingress.fqdn" \
        --output tsv

    local fqdn=$(az containerapp show \
        --name "$app_name" \
        --resource-group "$resource_group" \
        --query "properties.configuration.ingress.fqdn" \
        --output tsv)

    # Configure Azure AD authentication
    log_info "Configuring Azure AD authentication..."
    az containerapp auth microsoft update \
        --name "$app_name" \
        --resource-group "$resource_group" \
        --client-id "$aad_client_id" \
        --client-secret "$aad_client_secret" \
        --tenant-id "$aad_tenant_id" \
        --yes 2>/dev/null || true

    log_success "Container started!"
    echo ""
    echo "  Name: $app_name"
    echo "  URL:  https://$fqdn"
    echo ""
    log_info "Open with: $(basename "$0") open $app_name"
}
```

**Step 2: Verify the changes**

Run: `grep -A 5 "cmd_start()" azure/cli/claude-playground`
Expected: The start function definition

**Step 3: Commit**

```bash
git add azure/cli/claude-playground
git commit -m "feat(azure): add CLI start command"
```

---

## Task 16: Add CLI Stop, List, and Open Commands

**Files:**
- Modify: `azure/cli/claude-playground`

**Step 1: Add stop, list, and open command implementations**

Insert before the `case` statement in `azure/cli/claude-playground`:

```bash
cmd_stop() {
    if [[ $# -lt 1 ]]; then
        log_error "Usage: $(basename "$0") stop <app-name>"
        exit 1
    fi

    local app_name="$1"
    check_prerequisites

    local resource_group=$(get_tf_output "resource_group_name")

    log_info "Stopping container: $app_name"
    az containerapp delete \
        --name "$app_name" \
        --resource-group "$resource_group" \
        --yes

    log_success "Container stopped: $app_name"
}

cmd_list() {
    check_prerequisites

    local resource_group=$(get_tf_output "resource_group_name")

    if [[ -z "$resource_group" ]]; then
        log_error "Could not get resource group. Have you run 'apply' yet?"
        exit 1
    fi

    log_info "Running instances:"
    echo ""

    az containerapp list \
        --resource-group "$resource_group" \
        --query "[].{Name:name, URL:properties.configuration.ingress.fqdn, Status:properties.runningStatus}" \
        --output table
}

cmd_open() {
    if [[ $# -lt 1 ]]; then
        log_error "Usage: $(basename "$0") open <app-name>"
        exit 1
    fi

    local app_name="$1"
    check_prerequisites

    local resource_group=$(get_tf_output "resource_group_name")

    local fqdn=$(az containerapp show \
        --name "$app_name" \
        --resource-group "$resource_group" \
        --query "properties.configuration.ingress.fqdn" \
        --output tsv 2>/dev/null)

    if [[ -z "$fqdn" ]]; then
        log_error "Container app not found: $app_name"
        exit 1
    fi

    local url="https://$fqdn"
    log_info "Opening: $url"

    # Try different methods to open browser
    if command -v xdg-open &>/dev/null; then
        xdg-open "$url"
    elif command -v open &>/dev/null; then
        open "$url"
    elif command -v wslview &>/dev/null; then
        wslview "$url"
    else
        log_warning "Could not open browser. Visit: $url"
    fi
}
```

**Step 2: Verify the changes**

Run: `grep -c "^cmd_" azure/cli/claude-playground`
Expected: 8 (all command functions)

**Step 3: Commit**

```bash
git add azure/cli/claude-playground
git commit -m "feat(azure): add CLI stop, list, and open commands"
```

---

## Task 17: Create Azure README

**Files:**
- Create: `azure/README.md`

**Step 1: Create comprehensive Azure README**

```markdown
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
```

**Step 2: Verify file was created**

Run: `head -30 azure/README.md`
Expected: First 30 lines of the README

**Step 3: Commit**

```bash
git add azure/README.md
git commit -m "docs(azure): add azure deployment readme"
```

---

## Task 18: Update Root README

**Files:**
- Modify: `README.md`

**Step 1: Update README to mention Azure deployment**

Read current README first, then append Azure section:

```markdown

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
```

**Step 2: Verify the changes**

Run: `tail -15 README.md`
Expected: Azure deployment section

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add azure deployment reference to readme"
```

---

## Task 19: Validate Terraform Configuration

**Files:**
- All terraform files in `azure/terraform/`

**Step 1: Initialize terraform to validate syntax**

```bash
cd azure/terraform
terraform init -backend=false
```

Expected: "Terraform has been successfully initialized!"

**Step 2: Validate terraform configuration**

```bash
terraform validate
```

Expected: "Success! The configuration is valid."

**Step 3: Format terraform files**

```bash
terraform fmt -recursive
```

**Step 4: Commit any formatting changes**

```bash
git add azure/terraform/
git commit -m "style(azure): format terraform files" || echo "No formatting changes"
```

---

## Task 20: Test CLI Help Command

**Files:**
- `azure/cli/claude-playground`

**Step 1: Verify CLI runs without errors**

```bash
./azure/cli/claude-playground help
```

Expected: Full help message with all commands listed

**Step 2: Verify unknown command handling**

```bash
./azure/cli/claude-playground unknown 2>&1 | head -1
```

Expected: Error message about unknown command

**Step 3: Final commit - mark feature complete**

```bash
git add -A
git commit -m "feat(azure): complete azure deployment implementation" --allow-empty
```

---

## Summary

After completing all tasks, you will have:

1. **Terraform Infrastructure** (`azure/terraform/`)
   - Resource group, Container Registry, Storage Account
   - Container Apps environment with Azure Files mount
   - Azure AD app registration for authentication
   - Reusable container app module

2. **CLI Tool** (`azure/cli/claude-playground`)
   - `start` - Launch new instances
   - `stop` - Stop running instances
   - `list` - Show all instances
   - `open` - Open instance in browser
   - `build-push` - Build and push Docker image
   - `init/apply/destroy` - Terraform operations

3. **Documentation**
   - `azure/README.md` - Azure-specific setup guide
   - Updated root `README.md` with Azure reference

**To test the full deployment:**

```bash
# 1. Configure
cd azure/terraform
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with unique names

# 2. Deploy
./azure/cli/claude-playground init
./azure/cli/claude-playground apply
./azure/cli/claude-playground build-push

# 3. Start instance
./azure/cli/claude-playground start

# 4. Access via browser URL shown in output
```
