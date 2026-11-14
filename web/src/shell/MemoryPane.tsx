import { useCallback, useEffect, useState } from 'react'

import type { MemoryEntry, MemoryProposal, ProjectMemory } from '../gen/bindings'
import { ask, commands } from './live'
import { useShellPick } from './shellStore'
import { useShell } from './useShell'
import { onCarried } from './window'

/*
 * The project agent's page: what its memory keeps, each entry with its day
 * and the files it rests on, and what waits for the person. A proposal is
 * approved, edited or discarded here as under the top bar; what the agent's
 * own read of the project proposed can be approved at once, since it rests
 * only on the repository's files.
 */

function Sources({ files, cards }: { files: readonly string[]; cards: readonly string[] }): React.JSX.Element {
  const show = useShellPick((shell) => shell.show)
  return (
    <span className="pmem__src">
      {files.map((path) => (
        <button key={path} className="pask__path" onClick={() => show('file', { id: `file:${path}`, path })}>
          {path}
        </button>
      ))}
      {cards.length > 0 && <span className="pmem__cards">{cards.join(', ')}</span>}
    </span>
  )
}

function Kept({ title, entries }: { title: string; entries: readonly MemoryEntry[] }): React.JSX.Element {
  return (
    <section className="pmem__part" aria-label={title}>
      <h3 className="pmem__h">
        {title} <span className="pmem__n">{entries.length}</span>
      </h3>
      {entries.length === 0 && <p className="acc__note">Nothing yet.</p>}
      {entries.map((one, at) => (
        <div key={`${at} ${one.source} ${one.text}`} className="pmem__one">
          <p className="pmem__text">{one.text}</p>
          <span className="pmem__meta">
            <span className="pmem__day">{one.day}</span>
            <Sources files={one.files} cards={one.cards} />
          </span>
        </div>
      ))}
    </section>
  )
}

export function MemoryPane(): React.JSX.Element {
  const { project } = useShell()
  const here = project && !project.orchestrator ? project : null
  const hereId = here?.id ?? null
  const [memory, setMemory] = useState<ProjectMemory | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [editing, setEditing] = useState<{ of: MemoryProposal; text: string } | null>(null)

  const take = useCallback((said: { data: ProjectMemory | null; error: string | null }) => {
    if (said.data) setMemory(said.data)
    setProblem(said.error)
  }, [])
  const reload = useCallback(() => {
    if (hereId) void ask(() => commands.projectAgentMemory(hereId)).then(take)
  }, [hereId, take])
  useEffect(() => {
    reload()
    return onCarried<string>('lessons:proposed', (id) => id === hereId && reload())
  }, [hereId, reload])

  if (!here) return <p className="acc__note">The project agent belongs to a project.</p>
  const shown = memory?.projectId === here.id ? memory : null
  const decide = (one: MemoryProposal, kept: string | null): void => {
    const { projectId, source, text, replaces } = one.proposal
    setEditing(null)
    void ask(() => commands.lessonDecide(projectId, source, text, replaces, kept)).then((said) => {
      setProblem(said.error)
      reload()
    })
  }
  const learned = shown?.proposals.filter((one) => one.onboarding).length ?? 0

  return (
    <div className="pmem scroll">
      <div className="pmem__bar">
        <p className="acc__note">
          {shown?.learning
            ? 'Learning the project: what it finds waits here for your approval.'
            : shown?.learned
              ? `Learned the project on ${shown.learned}. Nothing enters the memory until you approve it.`
              : 'It learns the project on its own when Labs has it on. Nothing enters the memory until you approve it.'}
        </p>
        <button className="btn btn--sm" disabled={!shown || shown.learning} onClick={() => void ask(() => commands.projectAgentRelearn(here.id)).then(take)}>
          Re-learn
        </button>
      </div>
      {problem && (
        <p className="acc__note voice__bad" role="alert">
          {problem}
        </p>
      )}
      {shown && (
        <>
          <section className="pmem__part" aria-label="Proposed">
            <h3 className="pmem__h">
              Proposed <span className="pmem__n">{shown.proposals.length}</span>
              {learned > 0 && (
                <button className="btn btn--sm btn--go pmem__all" onClick={() => void ask(() => commands.projectAgentApproveLearned(here.id)).then(take)}>
                  Approve all from onboarding ({learned})
                </button>
              )}
            </h3>
            {shown.proposals.length === 0 && <p className="acc__note">Nothing waits for you.</p>}
            {shown.proposals.map((one) => {
              const { source, text, replaces, file, day } = one.proposal
              const edited = editing?.of === one ? editing : null
              return (
                <div key={`${source} ${text} ${replaces ?? ''}`} className="pmem__one" role="group" aria-label={text || `Remove: ${replaces ?? ''}`}>
                  {edited ? (
                    <input className="remind__edit" aria-label="Proposal" value={edited.text} onChange={(event) => setEditing({ of: one, text: event.target.value })} />
                  ) : (
                    <p className="pmem__text">{replaces !== null && !text ? `Remove: ${replaces}` : text}</p>
                  )}
                  <span className="pmem__meta">
                    <span className="pmem__day">{day}</span>
                    <span className="pmem__file">{file}</span>
                    {one.onboarding && <span className="pmem__file">onboarding</span>}
                    <Sources files={one.files} cards={one.cards} />
                    {replaces !== null && text && <span className="pmem__file">was: {replaces}</span>}
                  </span>
                  <span className="remind__acts">
                    <button className="remind__btn" onClick={() => decide(one, null)}>
                      Discard
                    </button>
                    {!edited && (
                      <button className="remind__btn" onClick={() => setEditing({ of: one, text })}>
                        Edit
                      </button>
                    )}
                    <button className="remind__btn remind__btn--done" onClick={() => decide(one, edited ? edited.text : text)}>
                      Approve
                    </button>
                  </span>
                </div>
              )
            })}
          </section>
          <Kept title="Facts" entries={shown.facts} />
          <Kept title="Lessons" entries={shown.lessons} />
          <Kept title="Decisions" entries={shown.decisions} />
        </>
      )}
    </div>
  )
}
