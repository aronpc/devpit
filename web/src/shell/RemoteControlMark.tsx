import type { RemoteState } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { went } from './problems'
import { Icon } from './Icon'

/** Remote Control's state, in words. */
export function remoteSays(remote: RemoteState): string {
  if (!remote.on) return t('workspace.remoteControl.off')
  return remote.url ? t('workspace.remoteControl.on') : t('workspace.remoteControl.onNoPage')
}

/*
 * Whether a session can be reached by Claude Code's Remote Control: a button
 * to its page while on, a mark otherwise. Turned on and off by /remote-control.
 */
export function RemoteControlMark({ remote, name }: { remote: RemoteState; name: string }): React.JSX.Element {
  const said = remoteSays(remote)
  const icon = (
    <Icon size="s"><rect x="7" y="2" width="10" height="20" rx="2" /><path d="M11 18h2" /></Icon>
  )
  if (remote.on && remote.url) {
    const url = remote.url
    return (
      <button className="sess__icon rctl" data-on="true" title={t('workspace.remoteControl.openTitle', { said, url })} aria-label={t('workspace.remoteControl.openLabel', { name })} onClick={() => void ask(() => commands.urlOpen(url)).then(went)}>
        {icon}
      </button>
    )
  }
  return (
    <span className="rcmark rctl" data-on={remote.on ? 'true' : undefined} role="img" title={said} aria-label={t('workspace.remoteControl.for', { said, name })}>
      {icon}
    </span>
  )
}
