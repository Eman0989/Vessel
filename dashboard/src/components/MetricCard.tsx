interface MetricCardProps {
  label: string
  value: number | string
  detail: string
  status?: 'default' | 'healthy' | 'warning'
}

export function MetricCard({
  label,
  value,
  detail,
  status = 'default',
}: MetricCardProps) {
  return (
    <article className={`metric-card metric-card--${status}`}>
      <div className="metric-card__header">
        <span className="metric-card__label">{label}</span>
        <span className="metric-card__indicator" aria-hidden="true" />
      </div>

      <strong className="metric-card__value">{value}</strong>
      <span className="metric-card__detail">{detail}</span>
    </article>
  )
}
