resource "kubernetes_namespace" "symphony" {
  count = var.namespace != "default" ? 1 : 0

  metadata {
    name = var.namespace
  }
}

resource "kubernetes_secret" "symphony" {
  metadata {
    name      = "${var.name}-secrets"
    namespace = var.namespace
  }

  data = {
    "linear-api-key" = var.linear_api_key
  }

  depends_on = [kubernetes_namespace.symphony]
}

resource "helm_release" "symphony" {
  name       = var.name
  namespace  = var.namespace
  chart      = "${path.module}/../../../helm/symphony"
  version    = var.chart_version
  values     = [yamlencode(local.helm_values)]

  depends_on = [kubernetes_secret.symphony]
}
