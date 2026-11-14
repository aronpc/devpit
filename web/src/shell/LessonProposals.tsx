import { useCallback, useEffect, useState } from 'react'

import type { LessonProposal, MemoryDecision, LessonProposals as Proposals } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * What the project agent proposed for its memory: lessons and decisions a
 * closed card left, and updates or removals of stale entries. Each waits under
 * the top bar, and on its card, until the person approves it as it is, edits
 * it first, or discards it. A card also shows the decisions that name it.
 */
export function LessonProposals({ projectId, source }: { projectId?: string; source?: string }): React.JSX.Element | null {
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

  const here = all.filter((one) => (!projectId || one.projectId === projectId) && (!source || one.source === source))
  /* The agent's own read proposes 15 to 30 at once: one line per project here,
   * one by one on the project's Project agent page. */
  const shown = here.filter((one) => !one.onboarding)
  const read = [...new Map(here.filter((one) => one.onboarding).map((one) => [one.projectId, one.project])).entries()].map(([id, name]) => ({
    id,
    name,
    count: here.filter((one) => one.onboarding && one.projectId === id).length,
  }))
  const made = source ? decisions.filter((one) => one.projectId === projectId && one.cards.includes(source)) : []
  if (shown.length === 0 && read.length === 0 && made.length === 0) return null
  const decide = (one: LessonProposal, kept: string | null): void => {
    setEditing(null)
    void ask(() => commands.lessonDecide(one.projectId, one.source, one.text, one.replaces, kept)).then(told)
  }

  return (
    <>
      {(shown.length > 0 || read.length > 0) && (
        <section className="remind" aria-label="Lessons to approve">
          <div className="remind__list">
            {read.map((one) => (
              <div key={`read ${one.id}`} className="remind__one">
                <span className="remind__what">
                  <b>
                    {one.name}: {one.count} facts and decisions from its own read of the project
                  </b>
                  <span className="remind__when">one by one on its Project agent page</span>
                </span>
                <span className="remind__acts">
                  <button className="remind__btn remind__btn--done" onClick={() => void ask(() => commands.projectAgentApproveLearned(one.id)).then((said) => (said.error ? setProblem(said.error) : reload()))}>
                    Approve all
                  </button>
                </span>
              </div>
            ))}
            {shown.map((one) => {
              const edited = editing?.of === one ? editing : null
              return (
                <div key={`${one.projectId} ${one.source} ${one.text} ${one.replaces ?? ''}`} className="remind__one">
                  {edited ? (
                    <input
                      className="remind__edit"
                      aria-label="Lesson"
                      value={edited.text}
                      onChange={(event) => setEditing({ of: one, text: event.target.value })}
                    />
                  ) : (
                    <span className="remind__what" title={one.replaces ?? one.text}>
                      <b>{one.replaces !== null && !one.text ? `Remove: ${one.replaces}` : one.text}</b>
                      <span className="remind__when">
                        {one.project} · {one.source}
                        {one.file !== 'lessons.md' && ` · ${one.file}`}
                        {one.replaces !== null && one.text && ` · was: ${one.replaces}`}
                      </span>
                    </span>
                  )}
                  <span className="remind__acts">
                    <button className="remind__btn" onClick={() => decide(one, null)}>
                      Discard
                    </button>
                    {!edited && (
                      <button className="remind__btn" onClick={() => setEditing({ of: one, text: one.text })}>
                        Edit
                      </button>
                    )}
                    <button className="remind__btn remind__btn--done" onClick={() => decide(one, edited ? edited.text : one.text)}>
                      Approve
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
        <section className="remind" aria-label="Decisions from this card">
          <div className="remind__list">
            {made.map((one) => (
              <div key={`${one.source} ${one.text}`} className="remind__one">
                <span className="remind__what" title={one.text}>
                  <b>{one.text}</b>
                  <span className="remind__when">
                    decisions.md · {one.day} · {one.cards.join(', ')}
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
