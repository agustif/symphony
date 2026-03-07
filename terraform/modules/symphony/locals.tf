locals {
  chart_name = "symphony"
  fullname_override = trimspace(try(var.helm_values.fullnameOverride, ""))
  name_override     = trimspace(try(var.helm_values.nameOverride, ""))

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

  resolved_chart_name = local.name_override != "" ? local.name_override : local.chart_name
  release_fullname = local.fullname_override != "" ? local.fullname_override : (
    strcontains(var.name, local.resolved_chart_name) ? var.name : "${var.name}-${local.resolved_chart_name}"
  )

  default_ingress_hosts = [
    {
      host  = var.ingress_host
      paths = [{ path = "/", pathType = "ImplementationSpecific" }]
    }
  ]

  default_ingress_tls = var.ingress_tls_enabled ? [
    {
      secretName = var.ingress_tls_secret
      hosts      = [var.ingress_host]
    }
  ] : []

  helm_values = merge(
    var.helm_values,
    {
      image = merge(
        try(var.helm_values.image, {}),
        {
          tag = var.image_tag
        }
      )
      replicaCount = var.replica_count
      resources    = var.resources
      service = merge(
        try(var.helm_values.service, {}),
        {
          port = var.service_port
        }
      )
      persistence = merge(
        try(var.helm_values.persistence, {}),
        {
          enabled      = var.persistence_enabled
          size         = var.persistence_size
          storageClass = var.persistence_storage_class
        }
      )
      config = merge(
        try(var.helm_values.config, {}),
        {
          workflow = try(var.helm_values.config.workflow, local.workflow)
          server = merge(
            try(var.helm_values.config.server, {}),
            {
              port = var.service_port
            }
          )
        }
      )
      secrets = merge(
        try(var.helm_values.secrets, {}),
        {
          existingSecret = kubernetes_secret.symphony.metadata[0].name
        }
      )
      ingress = merge(
        try(var.helm_values.ingress, {}),
        {
          enabled     = var.ingress_enabled
          className   = var.ingress_class
          hosts       = try(var.helm_values.ingress.hosts, local.default_ingress_hosts)
          tls         = try(var.helm_values.ingress.tls, local.default_ingress_tls)
          annotations = merge(var.ingress_annotations, try(var.helm_values.ingress.annotations, {}))
        }
      )
      monitoring = merge(
        try(var.helm_values.monitoring, {}),
        {
          enabled = var.monitoring_enabled
          serviceMonitor = merge(
            try(var.helm_values.monitoring.serviceMonitor, {}),
            {
              enabled = var.service_monitor_enabled
            }
          )
          prometheusRule = merge(
            try(var.helm_values.monitoring.prometheusRule, {}),
            {
              enabled = var.prometheus_rule_enabled
            }
          )
        }
      )
      autoscaling = merge(
        try(var.helm_values.autoscaling, {}),
        {
          enabled                        = var.autoscaling_enabled
          minReplicas                    = var.autoscaling_min_replicas
          maxReplicas                    = var.autoscaling_max_replicas
          targetCPUUtilizationPercentage = var.autoscaling_target_cpu
        }
      )
      nodeSelector = try(var.helm_values.nodeSelector, var.node_selector)
      tolerations  = try(var.helm_values.tolerations, var.tolerations)
      affinity     = try(var.helm_values.affinity, var.affinity)
      extraEnvVars = try(var.helm_values.extraEnvVars, var.extra_env_vars)
    }
  )
}
