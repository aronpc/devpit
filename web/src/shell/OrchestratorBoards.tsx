import { open as pickFolder } from '@tauri-apps/plugin-dialog'
import { useCallback, useEffect, useMemo, useState } from 'react'

import type { ProjectProposal } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { LINKS_CHANGED } from './projectProposal'
import { ProposalCard } from './ProposalCard'

import { PanelAct, PanelEmpty, PanelHead } from './PanelHead'
import { ProjectMark } from './ProjectMark'
import { useProjectBoards } from './useProjectBoards'
import { useShell } from './useShell'
import { Icon } from './Icon'

/*
 * The boards of the projects this orchestrator is linked to, at a glance,
 * beside its chat. Which projects is the person's choice, made here: an
 * orchestrator reaches only what it was linked to. Each lane says how many
 * cards it holds and opens to their titles; a title goes to the card on its
 * own board. Read, never written — the orchestrator's tools are how it acts.
 */
export function OrchestratorBoards({ shown }: { shown: boolean }): React.JSX.Element {
  const { project: here, projects: every, setProject, show, openCard, openManager, reloadProjects } = useShell()
  /* A folder picked to add, shown as the proposal the chat would show. */
  const [adding, setAdding] = useState<ProjectProposal | null>(null)
  const [linked, setLinked] = useState<readonly string[] | null>(null)
  const [linking, setLinking] = useState<Set<string> | null>(null)
  const [said, setSaid] = useState<string | null>(null)
  const candidates = useMemo(() => every.filter((one) => !one.orchestrator), [every])
  const projects = useMemo(() => candidates.filter((one) => linked?.includes(one.id)), [candidates, linked])
  const { boards, failed, reload } = useProjectBoards(projects)
  const [open, setOpen] = useState<string | null>(null)

  const readLinks = useCallback(() => {
    if (!here) return
    void ask(() => commands.orchestratorLinks(here.id)).then((answer) => setLinked(answer.data ?? []))
  }, [here])

  /* Linked from a proposal in the chat, the panel reads its links again. */
  useEffect(() => {
    window.addEventListener(LINKS_CHANGED, readLinks)
    return () => window.removeEventListener(LINKS_CHANGED, readLinks)
  }, [readLinks])

  const addFolder = async (): Promise<void> => {
    const picked = await pickFolder({ directory: true, multiple: false })
    if (typeof picked !== 'string') return
    const known = candidates.find((one) => one.rootPath === picked)
    setAdding({ id: 'local', projectId: known?.id ?? null, path: picked, name: known?.name ?? picked.split(/[\\/]/).filter(Boolean).pop() ?? picked, group: known?.group ?? null, link: true })
  }

  /* By group, with the group itself a box that takes all of it. */
  const byGroup = useMemo(() => {
    const groups = new Map<string, typeof candidates>()
    for (const one of candidates) groups.set(one.group ?? '', [...(groups.get(one.group ?? '') ?? []), one])
    return [...groups.entries()].sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : a.localeCompare(b)))
  }, [candidates])

  /* The panel stays mounted; what is on the boards moves while it is away. */
  useEffect(() => {
    if (!shown) return
    readLinks()
    reload()
  }, [shown, reload, readLinks])

  const saveLinks = (): void => {
    if (!here || !linking) return
    void ask(() => commands.orchestratorLink(here.id, [...linking])).then((answer) => {
      setSaid(answer.error)
      if (answer.error) return
      setLinking(null)
      readLinks()
    })
  }

  const go = (projectId: string, cardId?: string): void => {
    setProject(projectId)
    show('board')
    if (cardId) openCard(cardId)
  }

  return (
    <div className="oboards">
      <PanelHead title={t('workspace.orchestratorBoards.title')} meta={linked && linked.length > 0 ? t('workspace.orchestratorBoards.linked', { count: linked.length }) : null}>
        <PanelAct label={linking ? t('workspace.orchestratorBoards.stopLinking') : t('workspace.orchestratorBoards.linkProjects')} active={linking !== null} onClick={() => setLinking(linking ? null : new Set(linked ?? []))}>
          <Icon size="s"><path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7" /></Icon>
        </PanelAct>
        <PanelAct label={t('workspace.orchestratorBoards.addProject')} onClick={() => void addFolder()}>
          <Icon size="s"><path d="M12 5v14M5 12h14" /></Icon>
        </PanelAct>
        <PanelAct label={t('workspace.orchestratorBoards.everyBoard')} onClick={openManager}>
          <Icon size="s"><path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" /></Icon>
        </PanelAct>
      </PanelHead>
      {adding && here && (
        <ProposalCard
          proposal={adding}
          hereId={here.id}
          projects={candidates}
          editing
          onDrop={() => setAdding(null)}
          onDone={() => {
            setAdding(null)
            reloadProjects()
            readLinks()
          }}
        />
      )}
      {linking && (
        <div className="oboards__link" role="group" aria-label={t('workspace.orchestratorBoards.linkGroup')}>
          <p className="pempty__d">{t('workspace.orchestratorBoards.linkSaid')}</p>
          {byGroup.map(([group, members]) => (
            <div className="oboards__group" key={group || 'none'}>
              {group && (
                <label className="oboards__pick oboards__pick--group">
                  <input
                    type="checkbox"
                    checked={members.every((one) => linking.has(one.id))}
                    onChange={(event) =>
                      setLinking((was) => {
                        const next = new Set(was)
                        for (const one of members) {
                          if (event.target.checked) next.add(one.id)
                          else next.delete(one.id)
                        }
                        return next
                      })
                    }
                  />
                  <span>{group}</span>
                </label>
              )}
              {members.map((one) => (
                <label className="oboards__pick" key={one.id}>
                  <input
                    type="checkbox"
                    checked={linking.has(one.id)}
                    onChange={(event) =>
                      setLinking((was) => {
                        const next = new Set(was)
                        if (event.target.checked) next.add(one.id)
                        else next.delete(one.id)
                        return next
                      })
                    }
                  />
                  <ProjectMark project={one} />
                  <span>{one.name}</span>
                </label>
              ))}
            </div>
          ))}
          {said && <p className="pempty__d">{said}</p>}
          <div className="oboards__save">
            <button className="btn" onClick={() => setLinking(null)}>
              {t('workspace.orchestratorBoards.cancel')}
            </button>
            <button className="btn btn--go" onClick={saveLinks}>
              {t('workspace.orchestratorBoards.save')}
            </button>
          </div>
        </div>
      )}
      {linked?.length === 0 && !linking && (
        <PanelEmpty title={t('workspace.orchestratorBoards.noneLinked')} hint={t('workspace.orchestratorBoards.noneLinkedHint')}>
          <button className="btn" onClick={() => setLinking(new Set())}>
            {t('workspace.orchestratorBoards.linkProjects')}
          </button>
        </PanelEmpty>
      )}
      {failed && <p className="pempty__d">{failed}</p>}
      {linked !== null && linked.length > 0 && boards === null && <p className="pempty__d">{t('workspace.orchestratorBoards.reading')}</p>}
      {boards?.map(({ project, board }) => (
        <section className="oboards__p" key={project.id}>
          <button className="oboards__name" onClick={() => go(project.id)} title={t('workspace.orchestratorBoards.openBoard', { name: project.name })}>
            <ProjectMark project={project} />
            <span>{project.name}</span>
            <span className="oboards__n">{board.cards.length}</span>
          </button>
          {board.cards.length === 0 && <p className="oboards__lempty">{t('workspace.orchestratorBoards.noCards')}</p>}
          {board.columns.map((column) => {
            const cards = board.cards.filter((card) => card.columnId === column.id).sort((a, b) => a.position - b.position)
            if (cards.length === 0) return null
            const key = `${project.id}:${column.id}`
            return (
              <div className="oboards__lane" key={column.id}>
                <button className="oboards__lname" aria-expanded={open === key} onClick={() => setOpen((was) => (was === key ? null : key))}>
                  <span>{column.name}</span>
                  <span className="oboards__n">{cards.length}</span>
                </button>
                {open === key && (
                  <ul className="oboards__cards">
                    {cards.map((card) => (
                      <li key={card.id}>
                        <button className="oboards__card" onClick={() => go(project.id, card.id)} title={card.title}>
                          {card.title}
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )
          })}
        </section>
      ))}
    </div>
  )
}
