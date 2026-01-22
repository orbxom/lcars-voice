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
