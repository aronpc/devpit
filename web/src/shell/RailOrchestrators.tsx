import { useEffect, useMemo, useRef, useState } from 'react'

import type { Project, Thread } from '../gen/bindings'
import { ForwardedMark } from './ForwardedMark'
import { ChevronDown, Folder, Pencil, Plus, Trash } from './GitIcons'
import { t } from '../i18n'
import { ask, commands } from './live'
import { OrchestratorDialog } from './OrchestratorDialog'
import { ProjectMark } from './ProjectMark'
import type { RailItem } from './RailMenu'
import { menuPoint } from './menuRules'
import { orchestratorsInOrder, placed, saveOrchestrators, savedOrchestrators } from './rail'
import { useRailDrag, type Grab, type Spot } from './useRailDrag'
import { useShell } from './useShell'
import { Icon } from './Icon'

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

export function RailOrchestrators({
  onMenu,
  onRemove,
  onEdit,
  onHold,
}: {
  onMenu: Menu
  onRemove: (id: string) => void
  /** Name, icon and colour, in the dialog every project uses. */
  onEdit: (project: Project) => void
  /** Told while an orchestrator is being dragged, so the rail stays open. */
  onHold: (held: boolean) => void
}): React.JSX.Element {
  const { projects, project, setProject, show, open, reloadProjects } = useShell()
  const [making, setMaking] = useState(false)
  const [account, setAccount] = useState<Project | null>(null)
  const [folded, setFolded] = useState(savedFolded)
  const [order, setOrder] = useState<readonly string[]>(savedOrchestrators)
  const arriving = useRef<string | null>(null)
  const rows = useRef<HTMLDivElement>(null)
  /* In the person's order, the ones not placed yet after by age. The list
     comes back last-opened first, and opening one must not move it. */
  const mine = useMemo(() => orchestratorsInOrder(projects.filter((one) => one.orchestrator), order), [projects, order])

  /* The hook lets an orchestrator land only on another one. */
  const drop = (grab: Grab, spot: Spot): void => {
    if (grab.kind !== 'orchestrator') return
    const next = placed(mine.map((one) => one.id), grab.id, spot.key, spot.after)
    setOrder(next)
    saveOrchestrators(next)
  }
  const drag = useRailDrag(rows, drop)
  const held = drag.grab !== null
  useEffect(() => onHold(held), [held, onHold])
  const overOf = (key: string): string | undefined =>
    drag.grab && drag.spot?.key === key ? (drag.spot.after ? 'after' : 'before') : undefined

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
    { label: t('workspace.railMenu.edit'), glyph: <Pencil />, act: () => onEdit(one) },
    { label: t('workspace.railMenu.runsAs'), glyph: <Pencil />, act: () => setAccount(one) },
    { label: t('workspace.railMenu.reveal'), glyph: <Folder />, act: () => void ask(() => commands.pathReveal(one.rootPath)) },
    'rule',
    { label: t('workspace.railMenu.remove'), glyph: <Trash />, act: () => onRemove(one.id), bad: true },
  ]

  return (
    <div className="rail__sect rail__orch" role="group" aria-label={t('workspace.railOrchestrators.title')} data-folded={folded ? 'true' : undefined}>
      <div className="rail__orchhead">
        <button className="rail__group" aria-expanded={!folded} onClick={fold}>
          <span className="rail__gname">{t('workspace.railOrchestrators.title')}</span>
          <span className="rail__gchev" data-open={!folded}>
            <ChevronDown size={12} />
          </span>
        </button>
        <button className="rail__orchadd" onClick={() => setMaking(true)} title={t('workspace.railOrchestrators.new')} aria-label={t('workspace.railOrchestrators.new')}>
          <Plus size={12} />
        </button>
      </div>
      {!folded && (
        /* Rows and the button that makes another scroll together, so with
           many the group keeps its height and the projects below stay. */
        <div className="rail__orchrows" ref={rows}>
          {mine.map((one) => (
          <button
            key={one.id}
            className="rail__i"
            aria-current={one.id === project?.id ? 'true' : undefined}
            data-dragging={drag.grab?.kind === 'orchestrator' && drag.grab.id === one.id ? 'true' : undefined}
            data-over={overOf(one.id)}
            data-drop="orchestrator"
            data-key={one.id}
            title={one.rootPath}
            onPointerDown={drag.press({ kind: 'orchestrator', id: one.id })}
            onClick={() => drag.clicked() && enter(one)}
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
                  <Icon size="m"><circle cx="12" cy="12" r="3" /><circle cx="4.5" cy="5" r="2" /><circle cx="19.5" cy="5" r="2" /><circle cx="4.5" cy="19" r="2" /><circle cx="19.5" cy="19" r="2" /><path d="m6 6.4 3.8 3.6M18 6.4l-3.8 3.6M6 17.6l3.8-3.6M18 17.6l-3.8-3.6" /></Icon>
                </span>
              )}
            </span>
            <span className="rail__text">
              <span className="rail__n">{one.name}</span>
              <span className="rail__s">{t('workspace.railOrchestrators.one')}</span>
            </span>
            <ForwardedMark projectId={one.id} />
          </button>
          ))}
          <button className="rail__i rail__orchnew" data-first={mine.length === 0 ? 'true' : undefined} onClick={() => setMaking(true)} title={t('workspace.railOrchestrators.new')}>
            <span className="rail__pill" />
            <span className="rail__ico">
              <span className="pmark pmark--orch pmark--orchnew">
                <Plus size={14} />
              </span>
            </span>
            <span className="rail__text">
              <span className="rail__n">{t('workspace.railOrchestrators.new')}</span>
            </span>
          </button>
        </div>
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
