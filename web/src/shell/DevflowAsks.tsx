import { useCallback, useEffect, useState } from 'react'

import type { DevflowAsk, DevflowAsks as Asks } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * Devflow changes only the person makes: switching it on in a project, or
 * moving its Decisions to enforce. An agent asks, or a devflow.yaml says so
 * on its own; the card waits under the top bar, the way a reminder does, and
 * nothing changes until the click.
 */

export function devflowQuestion(one: DevflowAsk): { question: string; why: string; yes: string } {
  const args = { project: one.project, detail: one.detail ?? '' }
  return {
    question: t(`devflow.asks.${one.change}.question`, args),
    why: t(`devflow.asks.${one.change}.why`, args),
    yes: t(`devflow.asks.${one.change}.yes`),
  }
}

export function DevflowAsks(): React.JSX.Element | null {
  const [asks, setAsks] = useState<readonly DevflowAsk[]>([])
  const [problem, setProblem] = useState<string | null>(null)

  const told = useCallback((answer: { data: Asks | null; error: string | null }): void => {
    if (answer.data) setAsks(answer.data.asks)
    setProblem(answer.error)
  }, [])
  const reload = useCallback(() => void ask(() => commands.devflowAsks()).then(told), [told])

  useEffect(() => {
    reload()
    return onCarried<null>('devflow:asked', reload)
  }, [reload])

  if (asks.length === 0) return null
  const answer = (id: string, yes: boolean): void => void ask(() => commands.devflowAnswer(id, yes)).then(told)

  return (
    <section className="remind" role="alert" aria-label={t('devflow.name')}>
      <div className="remind__list">
        {asks.map((one) => {
          const said = devflowQuestion(one)
          return (
            <div key={one.id} className="remind__one">
              <span className="remind__what" title={said.why}>
                <b>{said.question}</b>
                <span className="remind__when">{said.why}</span>
              </span>
              <span className="remind__acts">
                <button className="remind__btn" onClick={() => answer(one.id, false)}>
                  {t('devflow.notNow')}
                </button>
                <button className="remind__btn remind__btn--done" onClick={() => answer(one.id, true)}>
                  {said.yes}
                </button>
              </span>
            </div>
          )
        })}
        {problem && <span className="remind__when">{problem}</span>}
      </div>
    </section>
  )
}
