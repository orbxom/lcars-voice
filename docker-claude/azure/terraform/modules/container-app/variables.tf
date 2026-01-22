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
