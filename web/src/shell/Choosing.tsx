import { useRef, useState } from 'react'

import type { IslandAsked } from '../gen/bindings'
import { keyOn, toggled, useFocused, type Chosen } from './choices'
import { committed } from './typing'
import { t } from '../i18n'
import { CardState } from './CardState'
import { Icon } from './Icon'

/*
 * A question the agent put to the person, as buttons: 1-9 picks, the arrows
 * move along, Escape refuses. Several questions are asked one at a time and
 * answered together, the way the agent waits for them.
 */

export function Choosing({
  asked,
  onPick,
  onRefuse,
}: {
  asked: readonly IslandAsked[]
  onPick: (chosen: Chosen) => void
  onRefuse: () => void
}): React.JSX.Element {
  const card = useFocused<HTMLElement>()
  const buttons = useRef<(HTMLButtonElement | null)[]>([])
  const [chosen, setChosen] = useState<Chosen>([])
  const [ticked, setTicked] = useState<readonly number[]>([])
  const [own, setOwn] = useState('')
  const one = asked[chosen.length]!

  const answer = (labels: readonly string[]): void => {
    const all = [...chosen, labels]
    if (all.length === asked.length) return onPick(all)
    setChosen(all)
    setTicked([])
    setOwn('')
    card.current?.focus()
  }
  const choose = (n: number): void => (one.multi ? setTicked(toggled(ticked, n)) : answer([one.options[n]!]))

  const onKeyDown = (event: React.KeyboardEvent<HTMLElement>): void => {
    /* Typed into the field, a digit is a word. */
    if (event.target instanceof HTMLInputElement && event.key !== 'Escape') return
    const at = buttons.current.indexOf(document.activeElement as HTMLButtonElement)
    const keyed = keyOn(event.key, one.options.length, at < 0 ? null : at)
    if (!keyed) return
    event.preventDefault()
    if (keyed === 'refuse') onRefuse()
    else if ('pick' in keyed) choose(keyed.pick)
    else buttons.current[keyed.move]?.focus()
  }

  return (
    <section className="asking" ref={card} tabIndex={-1} onKeyDown={onKeyDown} aria-label={one.question}>
      <div className="asking__t">
        <CardState state="waiting" />
        {one.header && <span className="asking__h">{one.header}</span>}
        {asked.length > 1 && <span className="asking__step">{t('workspace.choosing.step', { n: chosen.length + 1, total: asked.length })}</span>}
      </div>
      <p className="asking__q">{one.question}</p>
      <div className="asking__opts">
        {one.options.map((label, n) => (
          <button
            key={`${chosen.length}:${n}`}
            ref={(button) => void (buttons.current[n] = button)}
            className="asking__opt"
            aria-pressed={one.multi ? ticked.includes(n) : undefined}
            onClick={() => choose(n)}
          >
            <span className="asking__n">{n + 1}</span>
            {label}
            {one.multi && ticked.includes(n) && (
              <Icon size="s" className="asking__tick">
                <path d="M20 6 9 17l-5-5" />
              </Icon>
            )}
          </button>
        ))}
      </div>
      <input
        className="asking__own"
        value={own}
        placeholder={t('workspace.choosing.own')}
        onChange={(event) => setOwn(event.target.value)}
        onKeyDown={(event) => {
          if (committed(event) && own.trim()) answer([own.trim()])
        }}
      />
      <div className="asking__row">
        <button className="btn" onClick={onRefuse}>
          {t('workspace.choosing.refuse')}
        </button>
        {one.multi && (
          <button className="btn btn--go" disabled={ticked.length === 0} onClick={() => answer(ticked.map((n) => one.options[n]!))}>
            {t('workspace.choosing.confirm')}
          </button>
        )}
      </div>
    </section>
  )
}
