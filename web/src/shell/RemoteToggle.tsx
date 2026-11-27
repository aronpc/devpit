import { useEffect, useState } from 'react'

import type { RemoteState } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { went } from './problems'
import { onRemoteConnected } from './window'
import { Icon } from './Icon'

/*
 * Remote Control for a chat — a project's or an orchestrator's: the same
 * conversation, the same process, reachable from the phone or claude.ai while
 * this chat keeps working. What is written there arrives here as it happens.
 * Not in the Supervised mode, which runs a process per turn to ask in.
 */
export function RemoteToggle({
  projectId,
  conversationId,
  sending,
  supervised,
}: {
  projectId: string
  conversationId: string
  /** A turn in flight: a chat waiting to connect does so as its turn starts,
   *  so the page it was given is read again when the turn ends. */
  sending: boolean
  /** Supervised runs a process per turn to ask in, and there is none to reach. */
  supervised: boolean
}): React.JSX.Element {
  const [state, setState] = useState<RemoteState>({ on: false, url: null })
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void ask(() => commands.chatRemoteState(conversationId)).then((found) => found.data && setState(found.data))
  }, [conversationId, sending])

  /* A process started by a turn connects after it: its page comes when it
     comes, and is read then. */
  useEffect(
    () =>
      onRemoteConnected((connected) => {
        if (connected === conversationId) void ask(() => commands.chatRemoteState(conversationId)).then((found) => found.data && setState(found.data))
      }),
    [conversationId],
  )

  const flip = (): void => {
    setBusy(true)
    void ask(() => commands.chatRemote(projectId, conversationId, !state.on))
      .then((said) => said.data && setState(said.data))
      .finally(() => setBusy(false))
  }

  const title = supervised && !state.on
    ? t('workspace.remoteToggle.needsEdits')
    : !state.on
    ? t('workspace.remoteToggle.reach')
    : state.url
      ? t('workspace.remoteToggle.on')
      : t('workspace.remoteToggle.next')
  return (
    <>
      <button className="sq26 rctl" data-on={state.on ? 'true' : undefined} disabled={busy || (supervised && !state.on)} title={title} aria-label={t('workspace.remoteToggle.label')} aria-pressed={state.on} onClick={flip}>
        <Icon size="s"><rect x="7" y="2" width="10" height="20" rx="2" /><path d="M11 18h2" /></Icon>
      </button>
      {state.on && state.url && (
        <button className="sq26" title={t('workspace.remoteToggle.open', { url: state.url })} aria-label={t('workspace.remoteToggle.openLabel')} onClick={() => void ask(() => commands.urlOpen(state.url!)).then(went)}>
          <Icon size="s"><path d="M14 4h6v6M20 4l-9 9M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" /></Icon>
        </button>
      )}
    </>
  )
}
