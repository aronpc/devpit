import { useEffect, useState } from 'react'

import type { Installation, Profile, SessionHit } from '../gen/bindings'
import { ask, commands } from './live'
import { profileFor } from './outside'
import { pieces } from './snippet'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * Finding a conversation of this project by something said in it.
 *
 * Every installation's transcripts, devpit's own and the ones started in a
 * terminal. A hit on a conversation devpit already holds opens that tab; any
 * other is taken in the same way a terminal session is.
 */

/* Typing is not a query per keystroke: the first look at a project reads its
   whole history. */
const WAIT_MS = 250

/* What a query finds: in one project, or in every project's conversations
   when none is named. Cleared with the query: a search still on its way when
   the field was emptied used to leave "Reading transcripts…" behind. */
export function useHits(query: string, projectId: string | null, on = true): { hits: readonly SessionHit[]; error: string | null; looking: boolean } {
  const [hits, setHits] = useState<readonly SessionHit[]>([])
  const [error, setError] = useState<string | null>(null)
  const [looking, setLooking] = useState(false)
  useEffect(() => {
    if (!on || !query.trim()) {
      setHits([])
      setLooking(false)
      setError(null)
      return
    }
    let current = true
    const timer = window.setTimeout(() => {
      setLooking(true)
      void ask(() => commands.sessionsSearch(projectId, query)).then((answer) => {
        if (!current) return
        setHits(answer.data ?? [])
        setError(answer.error)
        setLooking(false)
      })
    }, WAIT_MS)
    return () => {
      current = false
      window.clearTimeout(timer)
    }
  }, [on, projectId, query])
  return { hits, error, looking }
}

/* Opens what a hit is in: the conversation devpit holds for it, or the session
   taken in under a profile of its installation. Answers why not, if it could not. */
export async function openHit(hit: SessionHit, open: (projectId: string, conversationId: string) => void): Promise<string | null> {
  if (hit.conversationId) {
    open(hit.projectId, hit.conversationId)
    return null
  }
  const [installed, known] = await Promise.all([ask(() => commands.cliInstallations()), ask(() => commands.agentProfiles())])
  const profile = profileFor(
    { sessionId: hit.sessionId, title: null, installation: hit.installation, lastAt: null },
    (installed.data ?? []) as readonly Installation[],
    (known.data ?? []) as readonly Profile[],
  )
  if (!profile) return t('sessions.resume.noProfile')
  const adopted = await ask(() => commands.chatAdopt(hit.projectId, hit.sessionId, profile.id, null, null))
  if (!adopted.data) return adopted.error
  open(hit.projectId, adopted.data)
  return null
}

export function SessionSearch(): React.JSX.Element | null {
  const { project, show } = useShell()
  const [query, setQuery] = useState('')
  const found = useHits(query, project?.id ?? null, Boolean(project))
  const [failed, setFailed] = useState<string | null>(null)
  const { hits, looking } = found
  const error = failed ?? found.error

  if (!project) return null

  const open = async (hit: SessionHit): Promise<void> => setFailed(await openHit(hit, (_, id) => show('chat', { id })))

  return (
    <div className="ssearch">
      <input
        className="fb__find"
        placeholder={t('sessions.search.placeholder')}
        value={query}
        onChange={(event) => {
          setQuery(event.target.value)
          setFailed(null)
        }}
        aria-label={t('sessions.search.label')}
      />
      {looking && <div className="sessions__none">{t('sessions.search.reading')}</div>}
      {error && <div className="sessions__none">{error}</div>}
      {!looking && query.trim() && hits.length === 0 && !error && <div className="sessions__none">{t('sessions.search.none')}</div>}
      {hits.map((hit, at) => (
        <button className="card" key={`${hit.sessionId}-${at}`} onClick={() => void open(hit)}>
          <span className="card__l2">
            <span className="card__loose">
              {pieces(hit.snippet).map((piece, index) =>
                piece.hit ? <mark key={index}>{piece.text}</mark> : piece.text,
              )}
            </span>
          </span>
        </button>
      ))}
    </div>
  )
}
