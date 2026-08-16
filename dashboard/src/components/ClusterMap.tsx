import './ClusterMap.css'

import type {
  ClusterInstance,
  ClusterNode,
  InstanceStatus,
  NodeStatus,
} from '../types/cluster'

interface ClusterMapProps {
  nodes: ClusterNode[]
  instances: ClusterInstance[]
}

interface NodePosition {
  x: number
  y: number
}

const MAX_VISIBLE_NODES = 8

const ACTIVE_INSTANCE_STATUSES = new Set<InstanceStatus>([
  'pending',
  'assigned',
  'starting',
  'running',
  'stopping',
])

function nodePosition(index: number, total: number): NodePosition {
  if (total <= 1) {
    return { x: 50, y: 18 }
  }

  const angle = -Math.PI / 2 + (index * Math.PI * 2) / total

  return {
    x: 50 + Math.cos(angle) * 38,
    y: 50 + Math.sin(angle) * 34,
  }
}

function statusLabel(status: NodeStatus): string {
  switch (status) {
    case 'ready':
      return 'Ready'
    case 'joining':
      return 'Joining'
    case 'draining':
      return 'Draining'
    case 'unreachable':
      return 'Unreachable'
  }
}

function allocationPercent(node: ClusterNode): number {
  if (node.capacity.max_instances === 0) {
    return 0
  }

  return Math.min(
    100,
    Math.round(
      (node.allocated_instances / node.capacity.max_instances) * 100,
    ),
  )
}

export function ClusterMap({
  nodes,
  instances,
}: ClusterMapProps) {
  const visibleNodes = nodes.slice(0, MAX_VISIBLE_NODES)
  const hiddenNodeCount = Math.max(0, nodes.length - visibleNodes.length)

  const activeInstancesByNode = new Map<string, number>()

  for (const instance of instances) {
    if (
      instance.node_id &&
      ACTIVE_INSTANCE_STATUSES.has(instance.status)
    ) {
      activeInstancesByNode.set(
        instance.node_id,
        (activeInstancesByNode.get(instance.node_id) ?? 0) + 1,
      )
    }
  }

  return (
    <div className="cluster-map">
      <div className="cluster-map__grid" aria-hidden="true" />

      {visibleNodes.length > 0 && (
        <svg
          className="cluster-map__edges"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {visibleNodes.map((node, index) => {
            const position = nodePosition(index, visibleNodes.length)

            return (
              <line
                key={node.id}
                x1="50"
                y1="50"
                x2={position.x}
                y2={position.y}
                className={`cluster-map__edge cluster-map__edge--${node.status}`}
              />
            )
          })}
        </svg>
      )}

      <div className="cluster-map__hub">
        <span className="cluster-map__hub-ring" aria-hidden="true" />

        <div className="cluster-map__hub-core">
          <span>VESSEL</span>
          <strong>CONTROL</strong>
        </div>

        <span className="cluster-map__hub-caption">
          {nodes.length} worker{nodes.length === 1 ? '' : 's'}
        </span>
      </div>

      {visibleNodes.map((node, index) => {
        const position = nodePosition(index, visibleNodes.length)
        const activeInstances =
          activeInstancesByNode.get(node.id) ?? 0
        const allocation = allocationPercent(node)

        return (
          <article
            className={`cluster-node cluster-node--${node.status}`}
            key={node.id}
            style={{
              left: `${position.x}%`,
              top: `${position.y}%`,
            }}
            title={`${node.id} · ${statusLabel(node.status)}`}
          >
            <div className="cluster-node__header">
              <span className="cluster-node__status-dot" />

              <strong>{node.name}</strong>

              <span className="cluster-node__state">
                {statusLabel(node.status)}
              </span>
            </div>

            <div className="cluster-node__meta">
              <span>{node.region}</span>
              <span>{activeInstances} active</span>
            </div>

            <div
              className="cluster-node__capacity"
              aria-label={`${allocation}% instance capacity allocated`}
            >
              <span style={{ width: `${allocation}%` }} />
            </div>
          </article>
        )
      })}

      {nodes.length === 0 && (
        <div className="cluster-map__empty">
          <strong>No workers registered</strong>
          <span>
            Nodes will appear here when they join the control plane.
          </span>
        </div>
      )}

      {hiddenNodeCount > 0 && (
        <div className="cluster-map__overflow">
          +{hiddenNodeCount} additional node
          {hiddenNodeCount === 1 ? '' : 's'}
        </div>
      )}
    </div>
  )
}
