import { useCallback, useEffect, useState } from 'react'

import type { LabsProject, Maintenance } from '../gen/bindings'
import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * Whether a project learns is the person's answer, never a default. A project
 * linked to an orchestrator while off waits under the top bar, the way a
 * Devflow ask does, until Turn on or Not now; Not now is not asked again.
 */

/** The question, worded the same wherever it is asked. */
export function learnQuestion(project?: string): { question: string; why: string; yes: string; no: string } {
  return {
    question: project ? `Turn on learning for ${project}?` : 'Turn on learning for this project?',
    why: 'Its agent reads the project once and keeps what it learns for the orchestrator and its sessions. Nothing reads it until you turn it on.',
    yes: 'Turn on',
    no: 'Not now',
  }
}

/** The projects waiting for the person's answer. */
const waiting = (shown: Maintenance): readonly LabsProject[] =>
  shown.projectAgent ? shown.projectAgentProjects.filter((one) => one.asked && !one.on) : []

export function LearnAsks(): React.JSX.Element | null {
  const [asked, setAsked] = useState<readonly LabsProject[]>([])
  const [problem, setProblem] = useState<string | null>(null)

  const told = useCallback((answer: { data: Maintenance | null; error: string | null }): void => {
    if (answer.data) setAsked(waiting(answer.data))
    setProblem(answer.error)
  }, [])
  const reload = useCallback(() => void ask(() => commands.maintenanceRead()).then(told), [told])

  useEffect(() => {
    reload()
    return onCarried<null>('learning:asked', reload)
  }, [reload])

  if (asked.length === 0) return null
  const answer = (id: string, yes: boolean): void => void ask(() => commands.maintenanceProjectAgent(id, yes)).then(told)

  return (
    <section className="remind" role="alert" aria-label="Project learning">
      <div className="remind__list">
        {asked.map((one) => {
          const said = learnQuestion(one.name)
          return (
            <div key={one.id} className="remind__one">
              <span className="remind__what" title={said.why}>
                <b>{said.question}</b>
                <span className="remind__when">{said.why}</span>
              </span>
              <span className="remind__acts">
                <button className="remind__btn" onClick={() => answer(one.id, false)}>
                  {said.no}
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
