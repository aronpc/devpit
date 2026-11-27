import { useEffect, useState } from 'react'

import mark from '../assets/brand/mark.png'
import { t } from '../i18n'
import { BranchPicker } from './BranchPicker'
import { HeadsDown } from './HeadsDownControl'
import { ask, commands } from './live'
import { Notices } from './Notices'
import { ProjectPicker } from './ProjectPicker'
import { TabStrip } from './TabStrip'
import { useNotices } from './useNotices'
import { useIslandOpens } from './useIslandOpens'
import { useShell } from './useShell'
import { useTrayOpens } from './useTrayOpens'
import { useTree } from './useTree'
import { WindowControls } from './WindowControls'
import { Icon } from './Icon'

/*
 * The window is one project, so the project sits above the columns rather than
 * inside one of them. The lead block is exactly as wide as the sidebar under
 * it, so the first tab starts on the line the content starts on.
 *
 * The two panel toggles sit together on the right, next to the window
 * controls: they are one job — showing and hiding the columns — and split
 * apart they read as two unrelated buttons.
 *
 * The bar carries `data-tauri-drag-region`, so it is the window's handle.
 * Everything interactive in it is a child without the attribute, which is why
 * the buttons still take their own clicks. A double click on it maximizes and
 * restores through Tauri's own drag script: a handler here would toggle twice.
 */
export function TopBar({ onAddProject, maximized }: { onAddProject: () => void; maximized: boolean }): React.JSX.Element {
  const { side, files, toggleSide, toggleFiles, project, show, openCard, managing, openManager, setProject, openPane, openPrefs } =
    useShell()
  useIslandOpens(project?.id ?? null, setProject, openPane, show)
  useTrayOpens(project?.id ?? null, setProject, show, openCard, openPrefs)
  /* One reader for the whole top bar: the panel and the pill draw from the
     same list, so a count on one cannot disagree with the other. */
  const bell = useNotices()
  const [bellOpen, setBellOpen] = useState(false)
  const { totals } = useTree(project?.id ?? null)
  const here = project?.worktrees.find((tree) => tree.current) ?? project?.worktrees[0]
  /* Whether this is the devpit being worked on. It keeps its own home, so the
     projects in it are not the real ones — and two devpits side by side look
     identical without this. */
  const [dev, setDev] = useState(false)
  useEffect(() => {
    void ask(() => commands.appInfo()).then((answer) => setDev(answer.data?.dev === true))
  }, [])

  return (
    <header className="top" data-tauri-drag-region>
      <div className="top__lead">
        <span className="logo">
          <img
            className="mark"
            alt="devpit" // ui-text: ok the product's name
            src={mark}
          />
        </span>
        {dev && (
          <span className="top__dev" title={t('workspace.top.dev')}>
            dev {/* // ui-text: ok the badge a development build wears */}
          </span>
        )}
        <ProjectPicker onAdd={onAddProject} />
        {/* Beside the picker because it answers the question the picker asks:
            the picker changes which project you are in, and this is the view
            that does not have one. */}
        <button
          className="sq26 tip"
          data-tip={t('workspace.top.managerTip')}
          aria-label={t('workspace.top.manager')}
          aria-pressed={managing}
          onClick={openManager}
        >
          <Icon size="s"><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></Icon>
        </button>
      </div>

      <TabStrip />

      <span className="drag" data-tauri-drag-region />

      <HeadsDown
        projectId={project?.id ?? null}
        waiting={bell.waiting.length}
        onOpenCard={(cardId) => {
          show('board')
          openCard(cardId)
        }}
      />
      {here && <BranchPicker branch={here.branch} ahead={here.ahead} />}
      <span className="netstat" hidden={totals.added + totals.removed === 0}>
        <span className="add">+{totals.added}</span>
        <span className="del">&minus;{totals.removed}</span>
      </span>

      {/* Beside the panel toggles rather than in the sidebar: what it has to
          say is about the whole window, and half of it arrives while the
          sidebar is hidden. */}
      <Notices
        bell={bell}
        open={bellOpen}
        setOpen={setBellOpen}
        onOpenCard={(cardId) => {
          show('board')
          openCard(cardId)
        }}
        onOpenPane={(projectId, paneId) => {
          if (projectId && projectId !== project?.id) setProject(projectId)
          openPane(paneId)
        }}
      />

      <button
        className="sq26 tip"
        data-tip={t(side ? 'workspace.top.hideSide' : 'workspace.top.showSide')}
        aria-label={t(side ? 'workspace.top.hideSide' : 'workspace.top.showSide')}
        onClick={toggleSide}
      >
        <Icon size="m">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M9 3v18" />
        </Icon>
      </button>
      <button
        className="sq26 tip"
        data-tip={t(files ? 'workspace.top.hideFiles' : 'workspace.top.showFiles')}
        aria-label={t(files ? 'workspace.top.hideFiles' : 'workspace.top.showFiles')}
        onClick={toggleFiles}
      >
        <Icon size="m">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M15 3v18" />
        </Icon>
      </button>

      <WindowControls maximized={maximized} />
    </header>
  )
}
