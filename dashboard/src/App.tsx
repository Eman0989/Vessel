import { useCallback, useEffect, useState } from 'react'

import { fetchClusterMetrics } from './api/control'
import './App.css'
import { MetricCard } from './components/MetricCard'
import type { ClusterMetrics } from './types/metrics'

const REFRESH_INTERVAL_MS = 5_000

function percentage(allocated: number, capacity: number): string {
  if (capacity === 0) {
    return '0%'
  }

  return `${Math.round((allocated / capacity) * 100)}%`
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

function App() {
  const [metrics, setMetrics] = useState<ClusterMetrics | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)

  const loadMetrics = useCallback(async (signal?: AbortSignal) => {
    setIsRefreshing(true)

    try {
      const snapshot = await fetchClusterMetrics(signal)

      setMetrics(snapshot)
      setError(null)
      setLastUpdated(new Date())
    } catch (caught) {
      if (caught instanceof DOMException && caught.name === 'AbortError') {
        return
      }

      setError(
        caught instanceof Error
          ? caught.message
          : 'Unable to reach the VESSEL control plane.',
      )
    } finally {
      if (!signal?.aborted) {
        setIsRefreshing(false)
      }
    }
  }, [])

  useEffect(() => {
    const controller = new AbortController()

    void loadMetrics(controller.signal)

    const interval = window.setInterval(() => {
      void loadMetrics(controller.signal)
    }, REFRESH_INTERVAL_MS)

    return () => {
      controller.abort()
      window.clearInterval(interval)
    }
  }, [loadMetrics])

  const nodes = metrics?.nodes
  const deployments = metrics?.deployments
  const instances = metrics?.instances
  const resources = metrics?.resources

  const clusterHealthy =
    metrics !== null &&
    (nodes?.unreachable ?? 0) === 0 &&
    (deployments?.failed ?? 0) === 0

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand__mark" aria-hidden="true">
            V
          </span>

          <div>
            <strong>VESSEL</strong>
            <span>Execution Fabric</span>
          </div>
        </div>

        <nav className="navigation" aria-label="Primary navigation">
          <a className="navigation__item navigation__item--active" href="#overview">
            <span className="navigation__glyph" aria-hidden="true">
              ◫
            </span>
            Overview
          </a>

          <span className="navigation__section">Cluster</span>

          <a className="navigation__item" href="#nodes">
            <span className="navigation__glyph" aria-hidden="true">
              ◇
            </span>
            Nodes
          </a>

          <a className="navigation__item" href="#deployments">
            <span className="navigation__glyph" aria-hidden="true">
              ↗
            </span>
            Deployments
          </a>

          <a className="navigation__item" href="#instances">
            <span className="navigation__glyph" aria-hidden="true">
              ⬡
            </span>
            Instances
          </a>

          <span className="navigation__section">Operations</span>

          <a className="navigation__item" href="#telemetry">
            <span className="navigation__glyph" aria-hidden="true">
              ∿
            </span>
            Telemetry
          </a>
        </nav>

        <div className="sidebar__footer">
          <span className="sidebar__status-dot" />
          Control plane
        </div>
      </aside>

      <main className="dashboard">
        <header className="topbar">
          <div>
            <p className="eyebrow">Cluster / Overview</p>
            <h1>Execution fabric</h1>
          </div>

          <div className="topbar__actions">
            <div className="connection-state">
              <span
                className={`connection-state__dot ${
                  error ? 'connection-state__dot--error' : ''
                }`}
              />

              <span>{error ? 'Disconnected' : 'Live'}</span>
            </div>

            <button
              type="button"
              className="refresh-button"
              disabled={isRefreshing}
              onClick={() => void loadMetrics()}
            >
              {isRefreshing ? 'Refreshing…' : 'Refresh'}
            </button>
          </div>
        </header>

        <section className="hero" id="overview">
          <div>
            <p className="eyebrow">System status</p>
            <h2>
              {error
                ? 'Control plane unavailable'
                : metrics
                  ? clusterHealthy
                    ? 'Cluster operating normally'
                    : 'Cluster requires attention'
                  : 'Connecting to cluster'}
            </h2>

            <p className="hero__copy">
              Real-time operational state from the VESSEL control plane.
            </p>
          </div>

          <div className="hero__meta">
            <span>Refresh interval</span>
            <strong>5 seconds</strong>

            <span>Last snapshot</span>
            <strong>
              {lastUpdated
                ? lastUpdated.toLocaleTimeString()
                : 'Waiting for data'}
            </strong>
          </div>
        </section>

        {error && (
          <section className="error-panel" role="alert">
            <div>
              <strong>Unable to load cluster metrics</strong>
              <span>{error}</span>
            </div>

            <button type="button" onClick={() => void loadMetrics()}>
              Retry
            </button>
          </section>
        )}

        <section className="metric-grid" aria-label="Cluster summary">
          <MetricCard
            label="Worker nodes"
            value={nodes?.total ?? '—'}
            detail={`${nodes?.ready ?? 0} ready · ${nodes?.unreachable ?? 0} unreachable`}
            status={(nodes?.unreachable ?? 0) > 0 ? 'warning' : 'healthy'}
          />

          <MetricCard
            label="Deployments"
            value={deployments?.total ?? '—'}
            detail={`${deployments?.healthy ?? 0} healthy · ${deployments?.desired_replicas ?? 0} desired replicas`}
            status={(deployments?.failed ?? 0) > 0 ? 'warning' : 'healthy'}
          />

          <MetricCard
            label="Active instances"
            value={instances?.active ?? '—'}
            detail={`${instances?.running ?? 0} running · ${instances?.restart_count ?? 0} restarts`}
          />

          <MetricCard
            label="CPU allocation"
            value={
              resources
                ? percentage(
                    resources.allocated_cpu_millis,
                    resources.capacity_cpu_millis,
                  )
                : '—'
            }
            detail={
              resources
                ? `${resources.allocated_cpu_millis} / ${resources.capacity_cpu_millis} millicores`
                : 'Waiting for resource metrics'
            }
          />
        </section>

        <section className="workspace-grid">
          <article className="panel panel--primary" id="nodes">
            <div className="panel__header">
              <div>
                <p className="eyebrow">Topology</p>
                <h3>Cluster map</h3>
              </div>

              <span className="panel__badge">Step 24</span>
            </div>

            <div className="foundation-placeholder">
              <div className="foundation-placeholder__core">
                <span>VESSEL</span>
              </div>

              <p>
                Node topology visualization will render here in the next
                milestone.
              </p>
            </div>
          </article>

          <article className="panel" id="telemetry">
            <div className="panel__header">
              <div>
                <p className="eyebrow">Capacity</p>
                <h3>Resource allocation</h3>
              </div>
            </div>

            <div className="resource-list">
              <div className="resource-row">
                <div>
                  <span>CPU</span>
                  <strong>
                    {resources
                      ? `${resources.allocated_cpu_millis} m`
                      : '—'}
                  </strong>
                </div>

                <div className="progress-track">
                  <span
                    style={{
                      width: resources
                        ? percentage(
                            resources.allocated_cpu_millis,
                            resources.capacity_cpu_millis,
                          )
                        : '0%',
                    }}
                  />
                </div>
              </div>

              <div className="resource-row">
                <div>
                  <span>Memory</span>
                  <strong>
                    {resources
                      ? memoryLabel(resources.allocated_memory_bytes)
                      : '—'}
                  </strong>
                </div>

                <div className="progress-track">
                  <span
                    style={{
                      width: resources
                        ? percentage(
                            resources.allocated_memory_bytes,
                            resources.capacity_memory_bytes,
                          )
                        : '0%',
                    }}
                  />
                </div>
              </div>

              <div className="resource-row">
                <div>
                  <span>Instance slots</span>
                  <strong>
                    {resources
                      ? `${resources.allocated_instances} / ${resources.max_instances}`
                      : '—'}
                  </strong>
                </div>

                <div className="progress-track">
                  <span
                    style={{
                      width: resources
                        ? percentage(
                            resources.allocated_instances,
                            resources.max_instances,
                          )
                        : '0%',
                    }}
                  />
                </div>
              </div>
            </div>
          </article>
        </section>

        <footer className="dashboard-footer">
          <span>VESSEL control plane</span>
          <span>Dashboard foundation · v0.1</span>
        </footer>
      </main>
    </div>
  )
}

export default App
