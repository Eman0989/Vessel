export type NodeStatus =
  | 'joining'
  | 'ready'
  | 'draining'
  | 'unreachable'

export type InstanceStatus =
  | 'pending'
  | 'assigned'
  | 'starting'
  | 'running'
  | 'stopping'
  | 'succeeded'
  | 'failed'
  | 'lost'
  | 'cancelled'

export interface ResourceCapacity {
  cpu_millis: number
  memory_bytes: number
  max_instances: number
}

export interface ResourceRequest {
  cpu_millis: number
  memory_bytes: number
}

export interface ClusterNode {
  id: string
  name: string
  region: string
  status: NodeStatus
  capacity: ResourceCapacity
  allocated: ResourceRequest
  allocated_instances: number
  labels: Record<string, string>
}

export interface ClusterInstance {
  id: string
  deployment_id: string
  workload_id: string
  node_id: string | null
  status: InstanceStatus
  resources: ResourceRequest
  restart_count: number
}
