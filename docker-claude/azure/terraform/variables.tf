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
