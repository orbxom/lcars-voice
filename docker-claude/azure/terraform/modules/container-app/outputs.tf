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
