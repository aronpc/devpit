import type { MissingSignIn } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/*
 * A start held for a sign-in, above the terminal it was meant for. The shell
 * underneath is where the fix is typed; then the start is checked again, or
 * made anyway when the person knows better.
 */
export function LaunchHeld({
  missing,
  onAgain,
  onAnyway,
  onDismiss,
}: {
  missing: readonly MissingSignIn[]
  onAgain: () => void
  onAnyway: () => void
  onDismiss: () => void
}): React.JSX.Element {
  return (
    <div className="theld" role="alert">
      <div className="theld__t">
        <strong>{t('workspace.launch.notStarted')}</strong>
        {missing.map((one) => (
          <span key={one.command}>
            {' '}
            {tRich('workspace.launch.run', { command: <code>{one.command}</code> }, { what: `${one.what[0]?.toUpperCase() ?? ''}${one.what.slice(1)}` })}
          </span>
        ))}
      </div>
      <button className="btn" onClick={onAgain}>
        {t('workspace.launch.again')}
      </button>
      <button className="btn" onClick={onAnyway}>
        {t('workspace.launch.anyway')}
      </button>
      <button className="theld__x" onClick={onDismiss} aria-label={t('workspace.launch.dismiss')}>
        ×
      </button>
    </div>
  )
}
