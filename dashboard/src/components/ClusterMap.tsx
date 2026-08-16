import { useEffect, useRef, useState } from 'react'

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

interface FailureEvent {
  eventId: string
  nodeId: string
  nodeName: string
  lostInstances: number
}

const MAX_VISIBLE_NODES = 8
const FAILURE_EVENT_DURATION_MS = 4_200

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

function percentage(allocated: number, capacity: number): number {
  if (capacity === 0) {
    return 0
  }

  return Math.min(
    100,
    Math.round((allocated / capacity) * 100),
  )
}

function allocationPercent(node: ClusterNode): number {
  return percentage(
    node.allocated_instances,
    node.capacity.max_instances,
  )
}

function memoryLabel(bytes: number): string {
  if (bytes === 0) {
    return '0 B'
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  )

  const value = bytes / 1024 ** exponent

  return `${value.toFixed(value >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`
}

export function ClusterMap({
  nodes,
  instances,
}: ClusterMapProps) {
  const [selectedNodeId, setSelectedNodeId] =
    useState<string | null>(null)
  const [failureEvent, setFailureEvent] =
    useState<FailureEvent | null>(null)

  const previousNodeStatuses =
    useRef<Map<string, NodeStatus> | null>(null)
  const previousInstanceStatuses =
    useRef<Map<string, InstanceStatus> | null>(null)

  const visibleNodes = nodes.slice(0, MAX_VISIBLE_NODES)
  const hiddenNodeCount = Math.max(
    0,
    nodes.length - visibleNodes.length,
  )

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

  useEffect(() => {
    const currentNodeStatuses = new Map(
      nodes.map((node) => [node.id, node.status]),
    )

    const currentInstanceStatuses = new Map(
      instances.map((instance) => [
        instance.id,
        instance.status,
      ]),
    )

    const previousNodes = previousNodeStatuses.current
    const previousInstances = previousInstanceStatuses.current

    if (previousNodes) {
      const newlyUnreachable = nodes.find((node) => {
        const previousStatus = previousNodes.get(node.id)

        return (
          node.status === 'unreachable' &&
          previousStatus !== undefined &&
          previousStatus !== 'unreachable'
        )
      })

      if (newlyUnreachable) {
        const newlyLostInstances = instances.filter(
          (instance) =>
            instance.node_id === newlyUnreachable.id &&
            instance.status === 'lost' &&
            previousInstances?.get(instance.id) !== 'lost',
        ).length

        setFailureEvent({
          eventId: `${newlyUnreachable.id}:${Date.now()}`,
          nodeId: newlyUnreachable.id,
          nodeName: newlyUnreachable.name,
          lostInstances: newlyLostInstances,
        })
      }
    }

    previousNodeStatuses.current = currentNodeStatuses
    previousInstanceStatuses.current = currentInstanceStatuses
  }, [nodes, instances])

  useEffect(() => {
    if (!failureEvent) {
      return
    }

    const timeout = window.setTimeout(() => {
      setFailureEvent(null)
    }, FAILURE_EVENT_DURATION_MS)

    return () => {
      window.clearTimeout(timeout)
    }
  }, [failureEvent])

  const selectedNode =
    visibleNodes.find((node) => node.id === selectedNodeId) ?? null

  const selectedActiveInstances = selectedNode
    ? activeInstancesByNode.get(selectedNode.id) ?? 0
    : 0

  return (
    <div className="cluster-topology">
      <div className="cluster-map">
        <div className="cluster-map__grid" aria-hidden="true" />

        {failureEvent && (
          <div
            className="failure-event"
            role="status"
            aria-live="polite"
            key={failureEvent.eventId}
          >
            <div className="failure-event__signal">
              <span />
              <span />
              <i />
            </div>

            <div>
              <span className="failure-event__eyebrow">
                Failure detected
              </span>

              <strong>
                {failureEvent.nodeName} became unreachable
              </strong>

              <small>
                {failureEvent.lostInstances > 0
                  ? `${failureEvent.lostInstances} instance${
                      failureEvent.lostInstances === 1 ? '' : 's'
                    } newly marked lost`
                  : 'No newly lost instances in this snapshot'}
              </small>
            </div>
          </div>
        )}

        <div className="cluster-map__legend" aria-label="Node status legend">
          {(
            [
              'ready',
              'joining',
              'draining',
              'unreachable',
            ] as NodeStatus[]
          ).map((status) => (
            <span key={status}>
              <i
                className={`cluster-map__legend-dot cluster-map__legend-dot--${status}`}
                aria-hidden="true"
              />
              {statusLabel(status)}
            </span>
          ))}
        </div>

        {visibleNodes.length > 0 && (
          <svg
            className="cluster-map__edges"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            {visibleNodes.map((node, index) => {
              const position = nodePosition(
                index,
                visibleNodes.length,
              )

              return (
                <line
                  key={node.id}
                  x1="50"
                  y1="50"
                  x2={position.x}
                  y2={position.y}
                  className={`cluster-map__edge cluster-map__edge--${node.status} ${
                    failureEvent?.nodeId === node.id
                      ? 'cluster-map__edge--failure-active'
                      : ''
                  }`}
                />
              )
            })}
          </svg>
        )}

        <div
          className={`cluster-map__hub ${
            failureEvent ? 'cluster-map__hub--failure' : ''
          }`}
        >
          <span
            className="cluster-map__hub-ring"
            aria-hidden="true"
          />

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
          const selected = selectedNodeId === node.id

          return (
            <button
              type="button"
              className={`cluster-node cluster-node--${node.status} ${
                selected ? 'cluster-node--selected' : ''
              } ${
                failureEvent?.nodeId === node.id
                  ? 'cluster-node--failure-active'
                  : ''
              }`}
              key={node.id}
              style={{
                left: `${position.x}%`,
                top: `${position.y}%`,
              }}
              aria-pressed={selected}
              aria-label={`${node.name}, ${statusLabel(node.status)}, ${activeInstances} active instances`}
              onClick={() =>
                setSelectedNodeId((current) =>
                  current === node.id ? null : node.id,
                )
              }
            >
              <div className="cluster-node__header">
                <span
                  className="cluster-node__status-dot"
                  aria-hidden="true"
                />

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
            </button>
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

      {selectedNode && (
        <section
          className={`node-inspector node-inspector--${selectedNode.status}`}
          aria-label={`Worker details for ${selectedNode.name}`}
        >
          <div className="node-inspector__identity">
            <div className="node-inspector__title">
              <span className="node-inspector__status-dot" />

              <div>
                <strong>{selectedNode.name}</strong>
                <span>{selectedNode.id}</span>
              </div>
            </div>

            <div className="node-inspector__tags">
              <span>{statusLabel(selectedNode.status)}</span>
              <span>{selectedNode.region}</span>
              <span>{selectedActiveInstances} active</span>
            </div>
          </div>

          <div className="node-inspector__metric">
            <div>
              <span>CPU</span>
              <strong>
                {selectedNode.allocated.cpu_millis} /{' '}
                {selectedNode.capacity.cpu_millis} m
              </strong>
            </div>

            <div className="node-inspector__track">
              <span
                style={{
                  width: `${percentage(
                    selectedNode.allocated.cpu_millis,
                    selectedNode.capacity.cpu_millis,
                  )}%`,
                }}
              />
            </div>
          </div>

          <div className="node-inspector__metric">
            <div>
              <span>Memory</span>
              <strong>
                {memoryLabel(selectedNode.allocated.memory_bytes)} /{' '}
                {memoryLabel(selectedNode.capacity.memory_bytes)}
              </strong>
            </div>

            <div className="node-inspector__track">
              <span
                style={{
                  width: `${percentage(
                    selectedNode.allocated.memory_bytes,
                    selectedNode.capacity.memory_bytes,
                  )}%`,
                }}
              />
            </div>
          </div>

          <div className="node-inspector__metric">
            <div>
              <span>Instance slots</span>
              <strong>
                {selectedNode.allocated_instances} /{' '}
                {selectedNode.capacity.max_instances}
              </strong>
            </div>

            <div className="node-inspector__track">
              <span
                style={{
                  width: `${allocationPercent(selectedNode)}%`,
                }}
              />
            </div>
          </div>
        </section>
      )}
    </div>
  )
}
