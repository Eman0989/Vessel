export interface NodeMetrics {
  total: number
  joining: number
  ready: number
  draining: number
  unreachable: number
}

export interface DeploymentMetrics {
  total: number
  pending: number
  progressing: number
  healthy: number
  degraded: number
  failed: number
  stopped: number
  desired_replicas: number
  autoscaling_enabled: number
  canary_active: number
}

export interface InstanceMetrics {
  total: number
  active: number
  pending: number
  assigned: number
  starting: number
  running: number
  stopping: number
  succeeded: number
  failed: number
  lost: number
  cancelled: number
  restart_count: number
}

export interface ResourceMetrics {
  capacity_cpu_millis: number
  allocated_cpu_millis: number
  available_cpu_millis: number

  capacity_memory_bytes: number
  allocated_memory_bytes: number
  available_memory_bytes: number

  max_instances: number
  allocated_instances: number
  available_instances: number
}

export interface ClusterMetrics {
  nodes: NodeMetrics
  deployments: DeploymentMetrics
  instances: InstanceMetrics
  resources: ResourceMetrics
}
