output "release_name" {
  description = "Name of the Helm release"
  value       = helm_release.symphony.name
}

output "namespace" {
  description = "Kubernetes namespace"
  value       = var.namespace
}

output "service_name" {
  description = "Name of the Kubernetes service"
  value       = local.release_fullname
}

output "service_port" {
  description = "Port of the Kubernetes service"
  value       = var.service_port
}

output "endpoint" {
  description = "Internal endpoint URL"
  value       = "http://${local.release_fullname}.${var.namespace}.svc.cluster.local:${var.service_port}"
}

output "ingress_host" {
  description = "Ingress hostname if enabled"
  value       = var.ingress_enabled ? var.ingress_host : null
}

output "persistence_claim_name" {
  description = "Name of the PVC if persistence is enabled"
  value       = var.persistence_enabled ? "${local.release_fullname}-workspaces" : null
}

output "secret_name" {
  description = "Name of the secrets resource"
  value       = kubernetes_secret.symphony.metadata[0].name
  sensitive   = true
}
