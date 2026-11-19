import { useCallback, useEffect, useRef, useState } from 'react'

import type { PlanLimits, Spend, SpendHistory, SpendRow } from '../gen/bindings'
import { t } from '../i18n'
import { formatNumber } from '../i18n/format'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { ago, allTokens, dollars, heights, n, resetIn, tokens, valueOf, type Metric } from './usageFormat'
import { useShell } from './useShell'

/*
 * What the agents on this machine spent, and how much of the plan is left.
 *
 * Read from the agents' own transcripts, deduplicated and priced here, so
 * nothing spends to be counted. The plan's windows are the one network read,
 * no more than every five minutes. A number the transcripts do not hold is not
 * drawn: an installation run against another provider says its dollars are
 * list price, and a model with no price says so in the footer.
 */

const RANGES = [7, 30, 90] as const

function Table({ rows, what }: { rows: readonly SpendRow[]; what: string }): React.JSX.Element | null {
  useLocale()
  if (rows.length === 0) return null
  return (
    <section className="use__block">
      <span className="use__label">{t('usage.by', { what })}</span>
      <div className="mrow mrow--h">
        <span className="mrow__n">{what}</span>
        <span>{t('usage.colTokens')}</span>
        <span>{t('usage.colSessions')}</span>
        <span>{t('usage.colCost')}</span>
      </div>
      {rows.slice(0, 8).map((row) => (
        <div className="mrow" key={row.name}>
          <span className="mrow__n" title={row.name}>
            <span>{row.name}</span>
          </span>
          <span>{tokens(allTokens(row.tokens))}</span>
          <span>{row.sessions}</span>
          <span>{dollars(row.costUsd)}</span>
        </div>
      ))}
    </section>
  )
}

