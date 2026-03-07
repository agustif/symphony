# Symphony Helm Chart

A Helm chart for deploying Symphony - Autonomous agent orchestration service on Kubernetes.

## Prerequisites

- Kubernetes 1.24+
- Helm 3.12+
- PV provisioner support in the underlying infrastructure (if persistence is enabled)

## Installing the Chart

### Add the Helm repository (when published)
```bash
helm repo add symphony https://charts.symphony.dev
helm repo update
```

### Install with default configuration
```bash
helm install symphony symphony/symphony \
  --set secrets.linearApiKey="your-api-key"
```

### Install with custom values
```bash
helm install symphony symphony/symphony \
  --values custom-values.yaml
```

## Configuration

See [values.yaml](values.yaml) for the full list of configurable parameters.

### Required Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `secrets.linearApiKey` | Linear API key | `""` |

### Common Configuration

```yaml
# Custom workflow configuration
config:
  workflow: |
    ---
    tracker:
      kind: linear
      project_slug: "my-project"
    # ... rest of workflow

# Resource limits
resources:
  limits:
    cpu: 2000m
    memory: 2Gi

# Enable ingress
ingress:
  enabled: true
  className: nginx
  hosts:
    - host: symphony.example.com

# Enable monitoring
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
```

## Uninstalling

```bash
helm uninstall symphony
```

## Persistence

The chart supports persistent storage for workspaces. By default, it creates a PVC of 10Gi. To disable persistence:

```yaml
persistence:
  enabled: false
```

## Monitoring

### Prometheus ServiceMonitor

Enable Prometheus scraping:

```yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
```

### PrometheusRule (Alerts)

Enable default alerts:

```yaml
monitoring:
  prometheusRule:
    enabled: true
```

## Autoscaling

Enable Horizontal Pod Autoscaler:

```yaml
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 80
```

## Security

The chart runs Symphony as a non-root user with:
- Read-only root filesystem
- Dropped capabilities
- Security context constraints

## Development

### Template rendering
```bash
helm template symphony . --debug
```

### Lint chart
```bash
helm lint .
```
