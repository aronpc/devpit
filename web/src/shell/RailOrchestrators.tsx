import { useEffect, useMemo, useRef, useState } from 'react'

import type { Project, Thread } from '../gen/bindings'
import { ForwardedMark } from './ForwardedMark'
import { ChevronDown, Folder, Pencil, Plus, Trash } from './GitIcons'
import { ask, commands } from './live'
import { OrchestratorDialog } from './OrchestratorDialog'
import { ProjectMark } from './ProjectMark'
import type { RailItem } from './RailMenu'
import { menuPoint } from './menuRules'
import { useShell } from './useShell'

/*
 * The orchestrators, as a group of their own above the projects.
 *
 * Each is a project devpit keeps for itself — its own folder, brief and
 * notes — speaking as one account; several may share an account. They are
 * made here, not added, and grouped apart because they are not work of the
 * person's: they are where the person looks at all of it at once.
 */

/** The conversation to pick back up: the one last spoken in. */
export const lastSpoken = (threads: readonly Thread[]): Thread | undefined =>
  [...threads].sort((a, b) => (b.lastAt ?? 0) - (a.lastAt ?? 0))[0]

/** The conversations on the account an orchestrator speaks as now: one begun
 *  on another stays with it, and a switch starts a new one. */
export const onAccount = (threads: readonly Thread[], profile: string | null): readonly Thread[] =>
  threads.filter((one) => !profile || !one.profile || one.profile === profile)

/** Hues an orchestrator without a colour of its own is told apart by, one
 *  picked from its id so it keeps it. */
const TINTS = ['#8b7cf6', '#3fa7d6', '#2bb07f', '#e0a33a', '#e0675c', '#d263b4'] as const

export const tintOf = (id: string): string => {
  let sum = 0
  for (const ch of id) sum = (sum * 31 + ch.charCodeAt(0)) >>> 0
  return TINTS[sum % TINTS.length]
}

const FOLDED = 'devpit.rail.orchestratorsFolded'

function savedFolded(): boolean {
  try {
    return localStorage.getItem(FOLDED) === '1'
  } catch {
    return false
  }
}

type Menu = (at: { x: number; y: number }, items: readonly RailItem[]) => void

/** Oldest first, whatever order the list arrived in. */
export const byAge = (all: readonly Project[]): readonly Project[] => [...all].sort((a, b) => a.id.localeCompare(b.id))

export function RailOrchestrators({
  onMenu,
  onRemove,
  onEdit,
}: {
  onMenu: Menu
  onRemove: (id: string) => void
  /** Name, icon and colour, in the dialog every project uses. */
  onEdit: (project: Project) => void
}): React.JSX.Element {
  const { projects, project, setProject, show, open, reloadProjects } = useShell()
  const [making, setMaking] = useState(false)
  const [account, setAccount] = useState<Project | null>(null)
  const [folded, setFolded] = useState(savedFolded)
  const arriving = useRef<string | null>(null)
  /* Made order: the list arrives last-opened first. */
  const mine = useMemo(() => byAge(projects.filter((one) => one.orchestrator)), [projects])

  useEffect(() => {
    if (!project || project.id !== arriving.current || open.length > 0) return
    arriving.current = null
    void ask(() => commands.chatList(project.id)).then(async (found) => {
      const last = lastSpoken(onAccount(found.data?.conversations ?? [], project.orchestrator))
      if (last) return show('chat', { id: last.id })
      const waiting = await ask(() => commands.forwardedList(project.id))
      const conversation = waiting.data?.requests[0]?.conversation
      show('chat', conversation ? { id: conversation } : undefined)
    })
  }, [project, open.length, show])

  /* devpit's half of the brief is brought up to this build by the shell,
     whichever way an orchestrator comes in front (`useBriefRefresh`). */
  const enter = (one: Project): void => {
    arriving.current = one.id
    if (one.id !== project?.id) setProject(one.id)
  }

  const fold = (): void => {
    setFolded((was) => {
      try {
        localStorage.setItem(FOLDED, was ? '0' : '1')
      } catch {
        // Unsaved, it still folds for now.
      }
      return !was
    })
  }

  const menu = (one: Project): readonly RailItem[] => [
    { label: 'Edit…', glyph: <Pencil />, act: () => onEdit(one) },
    { label: 'Runs as…', glyph: <Pencil />, act: () => setAccount(one) },
    { label: 'Reveal folder', glyph: <Folder />, act: () => void ask(() => commands.pathReveal(one.rootPath)) },
    'rule',
    { label: 'Remove…', glyph: <Trash />, act: () => onRemove(one.id), bad: true },
  ]

  return (
    <div className="rail__sect rail__orch" role="group" aria-label="Orchestrators" data-folded={folded ? 'true' : undefined}>
      <div className="rail__orchhead">
        <button className="rail__group" aria-expanded={!folded} onClick={fold}>
          <span className="rail__gname">Orchestrators</span>
          <span className="rail__gchev" data-open={!folded}>
            <ChevronDown size={12} />
          </span>
        </button>
        <button className="rail__orchadd" onClick={() => setMaking(true)} title="New orchestrator" aria-label="New orchestrator">
          <Plus size={12} />
        </button>
      </div>
      {!folded &&
        mine.map((one) => (
          <button
            key={one.id}
            className="rail__i"
            aria-current={one.id === project?.id ? 'true' : undefined}
            title={one.rootPath}
            onClick={() => enter(one)}
            onContextMenu={(event) => {
              event.preventDefault()
              event.stopPropagation()
              onMenu(menuPoint(event), menu(one))
            }}
          >
            <span className="rail__pill" />
            <span className="rail__ico">
              {one.icon || one.color ? (
                <ProjectMark project={one} />
              ) : (
                <span className="pmark pmark--orch" style={{ '--orch': tintOf(one.id) } as React.CSSProperties}>
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><circle cx="4.5" cy="5" r="2" /><circle cx="19.5" cy="5" r="2" /><circle cx="4.5" cy="19" r="2" /><circle cx="19.5" cy="19" r="2" /><path d="m6 6.4 3.8 3.6M18 6.4l-3.8 3.6M6 17.6l3.8-3.6M18 17.6l-3.8-3.6" /></svg>
                </span>
              )}
            </span>
            <span className="rail__text">
              <span className="rail__n">{one.name}</span>
              <span className="rail__s">orchestrator</span>
            </span>
            <ForwardedMark projectId={one.id} />
          </button>
        ))}
      {!folded && (
        <button className="rail__i rail__orchnew" data-first={mine.length === 0 ? 'true' : undefined} onClick={() => setMaking(true)} title="New orchestrator">
          <span className="rail__pill" />
          <span className="rail__ico">
            <span className="pmark pmark--orch pmark--orchnew">
              <Plus size={14} />
            </span>
          </span>
          <span className="rail__text">
            <span className="rail__n">New orchestrator</span>
          </span>
        </button>
      )}
      {(making || account) && (
        <OrchestratorDialog
          editing={account ?? undefined}
          onClose={() => (setMaking(false), setAccount(null))}
          onMade={(made) => {
            setMaking(false)
            setAccount(null)
            reloadProjects()
            if (!made) return
            arriving.current = made.id
            setProject(made.id)
          }}
        />
      )}
    </div>
  )
}
