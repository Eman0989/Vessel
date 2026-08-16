import type { ClusterMetrics } from '../types/metrics'

const CONTROL_API_BASE =
  import.meta.env.VITE_CONTROL_API_BASE?.replace(/\/$/, '') ?? '/api'

export class ControlApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ControlApiError'
    this.status = status
  }
}

async function requestJson<T>(
  path: string,
  signal?: AbortSignal,
): Promise<T> {
  const response = await fetch(`${CONTROL_API_BASE}${path}`, {
    headers: {
      Accept: 'application/json',
    },
    signal,
  })

  if (!response.ok) {
    throw new ControlApiError(
      response.status,
      `Control plane returned HTTP ${response.status}`,
    )
  }

  return (await response.json()) as T
}

export function fetchClusterMetrics(
  signal?: AbortSignal,
): Promise<ClusterMetrics> {
  return requestJson<ClusterMetrics>('/v1/metrics', signal)
}
