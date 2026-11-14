import { useEffect, useState } from 'react'

import './shell.css'
import { ContextMenu } from './ContextMenu'
import { DevflowAsks } from './DevflowAsks'
import { Grips } from './Grips'
import { Overlays } from './Overlays'
import { ProjectRail } from './ProjectRail'
import { Panes } from './Panes'
import { RemindersBanner } from './RemindersBanner'
import { QuotaBanner } from './QuotaBanner'
import { ResizeEdges } from './ResizeEdges'
import { RightPanel } from './RightPanel'
import { Sidebar } from './Sidebar'
import { StatusStrip } from './StatusStrip'
import { TopBar } from './TopBar'
import { PluginsProvider } from './usePlugins'
import { ShellProvider } from './useShell'
import { useWindowShell } from './shellStore'
import { BriefRefresh } from './useBriefRefresh'
import { useMaximized } from './useMaximized'
import { useShortcut } from './useShortcut'
import { abandoned } from './typing'

export function AppShell(): React.JSX.Element {
  return (
    <ShellProvider>
      <PluginsProvider>
        <Window />
      </PluginsProvider>
    </ShellProvider>
  )
}

/*
 * Three columns under one bar. The bar spans the whole width because the
 * window is one project, and the project is not a property of a column.
 *
 * A closed panel is a zero-width column rather than an unmounted one: the
 * terminal keeps its size and its scrollback instead of being rebuilt every
 * time the tree is hidden.
 */
function Window(): React.JSX.Element {
  const shell = useWindowShell()
  const { side, files, widths, signedIn, palette, openPalette, closePalette } = shell
  const [signIn, setSignIn] = useState(false)
  const [adding, setAdding] = useState(false)
  const [removing, setRemoving] = useState<string | null>(null)
  /* Opening a file is the same act wherever it was clicked: the tree on the
     right, the Files pane, or the palette. The id is the path, so the same
     file twice lands on the tab you already have. */
  const openInTab = (path: string): void =>
    shell.show('file', { id: `file:${path}`, path, title: path.split('/').pop() })
  /* Maximised, the rounded corners square off — a rounded rectangle floating
     in the middle of a screen it should fill reads as a bug. */
  const maximized = useMaximized()

  /* The account state is read by CSS, which decides which half of the
     sidebar foot exists. */
  useEffect(() => {
    document.body.dataset.account = signedIn ? 'in' : 'out'
  }, [signedIn])

  useEffect(() => {
    document.documentElement.dataset.theme = shell.theme
  }, [shell.theme])

  // Only the window's own. A pane's keys are the pane's: splitting belongs
  // to the terminal being split, ⌘P to the picker it opens.
  useShortcut('palette', () => (palette ? closePalette() : openPalette()))
  useShortcut('settings', () => shell.openPrefs('general'))
  useShortcut('terminal', () => shell.show('term'))
  useShortcut('chat', () => shell.show('chat'))

  useEffect(() => {
    const key = (event: KeyboardEvent): void => {
      if (!abandoned(event)) return
      // One layer a press, the top one first: a question asked over Settings
      // used to take Settings down with it.
      if (removing) setRemoving(null)
      else if (adding) setAdding(false)
      else if (signIn) setSignIn(false)
      else if (palette) closePalette()
      else if (shell.managing) shell.closeManager()
      else shell.closePrefs()
    }
    document.addEventListener('keydown', key)
    return () => document.removeEventListener('keydown', key)
  }, [shell, palette, closePalette, removing, adding, signIn])

  return (
    <div
      className="app"
      data-max={String(maximized)}
      /* On the shell and not on the grid: the top bar's lead is the same
         width as the sidebar, and it is not inside the grid. One variable,
         two readers. */
      style={
        {
          '--sidebar-w': `${widths.sidebar}px`,
          '--files-w': `${widths.files}px`,
        } as React.CSSProperties
      }
    >
      <ResizeEdges />
      <ContextMenu />
      <TopBar onAddProject={() => setAdding(true)} maximized={maximized} />
      <RemindersBanner />
      <QuotaBanner />
      <DevflowAsks />

      <div className="win" data-side={side ? 'open' : 'closed'} data-files={files ? 'open' : 'closed'}>
        <ProjectRail onAddProject={() => setAdding(true)} onRemove={setRemoving} />
        <Sidebar onSearch={openPalette} onSignIn={() => setSignIn(true)} />
        <Panes />
        <RightPanel onOpenFile={openInTab} />
        <Grips />
      </div>

      <StatusStrip />
      <BriefRefresh />

      <Overlays
        signIn={signIn}
        onSignInClose={() => setSignIn(false)}
        adding={adding}
        onAddingClose={() => setAdding(false)}
        onAdd={() => setAdding(true)}
        removing={removing}
        onRemove={setRemoving}
        onRemovingClose={() => setRemoving(null)}
      />
    </div>
  )
}
