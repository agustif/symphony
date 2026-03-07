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
  value       = "${var.name}"
}

output "service_port" {
  description = "Port of the Kubernetes service"
  value       = 8080
}

output "endpoint" {
  description = "Internal endpoint URL"
  value       = "http://${var.name}.${var.namespace}.svc.cluster.local:8080"
}

output "ingress_host" {
  description = "Ingress hostname if enabled"
  value       = var.ingress_enabled ? var.ingress_host : null
}

output "persistence_claim_name" {
  description = "Name of the PVC if persistence is enabled"
  value       = var.persistence_enabled ? "${var.name}-workspaces" : null
}

output "secret_name" {
  description = "Name of the secrets resource"
  value       = kubernetes_secret.symphony.metadata[0].name
  sensitive   = true
}
