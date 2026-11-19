import { useCallback, useEffect, useState } from 'react'

import type { LessonProposal, MemoryDecision, LessonProposals as Proposals } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'
import { t } from '../i18n'

/*
 * What the project agent proposed for its memory from one card, on that card:
 * approved as it is, edited first, or discarded. Never in the top bar: the
 * rest waits on the Project agent page, counted on the project in the rail.
 * A card also shows the decisions that name it.
 */
export function LessonProposals({ projectId, source }: { projectId: string; source: string }): React.JSX.Element | null {
  const [all, setAll] = useState<readonly LessonProposal[]>([])
  const [decisions, setDecisions] = useState<readonly MemoryDecision[]>([])
  const [editing, setEditing] = useState<{ of: LessonProposal; text: string } | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  const told = useCallback((answer: { data: Proposals | null; error: string | null }): void => {
    if (answer.data) {
      setAll(answer.data.proposals)
      setDecisions(answer.data.decisions)
    }
    setProblem(answer.error)
  }, [])
  const reload = useCallback(() => void ask(() => commands.lessonsProposed()).then(told), [told])

  useEffect(() => {
    reload()
    return onCarried<string>('lessons:proposed', reload)
  }, [reload])

  const shown = all.filter((one) => one.projectId === projectId && one.source === source)
  const made = decisions.filter((one) => one.projectId === projectId && one.cards.includes(source))
  if (shown.length === 0 && made.length === 0) return null
  const decide = (one: LessonProposal, kept: string | null): void => {
    setEditing(null)
    void ask(() => commands.lessonDecide(one.projectId, one.source, one.text, one.replaces, kept)).then(told)
  }

  return (
    <>
      {shown.length > 0 && (
        <section className="remind" aria-label={t('workspace.lessons.toApprove')}>
          <div className="remind__list">
            {shown.map((one) => {
              const edited = editing?.of === one ? editing : null
              return (
                <div key={`${one.projectId} ${one.source} ${one.text} ${one.replaces ?? ''}`} className="remind__one">
                  {edited ? (
                    <input
                      className="remind__edit"
                      aria-label={t('workspace.lessons.lesson')}
                      value={edited.text}
                      onChange={(event) => setEditing({ of: one, text: event.target.value })}
                    />
                  ) : (
                    <span className="remind__what" title={one.replaces ?? one.text}>
                      <b>{one.replaces !== null && !one.text ? t('workspace.lessons.remove', { text: one.replaces }) : one.text}</b>
                      <span className="remind__when">
                        {one.project} · {one.source}
                        {one.file !== 'lessons.md' && ` · ${one.file}`}
                        {one.replaces !== null && one.text && t('workspace.lessons.was', { text: one.replaces })}
                      </span>
                    </span>
                  )}
                  <span className="remind__acts">
                    <button className="remind__btn" onClick={() => decide(one, null)}>
                      {t('workspace.lessons.discard')}
                    </button>
                    {!edited && (
                      <button className="remind__btn" onClick={() => setEditing({ of: one, text: one.text })}>
                        {t('workspace.lessons.edit')}
                      </button>
                    )}
                    <button className="remind__btn remind__btn--done" onClick={() => decide(one, edited ? edited.text : one.text)}>
                      {t('workspace.lessons.approve')}
                    </button>
                  </span>
                </div>
              )
            })}
            {problem && <span className="remind__when">{problem}</span>}
          </div>
        </section>
      )}
      {made.length > 0 && (
        <section className="remind" aria-label={t('workspace.lessons.decisionsLabel')}>
          <div className="remind__list">
            {made.map((one) => (
              <div key={`${one.source} ${one.text}`} className="remind__one">
                <span className="remind__what" title={one.text}>
                  <b>{one.text}</b>
                  <span className="remind__when">
                    {t('workspace.lessons.decision', { day: one.day, cards: one.cards.join(', ') })}
                  </span>
                </span>
              </div>
            ))}
          </div>
        </section>
      )}
    </>
  )
}
