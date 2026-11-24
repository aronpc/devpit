import { useState } from 'react'

import type { Project, ProjectProposal } from '../gen/bindings'
import { GroupPicker } from './GroupPicker'
import { applied, proposalWords } from './projectProposal'
import { t } from '../i18n'

/*
 * One proposed change to an orchestrator's projects, made with one click or
 * edited first: the folder's name, its group, and whether it is linked here.
 */

export function ProposalCard({
  proposal,
  hereId,
  projects,
  editing: startEditing = false,
  onDone,
  onDrop,
}: {
  proposal: ProjectProposal
  hereId: string
  projects: readonly Project[]
  /** Opened at its fields, as the Boards panel's own "Add project" is. */
  editing?: boolean
  onDone: () => void
  onDrop: () => void
}): React.JSX.Element {
  const [editing, setEditing] = useState(startEditing)
  const [name, setName] = useState(proposal.name)
  const [group, setGroup] = useState(proposal.group ?? projects.find((one) => one.id === proposal.projectId)?.group ?? '')
  const [link, setLink] = useState(proposal.link ?? false)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const groups = [...new Set(projects.map((one) => one.group).filter((one): one is string => Boolean(one)))].sort()

  const wanted: ProjectProposal = editing
    ? { ...proposal, name: name.trim() || proposal.name, group: group.trim() || null, link: proposal.link === null ? null : link }
    : proposal

  const make = (): void => {
    setBusy(true)
    void applied(wanted, hereId, projects).then((error) => {
      setBusy(false)
      setProblem(error)
      if (!error) onDone()
    })
  }

  return (
    <div className="proposal" role="group" aria-label={t('workspace.proposalCard.label')}>
      {editing ? (
        <div className="proposal__fields">
          <span className="proposal__path" title={proposal.path}>
            {proposal.path}
          </span>
          <label className="proposal__row">
            <span>{t('workspace.proposalCard.name')}</span>
            <input className="proposal__input" value={name} onChange={(event) => setName(event.target.value)} aria-label={t('workspace.proposalCard.name')} />
          </label>
          <label className="proposal__row">
            <span>{t('workspace.proposalCard.group')}</span>
            <GroupPicker value={group} groups={groups} onChange={setGroup} />
          </label>
          {/* A proposal that leaves the link alone has no link to set. */}
          {proposal.link !== null && (
            <label className="proposal__row proposal__row--check">
              <input type="checkbox" checked={link} onChange={(event) => setLink(event.target.checked)} />
              <span>{t('workspace.proposalCard.link')}</span>
            </label>
          )}
        </div>
      ) : (
        <p className="proposal__words">{proposalWords(proposal, projects)}?</p>
      )}
      {problem && <p className="proposal__problem">{problem}</p>}
      <div className="proposal__acts">
        <button className="btn" onClick={onDrop} disabled={busy}>
          {editing && startEditing ? t('workspace.proposalCard.cancel') : t('workspace.proposalCard.drop')}
        </button>
        {!editing && (
          <button className="btn" onClick={() => setEditing(true)} disabled={busy}>
            {t('workspace.proposalCard.edit')}
          </button>
        )}
        <button className="btn btn--go" onClick={make} disabled={busy}>
          {editing ? t('workspace.proposalCard.save') : t('workspace.proposalCard.yes')}
        </button>
      </div>
    </div>
  )
}
