import { useEffect, useState } from 'react'

import type { Deciding, DevflowCalibration } from '../gen/bindings'
import { DevflowAccuracy } from './DevflowAccuracy'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefMore } from './PrefRow'
import { PrefSwitch } from './PrefSwitch'
import { useShell } from './useShell'

/*
 * Decisions: typed questions answered by Jev, a model that writes no text.
 * Off until a key is kept here; without one every gate does what it did
 * before, so nothing here is required.
 */

const PROVIDERS: readonly { id: string; label: string; url: string; model: string }[] = [
  { id: 'openrouter', label: 'OpenRouter', url: 'https://openrouter.ai/api/v1', model: 'typesafe/jev-1.13' },
  { id: 'vercel', label: 'Vercel AI Gateway', url: 'https://ai-gateway.vercel.sh/v1', model: 'typesafe-ai/jev' }, // ui-text: ok a product's name
]

const dollars = (value: number): string => `$${value.toFixed(value < 0.01 && value > 0 ? 4 : 2)}`

export function DecisionsSettings(): React.JSX.Element {
  const { project } = useShell()
  const [now, setNow] = useState<Deciding | null>(null)
  const [key, setKey] = useState('')
  const [said, setSaid] = useState<string | null>(null)
  const [tried, setTried] = useState<string | null>(null)
  const [testing, setTesting] = useState(false)
  const [accuracy, setAccuracy] = useState<DevflowCalibration | null>(null)

  useEffect(() => {
    void ask(() => commands.decisionsRead()).then((answer) => setNow(answer.data))
  }, [])

  const projectId = project?.id
  useEffect(() => {
    if (!projectId) return
    void ask(() => commands.devflowCalibration(projectId)).then((answer) => setAccuracy(answer.data))
  }, [projectId])

  const save = (next: Deciding): void => {
    setNow(next)
    void ask(() => commands.decisionsSet(next.provider, next.model, next.url, next.dailyCapUsd)).then((answer) => {
      setSaid(answer.error)
      if (answer.data) setNow(answer.data)
    })
  }

  const keep = (value: string | null): void => {
    void ask(() => commands.decisionsKeySet(value)).then((answer) => {
      setSaid(answer.error)
      if (answer.error || !now) return
      setKey('')
      setNow({ ...now, keySet: value !== null })
    })
  }

  const send = (projectId: string, yes: boolean): void => {
    void ask(() => commands.decisionsProjectSet(projectId, yes)).then((answer) => {
      setSaid(answer.error)
      if (answer.data) setNow(answer.data)
    })
  }

  /* Through the same path a gate takes, so it also counts toward today. */
  const test = (): void => {
    setTesting(true)
    setTried(null)
    void ask(() => commands.decisionsTest()).then((answer) => {
      setTesting(false)
      const got = answer.data
      if (!got) return setTried(answer.error ?? t('workspace.decisions.noAnswer'))
      if (got.note) return setTried(got.note)
      setTried(t('workspace.decisions.answered', { ms: got.latencyMs ?? 0, cost: dollars(got.costUsd ?? 0), p: got.probability == null ? '' : ` — p ${got.probability.toFixed(2)}` }))
      void ask(() => commands.decisionsRead()).then((again) => again.data && setNow(again.data))
    })
  }

  if (!now) return <></>
  const provider = PROVIDERS.find((one) => one.id === now.provider) ?? PROVIDERS[0]
  const sent = project ? !now.optedOut.includes(project.id) : true
  /* A float crosses the contract as number | null: NaN has no JSON. */
  const cap = now.dailyCapUsd ?? 1
  return (
    <>
      <div className="pref pref--stack">
        <div className="pref__body">
          <span className="pref__t">{t('workspace.decisions.title')}</span>
          <span className="pref__d">{t('workspace.decisions.hint')} {now.keySet ? t('workspace.decisions.on') : t('workspace.decisions.offUntilKey')}</span>
          <PrefMore>
            {tRich('workspace.decisions.more', { decide: <code>devpit_decide</code>, rubric: <code>devpit_rubric_run</code> })} {/* // ui-text: ok commands and file names are not words */}
          </PrefMore>
          <span className="voice__row" role="radiogroup" aria-label={t('workspace.decisions.provider')}>
            {PROVIDERS.map((one) => (
              <button key={one.id} className="btn" data-on={now.provider === one.id ? 'true' : undefined} role="radio" aria-checked={now.provider === one.id} onClick={() => save({ ...now, provider: one.id })}>
                {one.label}
              </button>
            ))}
          </span>
          <label className="voice__row">
            <span className="voice__l">{t('workspace.decisions.address')}</span>
            <input key={`url-${provider.id}`} className="voice__in" placeholder={provider.url} defaultValue={now.url} onBlur={(event) => event.target.value !== now.url && save({ ...now, url: event.target.value })} />
          </label>
          <label className="voice__row">
            <span className="voice__l">{t('workspace.decisions.model')}</span>
            <input key={`model-${provider.id}`} className="voice__in" placeholder={provider.model} defaultValue={now.model} onBlur={(event) => event.target.value !== now.model && save({ ...now, model: event.target.value })} />
          </label>
          <label className="voice__row">
            <span className="voice__l">{t('workspace.decisions.cap')}</span>
            <input
              className="voice__in voice__in--short"
              type="number"
              min="0"
              step="0.25"
              aria-label={t('workspace.decisions.capLabel')}
              defaultValue={cap}
              onBlur={(event) => {
                const next = Number(event.target.value)
                if (Number.isFinite(next) && next !== cap) save({ ...now, dailyCapUsd: next })
              }}
            />
          </label>
          <span className="voice__row">
            <span className="voice__l">{t('workspace.decisions.key')}</span>
            <input className="voice__in" type="password" autoComplete="off" placeholder={now.keySet ? t('workspace.decisions.keyKept') : t('workspace.decisions.keyPaste', { provider: provider.label })} value={key} onChange={(event) => setKey(event.target.value)} />
            <button className="btn" disabled={!key.trim()} onClick={() => keep(key)}>
              {t('workspace.decisions.keep')}
            </button>
            {now.keySet && (
              <button className="btn" onClick={() => keep(null)}>
                {t('workspace.decisions.forget')}
              </button>
            )}
          </span>
          <span className="voice__row">
            <button className="btn" disabled={testing || !now.keySet} onClick={test}>
              {testing ? t('workspace.decisions.asking') : t('workspace.decisions.test')}
            </button>
            {tried && <span className="pref__d">{tried}</span>}
          </span>
          <span className="pref__d">
            {t('workspace.decisions.today', { spent: dollars(now.spentTodayUsd ?? 0), cap: dollars(cap), count: now.decidedToday })}
          </span>
          {said && <span className="pref__d voice__bad">{said}</span>}
        </div>
      </div>
      {project && (
        <PrefSwitch
          on={sent}
          onFlip={() => send(project.id, !sent)}
          title={t('workspace.decisions.sendState', { name: project.name })}
          said={t('workspace.decisions.sendStateSaid')}
        />
      )}
      {accuracy && accuracy.rubrics.length > 0 && <DevflowAccuracy calibration={accuracy} />}
    </>
  )
}
