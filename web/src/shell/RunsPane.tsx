import { useCallback, useEffect, useRef, useState } from 'react'

import type { ProjectRun, RunCursor } from '../gen/bindings'
import type { Lane } from './board'
import { money } from './chat'
import { Checked } from './CheckedView'
import { ask, commands } from './live'
import { asQuery, emptyBecause, STATES, type RunFilters } from './runs'
import { abandoned } from './typing'
import { RunIdentity } from './PausedStep'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import { Icon } from './Icon'

/*
 * Every run on the board, newest first.
 *
 * A card shows its own runs; this answers what a card cannot — what ran across
 * the board, which lane keeps failing, what was lost when the app closed.
 * Paged from the last row shown, so a run starting while you read does not
 * shift the next page.
 */

export function RunsPane({
  projectId,
  lanes,
  onClose,
  onOpenCard,
}: {
  projectId: string
  lanes: readonly Lane[]
  onClose: () => void
  onOpenCard: (cardId: string) => void
}): React.JSX.Element {
  const [filters, setFilters] = useState<RunFilters>({ stepId: null, state: null, from: '', to: '' })
  /* One at a time: what a run proves is read from the repository, and a list
     that answered it for every row would run two processes per row. */
  const [opened, setOpened] = useState<string | null>(null)
  const [runs, setRuns] = useState<readonly ProjectRun[]>([])
  const [next, setNext] = useState<RunCursor | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  /* Only the latest request may land: a page for the filters before this
     change would otherwise be appended to the list for the new ones. */
  const latest = useRef(0)

  const load = useCallback(
    (after: RunCursor | null) => {
      const mine = ++latest.current
      setLoading(true)
      void ask(() => commands.runsList(asQuery(projectId, filters, after))).then((page) => {
        if (mine !== latest.current) return
        setLoading(false)
        setError(page.error)
        const got = page.data
        if (!got) return
        setRuns((was) => (after ? [...was, ...got.runs] : got.runs))
        setNext(got.next)
      })
    },
    [projectId, filters],
  )

  useEffect(() => load(null), [load])

  useEffect(() => {
    const key = (event: KeyboardEvent): void => {
      if (abandoned(event)) onClose()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [onClose])

  const stepped = lanes.filter((lane) => lane.column.step)
  /* Three empties, three sentences: nothing configured, nothing run yet, and
     nothing behind the filter somebody set. */
  const empty = emptyBecause(stepped.length, filters, runs.length === 0 && !loading && !error)

  return (
    <div className="cardp" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="cardp__box runsp" role="dialog" aria-modal="true" aria-label={t('sessions.runs.title')}>
        <header className="runsp__top">
          <h2 className="runsp__t">{t('sessions.runs.title')}</h2>
          <button className="sq26" onClick={onClose} aria-label={t('sessions.runs.close')} title={t('sessions.runs.close')}>
            <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
          </button>
        </header>

        <div className="runsp__filters">
          <label>
            {t('sessions.runs.lane')}
            <select value={filters.stepId ?? ''} onChange={(event) => setFilters({ ...filters, stepId: event.target.value || null })}>
              <option value="">{t('sessions.runs.everyLane')}</option>
              {stepped.map((lane) => (
                <option key={lane.column.id} value={lane.column.step?.id}>
                  {lane.column.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sessions.runs.state')}
            <select
              value={filters.state ?? ''}
              onChange={(event) => setFilters({ ...filters, state: STATES.find((one) => one === event.target.value) ?? null })}
            >
              <option value="">{t('sessions.runs.anyState')}</option>
              {STATES.map((state) => (
                <option key={state} value={state}>
                  {state}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sessions.runs.from')}
            <input type="date" value={filters.from} onChange={(event) => setFilters({ ...filters, from: event.target.value })} />
          </label>
          <label>
            {t('sessions.runs.to')}
            <input type="date" value={filters.to} onChange={(event) => setFilters({ ...filters, to: event.target.value })} />
          </label>
        </div>

        {error && <p className="wtb__no">{error}</p>}
        {empty === 'noChecks' && (
          <p className="runsp__none">
            {t('sessions.runs.noChecks')}
          </p>
        )}
        {empty === 'nothingRan' && (
          <p className="runsp__none">{t('sessions.runs.nothingRan')}</p>
        )}
        {empty === 'noneMatch' && <p className="runsp__none">{t('sessions.runs.noneMatch')}</p>}
        {runs.map(({ run, cardId, cardTitle }) => (
          <div className="crun" key={run.id} data-state={run.state}>
            <span className="crun__b">
              <button className="runsp__card" onClick={() => onOpenCard(cardId)}>
                {cardTitle}
              </button>
              <span className="crun__t">
                {run.stepName}
                {run.startedAt !== null && ` · ${formatDate(run.startedAt * 1000, { dateStyle: 'medium', timeStyle: 'short' })}`}
              </span>
              <RunIdentity run={run} />
              {run.output && <span className="crun__o">{run.output}</span>}
              {opened === run.id && (
                <Checked runId={run.id} cardId={cardId} stepId={run.stepId} />
              )}
            </span>
            <button
              className="crun__s"
              aria-expanded={opened === run.id}
              onClick={() => setOpened(opened === run.id ? null : run.id)}
            >
              {run.state}
            </button>
            {money(run.costUsd ?? 0) && <span className="crun__c">{money(run.costUsd ?? 0)}</span>}
          </div>
        ))}
        {next && (
          <button className="btn runsp__more" disabled={loading} onClick={() => load(next)}>
            {t('sessions.runs.more')}
          </button>
        )}
      </div>
    </div>
  )
}
