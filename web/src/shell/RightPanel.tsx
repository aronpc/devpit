import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { Changes } from './ChangesPanel'
import { Explorer } from './ExplorerView'
import { History } from './History'
import { ArtifactsView } from './ArtifactsView'
import { OrchestratorBoards } from './OrchestratorBoards'
import { SessionsView } from './SessionsView'
import { useExplorerState } from './useExplorerState'
import { useFileIndex } from './useFileIndex'
import { useShellPick } from './shellStore'
import { useTree } from './useTree'
import { useLiveSessions } from './liveStatus'

const Icon = ({ d, size = 15 }: { d: string; size?: number }): React.JSX.Element => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
)

const FOLDER = 'M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'
const UPLOAD = 'M12 16V4M8 8l4-4 4 4M4 20h16'
const BOARDS = 'M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2ZM9 3v18M15 3v18'
const ARCHIVE = 'M3 4h18v4H3ZM5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4'
const PEOPLE = 'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75'
const CLOCK = 'M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0'

/* Explorer or Changes: two questions about the same tree, so one is answered
   at a time rather than both being half-visible. The three views' own
   markup lives in `Explorer.tsx`, `Changes.tsx` and `History.tsx` — this
   file is just the tab bar and the project the tabs share. */
/** Opens the right panel on an orchestrator's tab: `detail` is the tab. */
export const SHOW_PANEL = 'devpit:show-panel'

