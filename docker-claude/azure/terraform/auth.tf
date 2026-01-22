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
