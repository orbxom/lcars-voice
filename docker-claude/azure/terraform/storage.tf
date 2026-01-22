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
