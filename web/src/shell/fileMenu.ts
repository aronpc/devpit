import { t } from '../i18n'
import type { ShortcutId } from './shortcuts'

/*
 * The file row's context menu, as data.
 *
 * Apart from the component so a test can call `wired` on it. What this
 * replaces was four entries that closed the menu and did nothing, which the
 * `a_control_either_works_or_goes` guard cannot catch: it asks whether a
 * handler exists, not whether it does anything. An entry here either draws a
 * rule or names an action, and `wired` is what fails when one stops doing so.
 *
 * "Open beside" is gone rather than disabled. Nothing in the shell opens a
 * second pane yet, and AGENTS.md is explicit that a control which cannot be
 * wired leaves the screen — the markup is in git for when it becomes real.
 */

export type ActionId =
  | 'open'
  | 'copyPath'
  | 'reveal'
  | 'newFile'
  | 'newFolder'
  | 'rename'
  | 'delete'

export interface Entry {
  readonly label?: string
  readonly key?: string
  /** A key from the registry, printed as it is bound now. */
  readonly shortcut?: ShortcutId
  readonly rule?: true
  /** Drawn as destructive. Only what cannot be undone from here earns it. */
  readonly bad?: true
  readonly act?: ActionId
}

export const FILE_MENU: readonly Entry[] = [
  { get label() { return t('workspace.menu.file.open') }, key: '↵', act: 'open' },
  { rule: true },
  { get label() { return t('workspace.menu.file.newFile') }, act: 'newFile' },
  { get label() { return t('workspace.menu.file.newFolder') }, act: 'newFolder' },
  { rule: true },
  { get label() { return t('workspace.menu.file.copyPath') }, act: 'copyPath' },
  { get label() { return t('workspace.menu.file.reveal') }, act: 'reveal' },
  { rule: true },
  { get label() { return t('workspace.menu.file.rename') }, shortcut: 'rename', act: 'rename' },
  { get label() { return t('workspace.menu.file.delete') }, act: 'delete', bad: true },
]

/** Whether every entry either draws a rule or does something. */
export const wired = (menu: readonly (Entry & { readonly run?: unknown })[]): boolean =>
  menu.every((entry) => entry.rule === true || entry.act !== undefined || entry.run !== undefined)
