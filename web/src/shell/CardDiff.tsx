import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { ask, commands } from './live'
import { DiffFiles, Findings } from './DiffFiles'
import { notesOf, SHAPE, tally, type Note } from './diffNotes'
import { severityWord } from './found'
import { went } from './problems'

/*
 * What this card has changed, against where it started.
 *
 * Against `base_ref` and never against HEAD — which is the whole reason the
 * card records one. A card branched three days ago and compared to today's
 * main would report every commit somebody else made as its own work.
 *
 * Asked for rather than loaded: a diff is a `git diff` over a whole checkout,
 * and most of the time somebody opening a card wants to read its description.
 * The button says how much there is once it knows.
 *
 * A card whose work happened in a checkout devpit did not make still has its
 * Devflow review, and this is the one place a finding is called a false
 * positive: without a checkout it shows the findings alone.
 */

export function CardDiff({ cardId, checkout = true }: { cardId: string; checkout?: boolean }): React.JSX.Element | null {
  const [raw, setRaw] = useState<string | null>(null)
  const [files, setFiles] = useState<readonly string[]>([])
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [notes, setNotes] = useState<readonly Note[]>([])

  const look = async (): Promise<void> => {
    setBusy(true)
    const [answer, found] = await Promise.all([
      ask(() => commands.cardDiff(cardId)),
      ask(() => commands.cardFindings(cardId)),
    ])
    setBusy(false)
    setNotes(found.data ? notesOf(found.data) : [])
    setProblem(answer.error)
    if (!answer.data) return
    setFiles(answer.data.files)
    setRaw(answer.data.diff)
  }

  useEffect(() => {
    if (checkout) return
    let live = true
    void ask(() => commands.cardFindings(cardId)).then((found) => {
      if (live) setNotes(found.data ? notesOf(found.data) : [])
    })
    return () => {
      live = false
    }
  }, [cardId, checkout])

  const dismiss = async (note: Note, dismissed: boolean): Promise<void> => {
    const answer = await ask(() => commands.findingDismiss(note.runId, note.at, dismissed))
    if (!went(answer)) return
    setNotes((was) => was.map((one) => (one === note ? { ...one, dismissed } : one)))
  }

  const onDismiss = (note: Note, dismissed: boolean): void => void dismiss(note, dismissed)

  if (!checkout) {
    if (notes.length === 0) return null
    return (
      <section className="cdiff">
        <h2 className="cardp__h">{t('card.reviewFindings')}</h2>
        <Tally notes={notes} />
        <Findings notes={notes} onDismiss={onDismiss} />
      </section>
    )
  }

  return (
    <section className="cdiff">
      <div className="prefs__hrow">
        <h2 className="cardp__h">{t('card.changes')}</h2>
        <button className="btn" disabled={busy} onClick={() => void look()}>
          {raw === null ? t('card.showChanged') : t('card.readAgain')}
        </button>
      </div>

      {problem && <p className="wtb__no">{problem}</p>}

      {raw !== null && files.length === 0 && (
        <p className="pref__d">{t('card.nothingChanged')}</p>
      )}

      {raw && notes.length > 0 && <Tally notes={notes} />}
      {raw && <DiffFiles diff={raw} notes={notes} onDismiss={onDismiss} />}
    </section>
  )
}

/* What the reviews still say, by severity; a click goes to the first one. */
function Tally({ notes }: { notes: readonly Note[] }): React.JSX.Element {
  const counts = tally(notes)
  const first = (): void => {
    const marker = document.querySelector<HTMLElement>('.cdiff__fd:not([data-dismissed])')
    marker?.closest<HTMLDetailsElement>('.cdiff__f')?.setAttribute('open', '')
    marker?.setAttribute('open', '')
    marker?.scrollIntoView({ block: 'center' })
  }
  return (
    <button className="cdiff__tally" onClick={first}>
      {(['blocking', 'worth', 'noted'] as const).map((severity) => (
        <span key={severity} className="fnd__s" data-severity={severity}>
          {SHAPE[severity]} {counts[severity]} {severityWord(severity)}
        </span>
      ))}
    </button>
  )
}
