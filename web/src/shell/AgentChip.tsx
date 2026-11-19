import { useCallback, useEffect, useRef, useState } from 'react'

import type { AgentHealth } from '../gen/bindings'
import { useAway } from './away'
import { ask, commands } from './live'
import { onAgentHealth } from './window'
import { t } from '../i18n'

/*
 * devpit's own MCP — the door its tools post to — beside the account's
 * servers: whether it answers, and a restart that leaves the window, the
 * chats and the terminals where they are.
 */

/** What the chip says: the answer's time, or that there was none. */
export function agentWord(health: AgentHealth): string {
  if (!health.answering) return t('workspace.agentChip.noAnswer')
  return health.latencyMs === null ? t('workspace.agentChip.ok') : `${Math.round(health.latencyMs)} ms`
}

export function AgentChip(): React.JSX.Element | null {
  const [health, setHealth] = useState<AgentHealth | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, useCallback(() => setOpen(false), []), open)

  const answered = (answer: { data: AgentHealth | null; error: string | null }): void => {
    if (answer.data) setHealth(answer.data)
    setError(answer.error)
  }

  useEffect(() => {
    void ask(() => commands.agentHealth()).then(answered)
    return onAgentHealth(setHealth)
  }, [])

  const restart = (): void => {
    setBusy(true)
    void ask(() => commands.agentRestart())
      .then(answered)
      .finally(() => setBusy(false))
  }

  if (!health && !error) return null
  const bad = Boolean(error) || (health !== null && !health.answering)
  return (
    <div className="mcpchip" ref={box}>
      <button className="strip__go" data-state={bad ? 'bad' : undefined} aria-expanded={open} onClick={() => setOpen((was) => !was)} title={t('workspace.agentChip.title')}>
        <span>devpit</span> {/* // ui-text: ok the product's name */}
        <span className="strip__n">{health ? agentWord(health) : '?'}</span>
      </button>
      {open && (
        <div className="mcpchip__pop" role="dialog" aria-label={t('workspace.agentChip.label')}>
          <div className="mcpchip__h">
            <span>{t('workspace.agentChip.label')}</span>
            <button className="mcpchip__again" disabled={busy} onClick={restart} title={t('workspace.agentChip.restartTitle')}>
              {busy ? t('workspace.agentChip.restarting') : t('workspace.agentChip.restart')}
            </button>
          </div>
          {error && <p className="mcpchip__why">{error}</p>}
          {health && (
            <p className="agentchip__said">
              {health.answering ? t('workspace.agentChip.answering', { word: agentWord(health) }) : t('workspace.agentChip.notAnswering', { count: health.failures })}
              {health.restarts > 0 && ` ${t('workspace.agentChip.restarted', { count: health.restarts })}`}
              {health.detail && <span className="mcpchip__why">{health.detail}</span>}
            </p>
          )}
        </div>
      )}
    </div>
  )
}
