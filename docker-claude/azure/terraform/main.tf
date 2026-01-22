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