export function Usage(): React.JSX.Element {
  useLocale()
  const { project, show, openCard } = useShell()
  const [days, setDays] = useState<number>(30)
  const [installation, setInstallation] = useState<string | null>(null)
  const [onlyProject, setOnlyProject] = useState(false)
  const [metric, setMetric] = useState<Metric>('cost')
  const [history, setHistory] = useState<SpendHistory | null>(null)
  const [limits, setLimits] = useState<PlanLimits | null>(null)
  const [cards, setCards] = useState<Spend | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [reading, setReading] = useState(false)
  const [now, setNow] = useState(() => Date.now())
  /* A long range reads slower than a short one; only the latest ask is drawn. */
  const asking = useRef(0)

  const read = useCallback(() => {
    const mine = ++asking.current
    setReading(true)
    const scope = onlyProject && project ? project.id : null
    void ask(() => commands.spendHistory(scope, installation, days)).then((answer) => {
      if (mine !== asking.current) return
      setReading(false)
      setError(answer.error)
      if (answer.data) setHistory(answer.data)
    })
    void ask(() => commands.planLimits(installation)).then((answer) => {
      if (mine === asking.current) setLimits(answer.data)
    })
    if (project) void ask(() => commands.usageRead(project.id)).then((answer) => setCards(answer.data))
  }, [days, installation, onlyProject, project])

  useEffect(read, [read])

  /* The reset countdown moves while the screen is open. */
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000)
    return () => window.clearInterval(timer)
  }, [])

  const tall = history ? heights(history.daily, metric) : []

  return (
    <>
      <h1 className="prefs__h">{t('usage.title')}</h1>

      <div className="use__top">
        <div className="seg2" role="tablist" aria-label={t('usage.range')}>
          {RANGES.map((range) => (
            <button key={range} role="tab" aria-selected={days === range} onClick={() => setDays(range)}>
              {t('usage.rangeDays', { days: range })}
            </button>
          ))}
        </div>
        <select
          className="use__pick"
          aria-label={t('usage.installation')}
          value={installation ?? ''}
          onChange={(event) => setInstallation(event.target.value || null)}
        >
          <option value="">{t('usage.allInstallations')}</option>
          {history?.installations.map((one) => (
            <option key={one.directory} value={one.directory}>
              {one.label}
              {one.billed ? '' : t('usage.notBilled')}
            </option>
          ))}
        </select>
        {project && (
          <div className="seg2" role="tablist" aria-label={t('usage.scope')}>
            <button role="tab" aria-selected={!onlyProject} onClick={() => setOnlyProject(false)}>
              {t('usage.allProjects')}
            </button>
            <button role="tab" aria-selected={onlyProject} onClick={() => setOnlyProject(true)}>
              {project.name}
            </button>
          </div>
        )}
        <button className="btn" disabled={reading} onClick={read}>
          {reading ? t('usage.reading') : t('usage.rescan')}
        </button>
      </div>

      {error && <p className="acc__note">{error}</p>}

      {limits && (
        <section className="use__block" aria-label={t('usage.planLimits')}>
          <span className="use__label">{t('usage.plan')}{limits.plan ? ` · ${limits.plan}` : ''}</span>
          {limits.problem && <p className="use__note">{limits.problem}</p>}
          <div className="use__limits">
          {limits.windows.map((window) => (
            <div className="bar" key={window.label}>
              <div className="bar__top">
                <span className="bar__n">{window.label}</span>
                <span className="bar__v">{Math.round(n(window.percent))}%</span>
              </div>
              <div className="bar__track">
                <div className="bar__fill" data-high={n(window.percent) >= 80} style={{ width: `${n(window.percent)}%` }} />
              </div>
              {window.resetsAt !== null && <div className="bar__sub">{resetIn(window.resetsAt, now)}</div>}
            </div>
          ))}
          </div>
        </section>
      )}

      {history && (
        <>
          <section className="use__block">
          <span className="use__label">{t('usage.lastDays', { days: history.days })}</span>
          <div className="tiles">
            <div className="tile2">
              <div className="tile2__l">{t('usage.estCost')}</div>
              <div className="tile2__v">{dollars(history.costUsd)}</div>
              <div className="tile2__s">{history.estimated ? t('usage.partlyList') : t('usage.atList')}</div>
            </div>
            <div className="tile2">
              <div className="tile2__l">{t('usage.tokens')}</div>
              <div className="tile2__v">
                {tokens(allTokens(history.tokens))}
              </div>
              <div className="tile2__s">{t('usage.written', { tokens: tokens(history.tokens.output) })}</div>
            </div>
            <div className="tile2">
              <div className="tile2__l">{t('usage.sessions')}</div>
              <div className="tile2__v">{history.sessions}</div>
              <div className="tile2__s">{t('usage.turnsCount', { count: history.turns })}</div>
            </div>
            <div className="tile2">
              <div className="tile2__l">{t('usage.activeDays')}</div>
              <div className="tile2__v">{history.activeDays}</div>
              <div className="tile2__s">{t('usage.ofDays', { days: history.days })}</div>
            </div>
            <div className="tile2">
              <div className="tile2__l">{t('usage.cacheReuse')}</div>
              <div className="tile2__v">{Math.round(n(history.cacheReuse) * 100)}%</div>
              <div className="tile2__s">{t('usage.fromCache')}</div>
            </div>
          </div>
          </section>

          <section className="use__block">
            <div className="use__hrow">
              <span className="use__label">{t('usage.byDay')}</span>
              <div className="seg2" role="tablist" aria-label={t('usage.chartShows')}>
                <button role="tab" aria-selected={metric === 'cost'} onClick={() => setMetric('cost')}>
                  {t('usage.cost')}
                </button>
                <button role="tab" aria-selected={metric === 'tokens'} onClick={() => setMetric('tokens')}>
                  {t('usage.tokens')}
                </button>
              </div>
            </div>
            <svg className="use__chart" viewBox={`0 0 ${history.daily.length * 10} 100`} preserveAspectRatio="none" role="img" aria-label={t(metric === 'cost' ? 'usage.chartCost' : 'usage.chartTokens')}>
              {history.daily.map((one, at) => {
                const height = (tall[at] ?? 0) * 96
                const value = valueOf(one, metric)
                return (
                  <rect key={one.day} x={at * 10 + 1} y={100 - height} width={8} height={height} rx={1} fill="var(--accent)">
                    <title>
                      {one.day}: {metric === 'cost' ? dollars(value) : tokens(value)}
                      {one.byModel.length > 0 ? ` · ${one.byModel.slice(0, 3).map((share) => share.name).join(', ')}` : ''}
                    </title>
                  </rect>
                )
              })}
            </svg>
            <div className="use__axis">
              <span>{history.daily[0]?.day}</span>
              <span>{history.daily.at(-1)?.day}</span>
            </div>
          </section>

          <div className="use__cols">
            <Table rows={history.models} what={t('usage.model')} />
            <Table rows={history.projects} what={t('usage.project')} />
          </div>

          {history.recent.length > 0 && (
            <section className="use__block">
              <span className="use__label">{t('usage.recent')}</span>
              {history.recent.map((session) => (
                <div className="srow" key={session.sessionId}>
                  <span className="srow__when">{ago(session.lastActive, now)}</span>
                  <span className="srow__what" title={session.project}>
                    {session.project}
                    <span className="srow__model">{session.model}</span>
                  </span>
                  {session.card ? (
                    <button
                      className="srow__card"
                      onClick={() => {
                        show('board')
                        openCard(session.card!.id)
                      }}
                    >
                      {session.card.title}
                    </button>
                  ) : (
                    <span />
                  )}
                  <span className="srow__n">{t('usage.turnsCount', { count: session.turns })}</span>
                  <span className="srow__n">{dollars(session.costUsd)}</span>
                </div>
              ))}
            </section>
          )}

          {cards && cards.cards.length > 0 && (
            <section className="use__block">
              <span className="use__label">{t('usage.cards')}</span>
              {cards.cards.map(([title, usd]) => (
                <div className="mrow" key={title}>
                  <span className="mrow__n">
                    <span>{title}</span>
                  </span>
                  <span />
                  <span />
                  <span>{dollars(usd)}</span>
                </div>
              ))}
            </section>
          )}

          <p className="use__scan">
            {t('usage.scan', { files: formatNumber(history.files), records: formatNumber(history.records), ms: formatNumber(Math.round(n(history.scanMs))) })}
            {n(history.unpricedTokens) > 0 ? t('usage.unpriced', { tokens: tokens(history.unpricedTokens) }) : ''}
          </p>
        </>
      )}
    </>
  )
}
