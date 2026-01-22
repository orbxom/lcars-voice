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
