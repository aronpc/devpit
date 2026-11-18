import { useEffect, useState } from 'react'

import type { DevflowCardEvidence, DevflowExploit, DevflowReviewed } from '../gen/bindings'
import { decisionsLine, decisionsTone } from './devflowDecisions'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { onCarried } from './window'

/*
 * What a card's Devflow runs left, compact: the newest gate, the newest
 * review's reviewers and confirmed exploits, where Decisions stands, and
 * the files that run attached. Nothing at all for a card Devflow never ran on.
 */

const seconds = (ms: number): string => (ms >= 60000 ? `${Math.floor(ms / 60000)} min ${Math.round((ms % 60000) / 1000)} s` : `${Math.round(ms / 1000)} s`)

export function reviewerSaid(one: DevflowReviewed): string {
  if (one.status !== 'answered') return `skipped${one.reason ? ` — ${one.reason}` : ''}`
  const said = [`${one.findings} ${one.findings === 1 ? 'finding' : 'findings'}`]
  if (one.durationMs != null) said.push(seconds(one.durationMs))
  if (one.costUsd != null) said.push(`$${one.costUsd.toFixed(2)}`)
  return said.join(' · ')
}

/** Where a confirmed exploit is, and the test the gate wants green for it. */
export function exploitSaid(one: DevflowExploit): string {
  const at = one.file ? `${one.file}${one.line != null ? `:${one.line}` : ''} · ` : ''
  return `${at}${one.test}`
}

export function CardDevflow({ cardId }: { cardId: string }): React.JSX.Element | null {
  const { project, show } = useShell()
  const projectId = project?.id ?? null
  const [evidence, setEvidence] = useState<DevflowCardEvidence | null>(null)

  useEffect(() => {
    if (!projectId) return
    const read = (): void => void ask(() => commands.devflowCardEvidence(projectId, cardId)).then((answer) => setEvidence(answer.data))
    read()
    /* A run attaches its files and signs a comment on the card: look again then. */
    const ran = onCarried<string>('run:changed', (changed) => changed === cardId && read())
    const commented = onCarried<string>('board:changed', (changed) => changed === projectId && read())
    return () => {
      ran()
      commented()
    }
  }, [projectId, cardId])

  if (!evidence || (!evidence.gate && evidence.reviewers.length === 0 && evidence.exploits.length === 0 && evidence.artifacts.length === 0)) return null
  const gate = evidence.gate
  const open = (name: string): void => {
    const path = `${evidence.folder}/${name}`
    show('file', { id: `file:${path}`, path, title: name.split('/').pop() })
  }
  return (
    <section>
      <h2 className="cardp__h">Devflow</h2>
      {gate && (
        <div className="crun" data-state={gate.passed ? 'ok' : 'failed'} title={gate.missing.join('\n')}>
          <span className="crun__b">
            <span className="crun__t">Gate {gate.passed ? 'passed' : 'not passed'}</span>
            <span className="crun__s">
              {gate.withEvidence} of {gate.criteria} criteria with evidence{gate.missing[0] ? ` · ${gate.missing[0]}` : ''}
            </span>
          </span>
        </div>
      )}
      {evidence.reviewers.map((one) => (
        <div className="crun" key={`${one.agent}:${one.persona}`} data-state={one.status === 'answered' ? 'ok' : 'lost'}>
          <span className="crun__b">
            <span className="crun__t">
              {one.agent} · {one.persona}
            </span>
            <span className="crun__s">{reviewerSaid(one)}</span>
          </span>
        </div>
      ))}
      {evidence.decisions && evidence.decisions.state !== 'off' && (
        <div className="crun" data-state={decisionsTone(evidence.decisions)}>
          <span className="crun__b">
            <span className="crun__t">Decisions</span>
            <span className="crun__s">{decisionsLine(evidence.decisions)}</span>
          </span>
        </div>
      )}
      {evidence.verdict && <p className="crun__s">Review: {evidence.verdict === 'request_changes' ? 'changes requested' : evidence.verdict}</p>}
      {evidence.exploits.map((one) => (
        <div className="crun" key={one.test} data-state="failed" title={one.test}>
          <span className="crun__b">
            <span className="crun__t">Exploit confirmed{one.title ? ` · ${one.title}` : ''}</span>
            <span className="crun__s">{exploitSaid(one)}</span>
          </span>
        </div>
      ))}
      {evidence.artifacts.map((name) => (
        <button key={name} className="conv__act" title={name} onClick={() => open(name)}>
          {name.split('/').slice(3).join('/') || name}
        </button>
      ))}
    </section>
  )
}
