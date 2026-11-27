import { useCallback, useEffect, useRef, useState } from 'react'

import type { McpHealth, McpServerHealth } from '../gen/bindings'
import { useAway } from './away'
import { ask, commands } from './live'
import { useCheckEvery } from './checkEvery'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * The MCP servers the default account has, as its CLI finds them when it
 * connects: a count in the strip, and the list — with what each said — a
 * click away. Asked when a project opens and on a beat while the window is in
 * view; the backend keeps the answer for ten minutes, so the beat only starts
 * the CLI when that has run out, and a session's start reads it without
 * waiting.
 */

/** Connected out of all, and whether anything needs looking at. */
export function summed(servers: readonly McpServerHealth[]): { up: number; all: number; bad: boolean } {
  const up = servers.filter((one) => one.state === 'connected').length
  return { up, all: servers.length, bad: up < servers.length }
}

/** Active first — the ones answering — then the ones that are not. */
export function parted(servers: readonly McpServerHealth[]): { title: string; servers: McpServerHealth[] }[] {
  return [
    { title: t('workspace.mcpChip.active'), servers: servers.filter((one) => one.state === 'connected') },
    { title: t('workspace.mcpChip.inactive'), servers: servers.filter((one) => one.state !== 'connected') },
  ]
}

const WORDS: Record<McpServerHealth['state'], string> = {
  connected: 'workspace.mcpChip.state.connected',
  failed: 'workspace.mcpChip.state.failed',
  needs_auth: 'workspace.mcpChip.state.needs_auth',
}

export function McpChip({ projectId }: { projectId: string | null }): React.JSX.Element | null {
  const [health, setHealth] = useState<McpHealth | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [asking, setAsking] = useState(false)
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useAway(box, useCallback(() => setOpen(false), []), open)

  const check = useCallback(
    (again: boolean) => {
      setAsking(true)
      void ask(() => commands.mcpHealth(projectId, again))
        .then((answer) => {
          if (answer.data) setHealth(answer.data)
          setError(answer.error)
        })
        .finally(() => setAsking(false))
    },
    [projectId],
  )

  useEffect(() => {
    check(false)
  }, [check])
  /* As often as Settings → Providers says; a new choice restarts the wait. */
  const everyMs = useCheckEvery()
  useEffect(() => {
    const every = window.setInterval(() => document.visibilityState === 'visible' && check(false), everyMs)
    return () => window.clearInterval(every)
  }, [check, everyMs])

  if (!health && !error) return null
  const { up, all, bad } = summed(health?.servers ?? [])
  return (
    <div className="mcpchip" ref={box}>
      <button className="strip__go" data-state={bad || error ? 'bad' : undefined} aria-expanded={open} onClick={() => setOpen((was) => !was)} title={t('workspace.mcpChip.title')}>
        <span>{t('workspace.mcpChip.mcp')}</span>
        <span className="strip__n">{error && !health ? '?' : `${up}/${all}`}</span>
      </button>
      {open && (
        <div className="mcpchip__pop" role="dialog" aria-label={t('workspace.mcpChip.title')}>
          <div className="mcpchip__h">
            <span>{health ? t('workspace.mcpChip.head', { profile: health.profileId }) : t('workspace.mcpChip.title')}</span>
            <button className="mcpchip__again" disabled={asking} onClick={() => check(true)} aria-label={t('workspace.mcpChip.again')}>
              <Icon size="s" aria-hidden="true"><path d="M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5" /></Icon>
              {asking ? t('workspace.mcpChip.checking') : t('workspace.mcpChip.refresh')}
            </button>
          </div>
          {error && <p className="mcpchip__why">{error}</p>}
          <div className="mcpchip__list">
            {parted(health?.servers ?? []).map(({ title, servers }) =>
              servers.length === 0 ? null : (
                <section key={title}>
                  <h4 className="mcpchip__g">
                    {title} <span>{servers.length}</span>
                  </h4>
                  <ul>
                    {servers.map((one) => (
                      <li key={one.name} data-state={one.state} title={one.detail ?? one.target}>
                        <i className="mcpchip__dot" />
                        <span className="mcpchip__n">{one.name}</span>
                        <span className="mcpchip__s">{t(WORDS[one.state])}</span>
                        {one.detail && <span className="mcpchip__why">{one.detail}</span>}
                      </li>
                    ))}
                  </ul>
                </section>
              ),
            )}
          </div>
        </div>
      )}
    </div>
  )
}
