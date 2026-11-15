import { useEffect, useRef, useState, useSyncExternalStore } from 'react'

import type { Maintenance, ProjectAgentAnswer } from '../gen/bindings'
import { learnQuestion } from './LearnAsks'
import { ask, commands } from './live'
import { Markdown } from './MarkdownView'
import { useShellPick } from './shellStore'
import { composing } from './typing'
import { useShell } from './useShell'

/*
 * "Ask the project": a chat with the project's own agent, the resident
 * read-only process the orchestrator asks. An answer names its files; it
 * becomes a fact only as a proposal the person approves under the top bar.
 * On a project that is off it asks the person to turn its learning on first.
 */

interface Turn {
  /* An answer finds its own question by this, whatever was asked since. */
  readonly id: number
  readonly question: string
  readonly answer?: ProjectAgentAnswer
  readonly error?: string
  readonly proposed?: boolean
}

/* Per project, while the window lives: the agent keeps its own conversation.
 * Every pane open on it redraws, so an answer reaches a pane opened again. */
const kept = new Map<string, readonly Turn[]>()
const watching = new Set<() => void>()
let asked = 0
const put = (projectId: string, change: (was: readonly Turn[]) => readonly Turn[]): void => {
  kept.set(projectId, change(kept.get(projectId) ?? []))
  watching.forEach((redraw) => redraw())
}
const watch = (redraw: () => void): (() => void) => {
  watching.add(redraw)
  return () => watching.delete(redraw)
}
const none: readonly Turn[] = []

const oneLine = (text: string): string => text.split(/\s+/).filter(Boolean).join(' ')

/** Whether the person turned `id` on, as Labs has it; null while unknown. */
const turnedOn = (shown: Maintenance | null, id: string): boolean | null =>
  shown ? (shown.projectAgentProjects.find((one) => one.id === id)?.on ?? false) : null

export function AskPane(): React.JSX.Element {
  const { project } = useShell()
  const here = project && !project.orchestrator ? project : null
  const hereId = here?.id ?? null
  const show = useShellPick((shell) => shell.show)
  const [draft, setDraft] = useState('')
  const [editing, setFact] = useState<{ projectId: string; at: number; text: string } | null>(null)
  const end = useRef<HTMLDivElement>(null)
  const turns = useSyncExternalStore(watch, () => (hereId ? (kept.get(hereId) ?? none) : none))
  const waiting = turns.some((turn) => !turn.answer && !turn.error)
  const fact = editing?.projectId === hereId ? editing : null
  const [on, setOn] = useState<boolean | null>(null)
  const [offered, setOffered] = useState(true)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    if (!hereId) return
    setOn(null)
    setOffered(true)
    setProblem(null)
    let current = true
    void ask(() => commands.maintenanceRead()).then((shown) => {
      if (current) setOn(turnedOn(shown.data, hereId))
    })
    return () => {
      current = false
    }
  }, [hereId])

  const last = turns.at(-1)
  useEffect(() => end.current?.scrollIntoView?.({ block: 'end' }), [last])

  if (!here) return <p className="acc__note">Ask the project belongs to a project.</p>
  const send = (): void => {
    const question = draft.trim()
    if (!question || waiting) return
    if (on === false) return setOffered(true)
    const projectId = here.id
    const id = ++asked
    setDraft('')
    put(projectId, (was) => [...was, { id, question }])
    void ask(() => commands.projectAgentChat(projectId, question)).then((said) => {
      const error = said.error ?? (said.data ? undefined : 'the project agent gave no answer')
      put(projectId, (was) => was.map((turn) => (turn.id === id ? { ...turn, answer: said.data ?? undefined, error } : turn)))
    })
  }
  const turn = (yes: boolean): void => {
    const projectId = here.id
    void ask(() => commands.maintenanceProjectAgent(projectId, yes)).then((said) => {
      setProblem(said.error)
      if (said.data) setOn(turnedOn(said.data, projectId))
      if (!said.error) setOffered(false)
    })
  }
  const offer = learnQuestion()
  const propose = (at: number, text: string, cited: readonly string[]): void => {
    const projectId = here.id
    void ask(() => commands.projectAgentFact(projectId, text, [...cited])).then((said) => {
      if (said.error) return put(projectId, (was) => was.map((turn, i) => (i === at ? { ...turn, error: said.error ?? undefined } : turn)))
      setFact(null)
      put(projectId, (was) => was.map((turn, i) => (i === at ? { ...turn, proposed: true, error: undefined } : turn)))
    })
  }

  return (
    <div className="pask">
      <div className="pask__log scroll" aria-live="polite">
        {on === false &&
          (offered ? (
            <div className="pask__off" role="group" aria-label={offer.question}>
              <p className="acc__note">
                <b>{offer.question}</b> {offer.why}
              </p>
              <button className="btn btn--sm" onClick={() => turn(false)}>
                {offer.no}
              </button>
              <button className="btn btn--sm btn--go" onClick={() => turn(true)}>
                {offer.yes}
              </button>
            </div>
          ) : (
            <p className="acc__note">Off for this project: its agent neither learns nor answers until you turn it on.</p>
          ))}
        {problem && <p className="acc__note voice__bad" role="alert">{problem}</p>}
        {on !== false && turns.length === 0 && (
          <p className="acc__note">
            Ask {here.name}&apos;s agent about its code and its memory. It reads and never writes; an answer becomes a fact only once you approve it.
          </p>
        )}
        {turns.map((turn, at) => (
          <div key={turn.id} className="pask__turn">
            <p className="pask__q">{turn.question}</p>
            {turn.answer && (
              <div className="pask__a">
                <Markdown source={turn.answer.answer} />
                {turn.answer.cited.length > 0 && (
                  <div className="pask__cited" aria-label="Files it cites">
                    {turn.answer.cited.map((path) => (
                      <button key={path} className="pask__path" onClick={() => show('file', { id: `file:${path}`, path })}>
                        {path}
                      </button>
                    ))}
                  </div>
                )}
                {turn.proposed ? (
                  <p className="acc__note">Proposed as a fact: it waits for your approval under the top bar.</p>
                ) : fact?.at === at ? (
                  <div className="pask__fact">
                    <textarea aria-label="Fact" value={fact.text} onChange={(event) => setFact({ projectId: here.id, at, text: event.target.value })} />
                    <button className="btn btn--sm" onClick={() => setFact(null)}>Cancel</button>
                    <button className="btn btn--sm btn--go" disabled={!fact.text.trim()} onClick={() => propose(at, fact.text, turn.answer?.cited ?? [])}>
                      Propose
                    </button>
                  </div>
                ) : (
                  turn.answer.cited.length > 0 && (
                    <button className="btn btn--sm" onClick={() => setFact({ projectId: here.id, at, text: oneLine(turn.answer?.answer ?? '') })}>
                      Save as fact
                    </button>
                  )
                )}
              </div>
            )}
            {turn.error && <p className="acc__note voice__bad" role="alert">{turn.error}</p>}
            {!turn.answer && !turn.error && <p className="acc__note">Reading…</p>}
          </div>
        ))}
        <div ref={end} />
      </div>
      <div className="pask__compose">
        <textarea
          aria-label="Ask the project"
          placeholder={`Ask ${here.name}…`}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' || event.shiftKey || composing(event)) return
            event.preventDefault()
            send()
          }}
        />
        <button className="btn btn--sm btn--go" disabled={waiting || !draft.trim()} onClick={send}>
          Ask
        </button>
      </div>
    </div>
  )
}