export function RightPanel({ onOpenFile }: { onOpenFile: (path: string) => void }): React.JSX.Element {
  const { project, files, active, toggleFiles } = useShellPick((all) => ({
    project: all.project,
    files: all.files,
    active: all.active,
    toggleFiles: all.toggleFiles,
  }))
  const tree = useTree(project?.id ?? null)
  const index = useFileIndex(project?.id ?? null)
  const { view: chosen, setView, mode, setMode, query, setQuery } = useExplorerState(project?.id ?? null)
  /* An orchestrator's folder is its notes, not a repository: nothing to
     commit and no history. What it has instead: the sessions it works with,
     first, and the boards of the projects it is linked to. */
  const git = !project?.orchestrator
  const [orch, setOrch] = useState<'sessions' | 'boards' | 'tree'>('sessions')
  /* Artifacts are every project's, outside the three views the explorer
     remembers, so which one is on is kept here. */
  const [arts, setArts] = useState(false)
  const view = arts ? 'artifacts' : git ? chosen : orch
  const pick = (next: () => void): void => {
    setArts(false)
    next()
  }

  /* Which tab is on belongs to the project it was picked in: carried over,
     an orchestrator's Artifacts opened on the next project's too. */
  const here = project?.id ?? null
  useEffect(() => {
    setArts(false)
    setOrch('sessions')
  }, [here])
  const waiting = useLiveSessions(project?.orchestrator ?? null).filter((one) => one.waiting).length

  /* Asked for from elsewhere — the chat's session count — opened if hidden. */
  useEffect(() => {
    const wanted = (event: Event): void => {
      const tab = (event as CustomEvent<'sessions' | 'boards'>).detail
      setArts(false)
      setOrch(tab)
      if (!files) toggleFiles()
    }
    window.addEventListener(SHOW_PANEL, wanted)
    return () => window.removeEventListener(SHOW_PANEL, wanted)
  }, [files, toggleFiles])

  /* Follows the focused tab, not a click remembered here — opening a file
     from the palette or from Changes must highlight the same row. */
  const current = active?.kind === 'file' ? (active.path ?? null) : null

  /* This panel never unmounts — CSS collapses it to zero width instead, per
     the reference: remounting on every visibility change is what caused an
     IPC storm elsewhere in this app. So reopening it needs its own reload,
     not a mount effect. Guarded to the false→true edge so the initial mount
     (panel already open) does not double the fetch `useTree` already makes. */
  const wasOpen = useRef(files)
  useEffect(() => {
    if (files && !wasOpen.current) tree.reload()
    wasOpen.current = files
  }, [files, tree.reload])

  return (
    <aside className="rp">
      {/* Icons, and the name of the one in front. Every name at once took
          the whole width of a panel somebody has just been given a handle to
          make narrower; no name at all left them guessing where they were. */}
      <div className="rp__bar">
        <button
          className="rtab"
          aria-selected={view === 'tree'}
          title={t('workspace.rightPanel.explorer')}
          aria-label={t('workspace.rightPanel.explorer')}
          onClick={() => pick(() => (setOrch('tree'), setView('tree')))}
        >
          <Icon d={FOLDER} size={15} />
          {view === 'tree' && <span className="rtab__l">{t('workspace.rightPanel.explorer')}</span>}
        </button>
        {!git && (
          <button className="rtab rtab--lead" aria-selected={view === 'sessions'} title={t('workspace.rightPanel.sessions')} aria-label={t('workspace.rightPanel.sessions')} onClick={() => pick(() => setOrch('sessions'))}>
            <Icon d={PEOPLE} size={15} />
            {view === 'sessions' && <span className="rtab__l">{t('workspace.rightPanel.sessions')}</span>}
            {waiting > 0 && (
              <span className="rtab__n" data-tone="warn" title={t('workspace.rightPanel.waiting', { count: waiting })}>
                {waiting}
              </span>
            )}
          </button>
        )}
        {!git && (
          <button className="rtab rtab--lead" aria-selected={view === 'boards'} title={t('workspace.rightPanel.boards')} aria-label={t('workspace.rightPanel.boards')} onClick={() => pick(() => setOrch('boards'))}>
            <Icon d={BOARDS} size={15} />
            {view === 'boards' && <span className="rtab__l">{t('workspace.rightPanel.boards')}</span>}
          </button>
        )}
        {git && (
          <>
          <button
            className="rtab"
            aria-selected={view === 'changes'}
            title={t('workspace.rightPanel.changes')}
            aria-label={t('workspace.rightPanel.changes')}
            onClick={() => pick(() => setView('changes'))}
          >
            <Icon d={UPLOAD} size={15} />
            {view === 'changes' && <span className="rtab__l">{t('workspace.rightPanel.changes')}</span>}
            {/* A zero is not news. The count is here to say there is something
                to look at, and `0` says the opposite while taking the room. */}
            {tree.changes.length > 0 && <span className="rtab__n">{tree.changes.length}</span>}
          </button>
          <button
            className="rtab"
            aria-selected={view === 'history'}
            title={t('workspace.rightPanel.history')}
            aria-label={t('workspace.rightPanel.history')}
            onClick={() => pick(() => setView('history'))}
          >
            <Icon d={CLOCK} size={15} />
            {view === 'history' && <span className="rtab__l">{t('workspace.rightPanel.history')}</span>}
          </button>
          </>
        )}
        <button className="rtab" aria-selected={view === 'artifacts'} title={t('workspace.rightPanel.artifacts')} aria-label={t('workspace.rightPanel.artifacts')} onClick={() => setArts(true)}>
          <Icon d={ARCHIVE} size={15} />
          {view === 'artifacts' && <span className="rtab__l">{t('workspace.rightPanel.artifacts')}</span>}
        </button>
      </div>

      <div className="rview" data-rview="tree" data-open={String(view === 'tree')}>
        <Explorer
          project={project}
          tree={tree}
          index={index}
          mode={mode}
          setMode={setMode}
          query={query}
          setQuery={setQuery}
          current={current}
          onOpen={onOpenFile}
        />
      </div>

      <div className="rview" data-rview="changes" data-open={String(view === 'changes')}>
        <Changes tree={tree} />
      </div>

      {!git && (
        <div className="rview" data-rview="sessions" data-open={String(view === 'sessions')}>
          <SessionsView shown={files && view === 'sessions'} />
        </div>
      )}

      {!git && (
        <div className="rview" data-rview="boards" data-open={String(view === 'boards')}>
          <OrchestratorBoards shown={files && view === 'boards'} />
        </div>
      )}

      <div className="rview" data-rview="artifacts" data-open={String(view === 'artifacts')}>
        <ArtifactsView shown={files && view === 'artifacts'} />
      </div>

      <div className="rview" data-rview="history" data-open={String(view === 'history')}>
        <div className="git__body">
          <History />
        </div>
      </div>
    </aside>
  )
}
