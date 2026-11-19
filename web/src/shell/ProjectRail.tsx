import { useEffect, useMemo, useRef, useState } from 'react'

import type { Project } from '../gen/bindings'
import { Close, Folder, Pencil, Plus, Trash } from './GitIcons'
import { t } from '../i18n'
import { ask, commands } from './live'
import { ProjectDialog } from './ProjectDialog'
import { ProjectMark } from './ProjectMark'
import { RailGroup } from './RailGroup'
import { RailOrchestrators } from './RailOrchestrators'
import { RailMenu, type RailItem } from './RailMenu'
import { ordered, placed, saveActiveOnly, savedActiveOnly, saveGroups, savedGroups, saveOrder, savedOrder, saveShut, savedShut, sections, shown } from './rail'
import { remembered } from './tabs'
import { useRailDrag, type Grab, type Spot } from './useRailDrag'
import { useShell } from './useShell'
import { menuPoint } from './menuRules'
import { useMemoryWaiting } from './memoryWaiting'

/*
 * Every project, one icon each, down the left edge.
 *
 * The picker in the top bar hid the others behind a click. The rail keeps them
 * all in sight the way a chat app keeps its servers — narrow enough to cost
 * nothing, and widening over the content, never beside it, to show names
 * while the pointer is on it. The terminal under it never changes size.
 *
 * A project with tabs open is *active*, and says so with a dot: that is where
 * work was left. Right-click edits its name, group and mark; dragging puts it
 * somewhere else — beside another project, into another group — and the order
 * is the person's. Groups fold, move by their heading, and are renamed or
 * dissolved from it.
 */

type Menu = { x: number; y: number; items: readonly RailItem[] }

