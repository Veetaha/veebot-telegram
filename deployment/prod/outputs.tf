output "server" {
  value = {
    ip                 = module.hetzner.server_ip
    status             = module.hetzner.server_status
    volume_mount_point = module.hetzner.volume_mount_point
  }
}

output "grafana_cloud_stack" {
  value = {
    id                = grafana_cloud_stack.this.id
    status            = grafana_cloud_stack.this.status
    logs_status       = grafana_cloud_stack.this.logs_status
    prometheus_status = grafana_cloud_stack.this.prometheus_status
  }
}

output "grafana_loki_creds" {
  sensitive = true
  value = {
    loki_url       = grafana_cloud_stack.this.logs_url
    loki_ursername = grafana_cloud_stack.this.logs_user_id
    loki_password  = var.grafana_cloud_api_key
  }
}
