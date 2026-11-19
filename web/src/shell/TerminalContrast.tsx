import { t } from '../i18n'

/*
 * How far the terminal lifts the colours a program paints.
 *
 * Offered as xterm's own ratios: none, WCAG's readable threshold, and its
 * enhanced one. Until someone picks, the terminal follows the ground — none
 * on dark, readable on light — so nothing here is selected.
 */

const CONTRASTS: readonly (readonly [number, string])[] = [
  [1, 'workspace.terminal.contrast.none'],
  [4.5, 'workspace.terminal.contrast.readable'],
  [7, 'workspace.terminal.contrast.strong'],
]

export function TerminalContrast({
  value,
  onPick,
}: {
  value: number | null
  onPick: (value: number) => void
}): React.JSX.Element {
  return (
    <div className="pref">
      <span className="pref__body">
        <span className="pref__t">{t('workspace.terminal.contrast.title')}</span>
        <span className="pref__d">{t('workspace.terminal.contrast.desc')}</span>
      </span>
      <div className="seg seg--tight" role="radiogroup" aria-label={t('workspace.terminal.contrast.title')}>
        {CONTRASTS.map(([step, label]) => (
          <button key={step} className="segb" role="radio" aria-checked={value === step} onClick={() => onPick(step)}>
            {t(label)}
          </button>
        ))}
      </div>
    </div>
  )
}
