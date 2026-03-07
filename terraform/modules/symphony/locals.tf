locals {
  default_workflow = <<-EOT
    ---
    tracker:
      kind: linear
      project_slug: "your-project"
      active_states:
        - Todo
        - In Progress
      terminal_states:
        - Done
        - Closed
    polling:
      interval_ms: 30000
    workspace:
      root: /var/lib/symphony/workspaces
    agent:
      max_concurrent_agents: 5
      max_turns: 10
    codex:
      command: echo "Configure your codex command"
      approval_policy: never
    server:
      port: 8080
    ---
    
    Configure your workflow prompt here.
  EOT

  workflow = var.workflow_config != "" ? var.workflow_config : local.default_workflow

  helm_values = merge(
    {
      image = {
        tag = var.image_tag
      }
      replicaCount = var.replica_count
      resources    = var.resources
      persistence = {
        enabled       = var.persistence_enabled
        size          = var.persistence_size
        storageClass  = var.persistence_storage_class
      }
      config = {
        workflow = local.workflow
      }
      secrets = {
        existingSecret = kubernetes_secret.symphony.metadata[0].name
      }
      ingress = {
        enabled   = var.ingress_enabled
        className = var.ingress_class
        hosts = [
          {
            host  = var.ingress_host
            paths = [{ path = "/", pathType = "ImplementationSpecific" }]
          }
        ]
        tls = var.ingress_tls_enabled ? [
          {
            secretName = var.ingress_tls_secret
            hosts      = [var.ingress_host]
          }
        ] : []
        annotations = var.ingress_annotations
      }
      monitoring = {
        enabled              = var.monitoring_enabled
        serviceMonitor       = { enabled = var.service_monitor_enabled }
        prometheusRule       = { enabled = var.prometheus_rule_enabled }
      }
      autoscaling = {
        enabled                    = var.autoscaling_enabled
        minReplicas                = var.autoscaling_min_replicas
        maxReplicas                = var.autoscaling_max_replicas
        targetCPUUtilizationPercentage = var.autoscaling_target_cpu
      }
      nodeSelector = var.node_selector
      tolerations  = var.tolerations
      affinity     = var.affinity
      extraEnvVars = var.extra_env_vars
    },
    var.helm_values
  )
}