export function ProjectRail({ onAddProject, onRemove }: { onAddProject: () => void; onRemove: (id: string) => void }): React.JSX.Element {
  const { projects, project, setProject, open, reloadProjects } = useShell()
  const [order, setOrder] = useState<readonly string[]>(savedOrder)
  const [shut, setShut] = useState<ReadonlySet<string>>(savedShut)
  const [activeOnly, setActiveOnly] = useState<ReadonlySet<string>>(savedActiveOnly)
  const [groupOrder, setGroupOrder] = useState<readonly string[]>(savedGroups)
  const scroller = useRef<HTMLDivElement>(null)
  const [editing, setEditing] = useState<Project | null>(null)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [menu, setMenu] = useState<Menu | null>(null)
  const memory = useMemoryWaiting()
  /* Orchestrators are drawn apart, above; they are not the person's projects. */
  const list = useMemo(() => ordered(projects.filter((one) => !one.orchestrator), order), [projects, order])

  /* A project that was not here a moment ago was just added: offer its mark
     while it is the thing being looked at. Not on the first list, which is
     every project there already was. */
  const known = useRef<ReadonlySet<string> | null>(null)
  useEffect(() => {
    const ids = new Set(projects.map((one) => one.id))
    const was = known.current
    known.current = ids
    if (!was || was.size === 0) return
    // An orchestrator is named when it is made; its mark has a default.
    const added = projects.find((one) => !was.has(one.id) && !one.orchestrator)
    if (added) setEditing(added)
  }, [projects])

  const tabsOf = (id: string): number => (id === project?.id ? open.length : remembered(id).open.length)

  const all = useMemo(() => sections(list, groupOrder), [list, groupOrder])

  /* A project into another group keeps everything else about it; only the
     group changes, the same edit the dialog makes. */
  const regroup = (one: Project, group: string | null): Promise<unknown> =>
    (one.group ?? null) === group
      ? Promise.resolve()
      : ask(() => commands.projectEdit(one.id, one.name, group, one.icon, one.color)).then(() => reloadProjects())

  const putProject = (id: string, beside: string, after: boolean): void => {
    const next = placed(list.map((one) => one.id), id, beside, after)
    setOrder(next)
    saveOrder(next)
  }

  const drop = (grab: Grab, spot: Spot): void => {
    if (grab.kind === 'project') {
      const moving = list.find((one) => one.id === grab.id)
      if (!moving) return
      if (spot.kind === 'project') {
        const target = list.find((one) => one.id === spot.key)
        if (!target || target.id === moving.id) return
        /* Beside a project is in its group, wherever the project came from. */
        putProject(moving.id, target.id, spot.after)
        void regroup(moving, target.group ?? null)
        return
      }
      /* On a heading: into that group, at its top. */
      const first = all.find((one) => one.group === spot.key)?.projects[0]
      if (first && first.id !== moving.id) putProject(moving.id, first.id, false)
      void regroup(moving, spot.key)
      return
    }
    /* A group lands beside the group of whatever it was dropped on. */
    const target = spot.kind === 'group' ? spot.key : list.find((one) => one.id === spot.key)?.group
    if (!target || target === grab.name) return
    const names = all.flatMap((one) => (one.group ? [one.group] : []))
    const next = placed(names, grab.name, target, spot.after)
    setGroupOrder(next)
    saveGroups(next)
  }
  const drag = useRailDrag(scroller, drop)
  const overOf = (kind: Spot['kind'], key: string): string | undefined => {
    const spot = drag.spot
    if (!drag.grab || !spot || spot.kind !== kind || spot.key !== key) return undefined
    /* A project over a heading goes into the group; everything else goes
       beside what it is over. */
    if (drag.grab.kind === 'project' && kind === 'group') return 'into'
    return spot.after ? 'after' : 'before'
  }

  const fold = (group: string): void => {
    const next = new Set(shut)
    if (next.has(group)) next.delete(group)
    else next.add(group)
    setShut(next)
    saveShut(next)
  }

  const renameGroup = (from: string, to: string): void => {
    void ask(() => commands.projectGroupRename(from, to)).then(() => {
      reloadProjects()
      /* The fold follows the group to its new name. */
      if (shut.has(from)) {
        const next = new Set(shut)
        next.delete(from)
        if (to.trim()) next.add(to.trim())
        setShut(next)
        saveShut(next)
      }
    })
  }

  const projectMenu = (one: Project): readonly RailItem[] => [
    { label: t('workspace.railMenu.editProject'), glyph: <Pencil />, act: () => setEditing(one) },
    { label: t('workspace.railMenu.reveal'), glyph: <Folder />, act: () => void ask(() => commands.pathReveal(one.rootPath)) },
    'rule',
    { label: t('workspace.railMenu.remove'), glyph: <Trash />, act: () => onRemove(one.id), bad: true },
  ]

  const filter = (group: string): void =>
    setActiveOnly((was) => {
      const next = new Set(was)
      if (!next.delete(group)) next.add(group)
      saveActiveOnly(next)
      return next
    })

  const groupMenu = (group: string): readonly RailItem[] => [
    activeOnly.has(group)
      ? { label: t('workspace.railMenu.showAll'), glyph: <Folder />, act: () => filter(group) }
      : { label: t('workspace.railMenu.showActive'), glyph: <Folder />, act: () => filter(group) },
    'rule',
    { label: t('workspace.railMenu.renameGroup'), glyph: <Pencil />, act: () => setRenaming(group) },
    { label: t('workspace.railMenu.ungroup'), glyph: <Close />, act: () => renameGroup(group, '') },
  ]

  return (
    /* Held open while something is dragged: WebKit drops `:hover` during a
       drag, and a rail that folds to icons under the pointer moves every
       target out from under it. */
    <nav className="rail" aria-label={t('workspace.projectRail.label')} data-held={drag.grab || menu || editing ? 'true' : undefined}>
      <div className="rail__panel">
        <RailOrchestrators onMenu={(at, items) => setMenu({ ...at, items })} onRemove={onRemove} onEdit={setEditing} />
        <div className="rail__list" ref={scroller}>
          {all.map((section) => {
            const folded = section.group !== null && shut.has(section.group)
            const only =
              section.group !== null && activeOnly.has(section.group)
                ? { active: (id: string) => tabsOf(id) > 0, current: project?.id ?? null }
                : undefined
            const listed = shown(section, folded, only)
            return (
              <div className="rail__sect" key={section.group ?? ''} data-folded={folded ? 'true' : undefined}>
                {section.group && (
                  <RailGroup
                    name={section.group}
                    hidden={folded ? 0 : section.projects.length - listed.length}
                    folded={folded}
                    renaming={renaming === section.group}
                    onToggle={() => fold(section.group!)}
                    onMenu={(at) => setMenu({ ...at, items: groupMenu(section.group!) })}
                    onRename={(to) => {
                      if (renaming !== section.group) return
                      setRenaming(null)
                      if (to !== null && to.trim() !== section.group) renameGroup(section.group!, to)
                    }}
                    over={overOf('group', section.group)}
                    grabbed={drag.grab?.kind === 'group' && drag.grab.name === section.group}
                    onPress={drag.press({ kind: 'group', name: section.group })}
                    clicked={drag.clicked}
                  />
                )}
                {listed.map((one) => {
                  const tabs = tabsOf(one.id)
                  const learned = memory.get(one.id) ?? 0
                  const branch = (one.worktrees.find((tree) => tree.current) ?? one.worktrees[0])?.branch
                  return (
                    <button
                      key={one.id}
                      className="rail__i"
                      aria-current={one.id === project?.id ? 'true' : undefined}
                      data-active={tabs > 0 ? 'true' : undefined}
                      data-unreadable={one.unreadable ? 'true' : undefined}
                      data-dragging={drag.grab?.kind === 'project' && drag.grab.id === one.id ? 'true' : undefined}
                      data-over={overOf('project', one.id)}
                      data-drop="project"
                      data-key={one.id}
                      title={one.unreadable ?? undefined}
                      onPointerDown={drag.press({ kind: 'project', id: one.id })}
                      onClick={() => drag.clicked() && one.id !== project?.id && setProject(one.id)}
                      onContextMenu={(event) => {
                        event.preventDefault()
                        event.stopPropagation()
                        setMenu({ ...menuPoint(event), items: projectMenu(one) })
                      }}
                    >
                      <span className="rail__pill" />
                      <span className="rail__ico">
                        <ProjectMark project={one} />
                        <span className="rail__dot" />
                        {learned > 0 && (
                          <span className="rail__memo" aria-hidden="true">
                            {learned}
                          </span>
                        )}
                      </span>
                      <span className="rail__text">
                        <span className="rail__n">{one.name}</span>
                        <span className="rail__s">
                          {branch && <span className="rail__b">{branch}</span>}
                          {tabs > 0 && <span className="rail__t">{t('workspace.projectRail.tabs', { count: tabs })}</span>}
                          {learned > 0 && <span className="rail__m">{t('workspace.projectRail.agent', { count: learned })}</span>}
                        </span>
                      </span>
                    </button>
                  )
                })}
              </div>
            )
          })}
        </div>
        <button className="rail__i rail__add" onClick={onAddProject} title={t('workspace.projectRail.addTitle')}>
          <span className="rail__pill" />
          <span className="rail__ico">
            <span className="pmark pmark--add">
              <Plus size={16} />
            </span>
          </span>
          <span className="rail__text">
            <span className="rail__n">{t('workspace.projectRail.add')}</span>
          </span>
        </button>
      </div>

      {menu && <RailMenu at={menu} items={menu.items} onClose={() => setMenu(null)} />}
      {editing && <ProjectDialog project={editing} onClose={() => setEditing(null)} />}
    </nav>
  )
}
