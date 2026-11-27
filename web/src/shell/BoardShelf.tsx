import { useCallback, useEffect, useState } from 'react'

import { t } from '../i18n'
import { formatDate } from '../i18n/format'
import type { ArchivedCard, DeleteRefusal } from '../gen/bindings'
import { CardEnding, type Ending } from './CardHeader'
import { ask, commands } from './live'
import { abandoned } from './typing'
import { Icon } from './Icon'
import { Toast } from './Toast'

/*
 * The board's archived cards, and Undo for the card just archived.
 *
 * Archiving had no way back. The row stayed in the table with `archived_at`
 * set and nothing listed it, so a card archived by mistake was a card lost.
 */

/** How long Undo is offered after an archive. */
export const UNDO_MS = 6000

/** Undo, for a few seconds after an archive. On the body: the board animates in. */
export function UndoArchive({
  projectId,
  cardId,
  onRestored,
  onGone,
}: {
  projectId: string
  cardId: string
  onRestored: () => void
  onGone: () => void
}): React.JSX.Element {
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    const timer = window.setTimeout(onGone, UNDO_MS)
    return () => window.clearTimeout(timer)
  }, [cardId, onGone])

  const undo = (): void => {
    void ask(() => commands.cardRestore(projectId, cardId)).then((answer) => {
      if (answer.error) return setProblem(answer.error)
      onRestored()
      onGone()
    })
  }

  return (
    <Toast
      tone={problem ? 'danger' : 'info'}
      icon={<Icon size="m"><rect x="3" y="4" width="18" height="4" rx="1" /><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4" /></Icon>}
      action={{ label: t('board.undo'), onClick: undo }}
    >
      {problem ?? t('board.cardArchived')}
    </Toast>
  )
}

export function ArchivedPane({
  projectId,
  onClose,
  onChanged,
}: {
  projectId: string
  onClose: () => void
  onChanged: () => void
}): React.JSX.Element {
  const [cards, setCards] = useState<readonly ArchivedCard[]>([])
  const [error, setError] = useState<string | null>(null)
  const [ending, setEnding] = useState<{ cardId: string; ending: Ending } | null>(null)

  const load = useCallback(() => {
    void ask(() => commands.boardArchived(projectId)).then((answer) => {
      setError(answer.error)
      if (answer.data) setCards(answer.data.cards)
    })
  }, [projectId])

  useEffect(load, [load])

  useEffect(() => {
    /* A question over the list hears Escape first, as it does over a card. */
    const key = (event: KeyboardEvent): void => {
      if (abandoned(event) && document.querySelector('.ask') === null) onClose()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [onClose])

  const changed = (): void => {
    load()
    onChanged()
  }

  const restore = (cardId: string): void => {
    void ask(() => commands.cardRestore(projectId, cardId)).then((answer) => {
      setError(answer.error)
      if (!answer.error) changed()
    })
  }

  const remove = async (cardId: string, force: boolean): Promise<DeleteRefusal | string | null> => {
    const answer = await ask(() => commands.cardDelete(projectId, cardId, force))
    if (answer.error) return answer.error
    return answer.data?.refused ?? null
  }

  return (
    <div className="cardp" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="cardp__box runsp" role="dialog" aria-modal="true" aria-label={t('board.archivedCards')}>
        <header className="runsp__top">
          <h2 className="runsp__t">{t('board.archived')}</h2>
          <button className="sq26" onClick={onClose} aria-label={t('board.closeArchived')} title={t('board.closeArchived')}>
            <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
          </button>
        </header>

        {error && <p className="wtb__no">{error}</p>}
        {cards.length === 0 && !error && <p className="runsp__none">{t('board.nothingArchived')}</p>}
        {cards.map((card) => (
          <div className="crun" key={card.id}>
            <span className="crun__b">
              <span className="crun__t">{card.title}</span>
              <span className="crun__o">
                {card.columnName ?? t('board.laneGone')}
                {card.archivedAt !== null && ` · ${t('board.archivedAt', { when: formatDate(card.archivedAt * 1000, { dateStyle: 'short', timeStyle: 'short' }) })}`}
              </span>
            </span>
            <span className="pin__acts">
              <button className="btn" onClick={() => restore(card.id)}>
                {t('board.restore')}
              </button>
              <button className="btn" data-danger onClick={() => setEnding({ cardId: card.id, ending: { what: 'delete' } })}>
                {t('board.deleteEllipsis')}
              </button>
            </span>
          </div>
        ))}
      </div>

      {ending && (
        <CardEnding
          ending={ending.ending}
          detail={null}
          summary={t('board.deleteSummary')}
          remove={(force) => remove(ending.cardId, force)}
          onAsk={(next) => setEnding(next ? { cardId: ending.cardId, ending: next } : null)}
          onProblem={setError}
          onDone={changed}
        />
      )}
    </div>
  )
}
