import { useCallback, useEffect, useRef, useState } from 'react'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'

import type { Commit } from '../gen/bindings'
import { onCameBack } from './cameBack'
import { appended, byDay, matches, since, subjectOf } from './commitLog'
import { ask, commands } from './live'
import { CommitMenu, type CommitAt } from './CommitMenu'
import { menuPoint } from './menuRules'
import { Skeleton } from './Skeleton'
import { CHANGED } from './useTree'
import { useShell } from './useShell'
import { t } from '../i18n'

/*
 * What was committed here, most recent first, under the day it was made.
 *
 * Local history only. Nothing fetches, so what is listed is what this
 * checkout has — and the branch chip beside it says how far behind that is.
 *
 * A row is two lines, because one could not hold both halves of the
 * question: the subject, with its kind as a badge rather than a prefix eating
 * the width, and who, when and which commit under it. Older pages load as
 * the list is scrolled to its end; the list comes back fresh when the window
 * does, or when something here changed the repository.
 */

export function History(): React.JSX.Element {
  const { project, show } = useShell()
  const [commits, setCommits] = useState<readonly Commit[]>([])
  const [hasMore, setHasMore] = useState(false)
  // Starts true: an empty list before the first answer arrives is "not
  // fetched yet", not "no commits", and the two must not draw the same.
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [copied, setCopied] = useState<string | null>(null)
  const [menu, setMenu] = useState<CommitAt | null>(null)
  const closeMenu = useCallback(() => setMenu(null), [])
  const end = useRef<HTMLDivElement>(null)

  const load = useCallback(() => {
    if (!project) {
      setCommits([])
      setLoading(false)
      return
    }
    setLoading(true)
    void ask(() => commands.projectHistory(project.id, null, null)).then((answer) => {
      setCommits(answer.data?.commits ?? [])
      setHasMore(answer.data?.hasMore ?? false)
      setError(answer.error)
      setLoading(false)
    })
  }, [project])

  useEffect(() => {
    setCommits([])
    setHasMore(false)
    setError(null)
    load()
  }, [load])

  /* A commit made from a terminal, an agent or another window moves nothing
     here; coming back to the window, or a change announced in it, does. */
  useEffect(() => {
    const back = onCameBack(load)
    window.addEventListener(CHANGED, load)
    return () => {
      back()
      window.removeEventListener(CHANGED, load)
    }
  }, [load])

  const older = useCallback((): void => {
    if (!project || loading) return
    setLoading(true)
    void ask(() => commands.projectHistory(project.id, null, commits.length)).then((answer) => {
      setCommits((was) => appended(was, answer.data?.commits ?? []))
      setHasMore(answer.data?.hasMore ?? false)
      setError(answer.error)
      setLoading(false)
    })
  }, [project, loading, commits.length])

  /* The end of the list coming into view is the ask for more. */
  useEffect(() => {
    const at = end.current
    if (!at || !hasMore || typeof IntersectionObserver === 'undefined') return
    const watch = new IntersectionObserver((seen) => seen.some((one) => one.isIntersecting) && older())
    watch.observe(at)
    return () => watch.disconnect()
  }, [hasMore, older])

  const copy = (sha: string): void => {
    void writeText(sha).then(() => {
      setCopied(sha)
      setTimeout(() => setCopied((was) => (was === sha ? null : was)), 1200)
    })
  }

  const openCommit = (commit: Commit): void => show('diff', { id: `commit:${commit.sha}`, path: commit.sha, title: commit.sha })

  const now = new Date()
  const shown = commits.filter((one) => matches(one, query))

  return (
    <div className="hist">
      {menu && project && <CommitMenu projectId={project.id} at={menu} onOpen={openCommit} onClose={closeMenu} />}
      {commits.length > 0 && (
        <label className="hist__find">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
          <input value={query} placeholder={t('workspace.history.filter')} spellCheck={false} aria-label={t('workspace.history.filter')} onChange={(event) => setQuery(event.target.value)} />
        </label>
      )}
      {error && (
        <div className="exempty">
          <span className="exempty__t">{error}</span>
        </div>
      )}
      {loading && commits.length === 0 && <Skeleton />}
      {!error && !loading && commits.length === 0 && (
        <div className="exempty">
          <span className="exempty__t">{t('workspace.history.none')}</span>
        </div>
      )}
      {commits.length > 0 && shown.length === 0 && <div className="hist__none">{t('workspace.history.noMatch')}</div>}

      {byDay(shown, now).map((group) => (
        <section key={group.day} className="hist__group">
          <div className="hist__day">{group.day}</div>
          {group.commits.map((commit) => {
            const said = subjectOf(commit.subject)
            return (
              <button
                key={commit.sha}
                className="hist__row"
                data-menu={menu?.commit.sha === commit.sha ? 'true' : undefined}
                onClick={() => openCommit(commit)}
                onContextMenu={(event) => {
                  event.preventDefault()
                  setMenu({ commit, ...menuPoint(event) })
                }}
              >
                <span className="hist__top">
                  {said.kind && (
                    <span className="hist__kind" data-kind={said.breaking ? 'breaking' : said.kind}>
                      {said.kind}
                      {said.scope && <span className="hist__scope">{said.scope}</span>}
                    </span>
                  )}
                  <span className="hist__s">{said.text}</span>
                </span>
                <span className="hist__meta">
                  <span className="hist__who">{commit.author}</span>
                  {commit.committedAt !== null && <span>{since(commit.committedAt, now.getTime())}</span>}
                  <span
                    className="hist__sha"
                    role="button"
                    title={copied === commit.sha ? t('workspace.history.copied') : t('workspace.history.copy')}
                    data-copied={copied === commit.sha ? 'true' : undefined}
                    onClick={(event) => {
                      event.stopPropagation()
                      copy(commit.sha)
                    }}
                  >
                    {copied === commit.sha ? 'copied' : commit.sha}
                  </span>
                </span>
              </button>
            )
          })}
        </section>
      ))}

      <div ref={end} className="hist__end">
        {hasMore ? (
          <button className="hist__more" disabled={loading} onClick={older}>
            {loading ? t('workspace.history.loading') : t('workspace.history.older')}
          </button>
        ) : (
          commits.length > 0 && !query && <span>{t('workspace.history.first')}</span>
        )}
      </div>
    </div>
  )
}
