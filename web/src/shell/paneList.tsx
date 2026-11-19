/* One row per pane: what the tab shows and what the strip calls it.
 *
 * The prototype kept these in the markup and moved the elements around; a
 * strip rendered from state needs them as data, or reordering a tab means
 * reordering DOM the renderer also owns. */

import { t } from '../i18n'

export type PaneName =
  | 'chat'
  | 'term'
  | 'board'
  | 'devflow'
  | 'skills'
  | 'mcps'
  | 'workspace'
  | 'files'
  | 'file'
  | 'diff'
  | 'plugins'
  | 'drawing'
  | 'note'
  | 'browser'
  | 'schedules'
  | 'ask'
  | 'memory'

export interface PaneMeta {
  readonly name: PaneName
  readonly title: string
  readonly label: string
  readonly icon: React.JSX.Element
}

export const PANES: readonly PaneMeta[] = [
  { name: 'chat', get title() { return t('workspace.pane.chat.title') }, get label() { return t('workspace.pane.chat.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9.9 9.9 0 0 1-4.2-.9L3 20.5l1.6-4.4A8.4 8.4 0 0 1 3.6 11.5 8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4Z" /></svg>) },
  { name: 'term', get title() { return t('workspace.pane.term.title') }, get label() { return t('workspace.pane.term.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="m4 17 6-6-6-6M12 19h8" /></svg>) },
  { name: 'browser', get title() { return t('workspace.pane.browser.title') }, get label() { return t('workspace.pane.browser.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18a14 14 0 0 1 0-18" /></svg>) },
  { name: 'board', get title() { return t('workspace.pane.board.title') }, get label() { return t('workspace.pane.board.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M9 3v18M15 3v18" /></svg>) },
  { name: 'devflow', get title() { return t('workspace.pane.devflow.title') }, get label() { return t('workspace.pane.devflow.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><circle cx="5" cy="6" r="2" /><circle cx="12" cy="18" r="2" /><circle cx="19" cy="6" r="2" /><path d="M5 8v2a4 4 0 0 0 4 4h1M19 8v2a4 4 0 0 1-4 4h-1" /></svg>) },
  { name: 'skills', get title() { return t('workspace.pane.skills.title') }, get label() { return t('workspace.pane.skills.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" /><rect x="7" y="12" width="10" height="9" rx="2" /></svg>) },
  { name: 'mcps', get title() { return t('workspace.pane.mcps.title') }, get label() { return t('workspace.pane.mcps.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M9 2v6M15 2v6" /><path d="M6 8h12v4a6 6 0 0 1-12 0Z" /><path d="M12 18v4" /></svg>) },
  { name: 'plugins', get title() { return t('workspace.pane.plugins.title') }, get label() { return t('workspace.pane.plugins.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2 4 6v6c0 5 3.4 9.1 8 10 4.6-.9 8-5 8-10V6Z" /><path d="m9 12 2 2 4-4" /></svg>) },
  { name: 'note', get title() { return t('workspace.pane.note.title') }, get label() { return t('workspace.pane.note.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9Z" /><path d="M14 3v6h6M8 13h8M8 17h5" /></svg>) },
  { name: 'drawing', get title() { return t('workspace.pane.drawing.title') }, get label() { return t('workspace.pane.drawing.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 19l7-7 3 3-7 7-3-3ZM18 13l-1.5-7.5L2 2l3.5 14.5L13 18ZM2 2l7.6 7.6" /></svg>) },
  { name: 'workspace', get title() { return t('workspace.pane.workspace.title') }, get label() { return t('workspace.pane.workspace.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>) },
  { name: 'schedules', get title() { return t('workspace.pane.schedules.title') }, get label() { return t('workspace.pane.schedules.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" /><path d="M16 2v4M8 2v4M3 10h18M12 14v3l2 1" /></svg>) },
  { name: 'ask', get title() { return t('workspace.pane.ask.title') }, get label() { return t('workspace.pane.ask.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9" /><path d="M9.5 9.2a2.6 2.6 0 0 1 5 .8c0 1.7-2.5 2.3-2.5 3.8M12 17h.01" /></svg>) },
  { name: 'memory', get title() { return t('workspace.pane.memory.title') }, get label() { return t('workspace.pane.memory.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 4a3 3 0 0 0-3 3 3 3 0 0 0-3 3c0 1 .4 1.8 1 2.4A3 3 0 0 0 9 18a3 3 0 0 0 3 2 3 3 0 0 0 3-2 3 3 0 0 0 2-5.6c.6-.6 1-1.4 1-2.4a3 3 0 0 0-3-3 3 3 0 0 0-3-3Z" /><path d="M12 4v16" /></svg>) },
  { name: 'files', get title() { return t('workspace.pane.files.title') }, get label() { return t('workspace.pane.files.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>) },
  { name: 'diff', get title() { return t('workspace.pane.diff.title') }, get label() { return t('workspace.pane.diff.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M9 3v18M15 3v18M3 9h18M3 15h18" /></svg>) },
  { name: 'file', get title() { return t('workspace.pane.file.title') }, get label() { return t('workspace.pane.file.label') }, icon: (<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" /><path d="M14 2v5h5" /></svg>) },
]

export const paneMeta = (name: PaneName): PaneMeta =>
  PANES.find((pane) => pane.name === name)!
