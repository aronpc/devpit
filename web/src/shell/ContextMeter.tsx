import type { Context } from '../gen/bindings'
import { t } from '../i18n'

/* Where a reading starts to deserve attention, and where it is urgent: the
   CLI compacts on its own near the top, and a person would rather choose. */
const FILLING = 0.7
const FULL = 0.9

export const share = (context: Context): number => (context.window > 0 ? context.used / context.window : 0)

const thousands = (tokens: number): string =>
  tokens >= 1_000_000 ? `${(tokens / 1_000_000).toFixed(1)}M` : `${Math.round(tokens / 1000)}k`

/* How full the model's context was when the last turn ended, as a small ring
   beside the cost. Nothing before the first turn: an empty meter is a number
   nobody has measured. An agent that never reports it reads as unavailable. */
export function ContextMeter({ context, metered = true }: { context: Context | null; metered?: boolean }): React.JSX.Element | null {
  if (!context) {
    return metered ? null : (
      <span className="pcorner__ctx" title={t('workspace.meter.unreported')}>
        {t('workspace.meter.na')}
      </span>
    )
  }
  const full = Math.min(1, share(context))
  const level = full >= FULL ? 'full' : full >= FILLING ? 'filling' : undefined
  const round = 2 * Math.PI * 5
  return (
    <span
      className="pcorner__ctx"
      data-level={level}
      title={t(level ? 'workspace.meter.usedFull' : 'workspace.meter.used', { used: thousands(context.used), window: thousands(context.window) })}
    >
      <svg width="13" height="13" viewBox="0 0 14 14" aria-hidden="true">
        <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="2" />
        <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="2" strokeDasharray={`${full * round} ${round}`} transform="rotate(-90 7 7)" strokeLinecap="round" />
      </svg>
      {Math.round(full * 100)}%
    </span>
  )
}
