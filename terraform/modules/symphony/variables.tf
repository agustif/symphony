variable "name" {
  description = "Name of the Symphony deployment"
  type        = string
  default     = "symphony"
}

variable "namespace" {
  description = "Kubernetes namespace for Symphony"
  type        = string
  default     = "default"
}

variable "chart_version" {
  description = "Version of the Helm chart to install"
  type        = string
  default     = "0.1.0"
}

variable "image_tag" {
  description = "Docker image tag to use"
  type        = string
  default     = "latest"
}

variable "service_port" {
  description = "Port exposed by the Symphony service and HTTP listener"
  type        = number
  default     = 8080
}

variable "replica_count" {
  description = "Number of replicas"
  type        = number
  default     = 1
}

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    limits = object({
      cpu    = string
      memory = string
    })
    requests = object({
      cpu    = string
      memory = string
    })
  })
  default = {
    limits = {
      cpu    = "1000m"
      memory = "1Gi"
    }
    requests = {
      cpu    = "100m"
      memory = "256Mi"
    }
  }
}

variable "linear_api_key" {
  description = "Linear API key (sensitive)"
  type        = string
  sensitive   = true
}

variable "workflow_config" {
  description = "Workflow configuration content"
  type        = string
  default     = ""
}

variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "persistence_size" {
  description = "Size of persistent volume"
  type        = string
  default     = "10Gi"
}

variable "persistence_storage_class" {
  description = "Storage class for persistent volume"
  type        = string
  default     = ""
}

variable "ingress_enabled" {
  description = "Enable ingress"
  type        = bool
  default     = false
}

variable "ingress_host" {
  description = "Ingress hostname"
  type        = string
  default     = "symphony.local"
}

variable "ingress_class" {
  description = "Ingress class name"
  type        = string
  default     = "nginx"
}

variable "ingress_annotations" {
  description = "Ingress annotations"
  type        = map(string)
  default     = {}
}

variable "ingress_tls_enabled" {
  description = "Enable TLS for ingress"
  type        = bool
  default     = false
}

variable "ingress_tls_secret" {
  description = "TLS secret name"
  type        = string
  default     = ""
}

variable "monitoring_enabled" {
  description = "Enable monitoring"
  type        = bool
  default     = true
}

variable "service_monitor_enabled" {
  description = "Enable ServiceMonitor for Prometheus"
  type        = bool
  default     = false
}

variable "prometheus_rule_enabled" {
  description = "Enable PrometheusRule"
  type        = bool
  default     = false
}

variable "autoscaling_enabled" {
  description = "Enable HPA"
  type        = bool
  default     = false
}

variable "autoscaling_min_replicas" {
  description = "Minimum number of replicas"
  type        = number
  default     = 1
}

variable "autoscaling_max_replicas" {
  description = "Maximum number of replicas"
  type        = number
  default     = 10
}

variable "autoscaling_target_cpu" {
  description = "Target CPU utilization percentage"
  type        = number
  default     = 80
}

variable "node_selector" {
  description = "Node selector for pod scheduling"
  type        = map(string)
  default     = {}
}

variable "tolerations" {
  description = "Tolerations for pod scheduling"
  type        = list(any)
  default     = []
}

variable "affinity" {
  description = "Affinity rules for pod scheduling"
  type        = any
  default     = {}
}

variable "extra_env_vars" {
  description = "Extra environment variables"
  type        = list(map(string))
  default     = []
}

variable "helm_values" {
  description = "Additional Helm values as map"
  type        = any
  default     = {}
}
