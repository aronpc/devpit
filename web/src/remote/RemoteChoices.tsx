import { useState } from 'react'

import type { IslandAsked } from '../gen/bindings'
import { t } from '../i18n'

/*
 * A question an agent put to the person, answered from the Remote with their
 * choices: one question at a time, all sent together, the way the agent
 * waits for them. A bare Allow would answer nothing, so there is none.
 */

export function RemoteChoices({
  asked,
  onPick,
  onDeny,
}: {
  asked: readonly IslandAsked[]
  onPick: (chosen: string[][]) => void
  onDeny: () => void
}): React.JSX.Element {
  const [chosen, setChosen] = useState<string[][]>([])
  const [ticked, setTicked] = useState<readonly string[]>([])
  const one = asked[chosen.length]!
  const last = chosen.length === asked.length - 1

  const answer = (labels: string[]): void => {
    const all = [...chosen, labels]
    if (all.length === asked.length) return onPick(all)
    setChosen(all)
    setTicked([])
  }
  const choose = (label: string): void => {
    if (!one.multi) return answer([label])
    setTicked(ticked.includes(label) ? ticked.filter((one) => one !== label) : [...ticked, label])
  }

  return (
    <>
      {(one.header || asked.length > 1) && (
        <span className="rm__dim">
          {[one.header, asked.length > 1 ? t('remote.waiting.step', { n: chosen.length + 1, count: asked.length }) : null].filter(Boolean).join(' · ')}
        </span>
      )}
      <span className="rm__what">{one.question}</span>
      <div className="rm__acts rm__choices">
        {one.options.map((label) => (
          <button key={`${chosen.length}:${label}`} className="rm__btn" aria-pressed={one.multi ? ticked.includes(label) : undefined} onClick={() => choose(label)}>
            {label}
          </button>
        ))}
      </div>
      <div className="rm__acts">
        <button className="rm__btn" onClick={() => confirm(t('remote.waiting.confirmDeny', { what: one.question })) && onDeny()}>
          {t('remote.waiting.deny')}
        </button>
        {one.multi && (
          <button className="rm__btn rm__btn--go" disabled={ticked.length === 0} onClick={() => answer([...ticked])}>
            {last ? t('remote.waiting.answer') : t('remote.waiting.next')}
          </button>
        )}
      </div>
    </>
  )
}
