import { useEffect, useRef, useState } from 'react'

import type { FolderGlance } from '../gen/bindings'
import { shortPath } from './blockText'
import { Folder, Pencil } from './GitIcons'
import { ask, commands } from './live'
import { PluginChip } from './PluginChip'
import { abandoned, committed } from './typing'
import { useShellPick } from './shellStore'
import { useKeys, useShortcut } from './useShortcut'
import { t } from '../i18n'

/*
 * The strip under a terminal while an agent runs in it.
 *
 * Where you are — the folder and its branch — and two things an agent's TUI
 * cannot offer from inside a terminal: the project's files a click away, and
 * a real editor to write the next message in. What is written there goes to
 * the agent as one paste and is sent, the way it would have been typed.
 */

export function AgentBar({
  projectId,
  paneId,
  agent,
  agentId,
  cwd,
  home,
  onSent,
}: {
  projectId: string
  paneId: string
  agent: string
  /** `claude`, `codex` — which agent, where `agent` is what it is called. */
  agentId: string | null
  cwd: string | null
  home: string | null
  /** The keyboard goes back to the terminal. */
  onSent: () => void
}): React.JSX.Element {
  const toggleFiles = useShellPick((shell) => shell.toggleFiles)
  const [composing, setComposing] = useState(false)
  const [text, setText] = useState('')
  const [glance, setGlance] = useState<FolderGlance | null>(null)
  const field = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (!cwd) return setGlance(null)
    let live = true
    void ask(() => commands.folderGlance(cwd)).then((answer) => live && setGlance(answer.data ?? null))
    return () => {
      live = false
    }
  }, [cwd])

  /* Opens the editor from anywhere in this pane, as Ctrl+G does in Warp. Taken
     before xterm sees it: it used to reach the agent as well, and Claude Code's
     own Ctrl+G opened a second editor inside the terminal. */
  useShortcut('compose', () => {
    if (!document.activeElement?.closest(`[data-pane-id="${paneId}"]`)) return false
    setComposing(true)
  })
  const keys = useKeys()

  useEffect(() => {
    if (composing) field.current?.focus()
  }, [composing])

  const send = (): void => {
    if (!text.trim()) return
    void ask(() => commands.paneCompose(projectId, paneId, text)).then(() => {
      setText('')
      setComposing(false)
      onSent()
    })
  }

  return (
    <div className="abar">
      {composing && (
        <div className="abar__compose">
          <textarea
            ref={field}
            className="abar__field"
            value={text}
            rows={Math.min(10, Math.max(3, text.split('\n').length))}
            placeholder={t('workspace.agentBar.placeholder', { agent })}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (abandoned(event)) {
                setComposing(false)
                onSent()
              } else if (committed(event) && !event.shiftKey) {
                event.preventDefault()
                send()
              }
            }}
          />
        </div>
      )}
      <div className="abar__row">
        <span className="abar__agent">{agent}</span>
        {cwd && (
          <span className="cin__chip" title={cwd}>
            <Folder size={12} />
            {shortPath(cwd, home)}
          </span>
        )}
        {glance && (
          <span className="cin__chip cin__chip--git">
            {glance.branch}
            {glance.files > 0 && (
              <span className="cin__diff">
                {' '}
                {glance.files} · <span className="cin__add">+{glance.added}</span> <span className="cin__del">-{glance.removed}</span>
              </span>
            )}
          </span>
        )}
        <span className="abar__gap" />
        {agentId === 'claude' && <PluginChip />}
        <button className="abar__b" onClick={toggleFiles} title={t('workspace.agentBar.filesTitle')}>
          <Folder size={14} /> {t('workspace.agentBar.files')}
        </button>
        <button className="abar__b" data-on={composing ? 'true' : undefined} onClick={() => setComposing(!composing)} title={t('workspace.agentBar.composeTitle', { keys: keys('compose') })}>
          <Pencil size={14} /> {t('workspace.agentBar.compose')}
        </button>
      </div>
    </div>
  )
}
