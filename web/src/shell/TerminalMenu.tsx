import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import type { Terminal } from '@xterm/xterm'

import { t } from '../i18n'
import { Clipboard, Close, Columns, Copy, Plus, Rows, SelectAll } from './GitIcons'
import { ask, commands } from './live'
import { Menu, MenuItem, MenuRule } from './Menu'
import { copySelection, pasteClipboard } from './terminalClipboard'
import { useKeys } from './useShortcut'

/*
 * A terminal's right-click, in Orca's order: what to do with text first, then
 * what to do with the pane.
 *
 * The window's own context menu leaves text fields to the system, and a
 * terminal's input is a hidden textarea — so right-click on a terminal used
 * to offer the webview's menu, which cannot reach xterm's selection.
 *
 * No Clear. Clearing is the program's business: xterm wiping its buffer under
 * a full-screen program like Claude Code leaves that program drawing into
 * rows it believes are still there, and the pane comes apart.
 */

/* The pane in a tmux session, and the line that attaches to it copied: what
   reaches a devpit terminal from another machine over ssh. Answers what to
   tell the person. */
export async function openInTmux(projectId: string, paneId: string): Promise<string> {
  const opened = await ask(() => commands.paneOpenInTmux(projectId, paneId))
  if (!opened.data) return opened.error ?? t('workspace.terminal.tmuxFailed')
  const line = opened.data.attach
  return writeText(line).then(
    () => t('workspace.terminal.copiedLine', { line }),
    () => t('workspace.terminal.runLine', { line }),
  )
}

export function TerminalMenu({
  at,
  terminal,
  projectId,
  paneId,
  onClose,
  onFailed,
  onSplit,
  onClosePane,
  onBlocks,
  onSeparate,
}: {
  at: { readonly x: number; readonly y: number }
  terminal: Terminal
  projectId: string
  /** Absent where the pane has no leaf of its own to open elsewhere. */
  paneId?: string
  onClose: () => void
  onFailed: (why: string | null) => void
  onSplit?: (direction: 'horizontal' | 'vertical') => void
  onClosePane?: () => void
  onBlocks?: () => void
  /** Absent for the only pane of a tab, which already has a tab of its own. */
  onSeparate?: () => void
}): React.JSX.Element {
  const keys = useKeys()

  const run = (act: () => void | Promise<unknown>) => () => {
    onClose()
    void Promise.resolve(act()).finally(() => terminal.focus())
  }

  return (
    <Menu at={at} label={t('workspace.terminal.actions')} onClose={onClose}>
      <MenuItem label={t('workspace.terminal.copy')} glyph={<Copy />} keys={keys('copy')} disabled={!terminal.hasSelection()} onPick={run(() => copySelection(terminal))} />
      <MenuItem label={t('workspace.terminal.selectAll')} glyph={<SelectAll />} onPick={run(() => terminal.selectAll())} />
      <MenuItem label={t('workspace.terminal.paste')} glyph={<Clipboard />} keys={keys('paste')} onPick={run(() => pasteClipboard(terminal, projectId).then(onFailed))} />
      {onSplit && (
        <>
          <MenuRule />
          <MenuItem label={t('workspace.terminal.splitRight')} glyph={<Columns />} keys={keys('splitRight')} onPick={run(() => onSplit('horizontal'))} />
          <MenuItem label={t('workspace.terminal.splitDown')} glyph={<Rows />} keys={keys('splitDown')} onPick={run(() => onSplit('vertical'))} />
        </>
      )}
      {onBlocks && (
        <>
          <MenuRule />
          <MenuItem label={t('workspace.terminal.showBlocks')} glyph={<Rows />} onPick={run(onBlocks)} />
        </>
      )}
      {paneId && (
        <>
          <MenuRule />
          <MenuItem label={t('workspace.terminal.openTmux')} glyph={<Copy />} onPick={run(() => openInTmux(projectId, paneId).then(onFailed))} />
        </>
      )}
      {(onClosePane || onSeparate) && <MenuRule />}
      {onSeparate && <MenuItem label={t('workspace.terminal.moveTab')} glyph={<Plus />} onPick={run(onSeparate)} />}
      {onClosePane && <MenuItem label={t('workspace.terminal.closePane')} glyph={<Close />} bad onPick={run(onClosePane)} />}
    </Menu>
  )
}
