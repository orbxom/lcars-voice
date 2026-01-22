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
