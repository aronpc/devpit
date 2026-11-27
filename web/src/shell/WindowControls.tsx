import { t } from '../i18n'
import { close, minimize, toggleMaximize } from './window'
import { Icon } from './Icon'

/*
 * A frameless window still has to be minimised, maximised and closed; these
 * are ours to draw because the system's are not there. The middle one names
 * what it will do next, as the system's does.
 */
export function WindowControls({ maximized }: { maximized: boolean }): React.JSX.Element {
  const middle = maximized ? t('workspace.window.restore') : t('workspace.window.maximize')
  return (
    <span className="wctl">
      <button className="wbtn tip" data-tip={t('workspace.window.minimize')} aria-label={t('workspace.window.minimize')} onClick={minimize}>
        <Icon size="s">
          <path d="M5 12h14" />
        </Icon>
      </button>
      <button className="wbtn tip" data-tip={middle} aria-label={middle} onClick={toggleMaximize}>
        {maximized ? (
          <Icon size="s">
            <rect x="4" y="8" width="12" height="12" rx="1.5" />
            <path d="M8 8V5.5A1.5 1.5 0 0 1 9.5 4h9A1.5 1.5 0 0 1 20 5.5v9a1.5 1.5 0 0 1-1.5 1.5H16" />
          </Icon>
        ) : (
          <Icon size="s">
            <rect x="5" y="5" width="14" height="14" rx="1.5" />
          </Icon>
        )}
      </button>
      <button className="wbtn wbtn--x tip" data-tip={t('workspace.window.close')} aria-label={t('workspace.window.close')} onClick={close}>
        <Icon size="s">
          <path d="M18 6 6 18M6 6l12 12" />
        </Icon>
      </button>
    </span>
  )
}
