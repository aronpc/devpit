import { t } from '../i18n'
import { bare, comboOf, keyOf, labelOf, onMacNow, pressMatches, sameCombo, specOf, type Press } from './keys'
import { keepRebound, rebound } from './rebound'

/*
 * Every key the window answers to, in one list.
 *
 * Announced in one file and handled in another is how ⌘P came to be printed
 * on a button that did nothing. Listeners, titles, menus, the cheat sheet and
 * Settings all read this list, and the keys a person records replace the
 * defaults here, so none of them can drift from the others.
 */

export type Scope = 'global' | 'board' | 'terminal' | 'chat' | 'file' | 'browser'

/** What must hold, besides the scope, for a key to act. */
export type When = 'composerFocus' | 'terminalFocus' | 'turnRunning'

/* What `when` says, read from the catalog when it is said. */
const whenWord = (need: When): string => t(`workspace.keyboard.when.${need}`)

interface PerPlatform {
  readonly mac: readonly string[]
  readonly other: readonly string[]
}

interface Definition {
  readonly id: string
  readonly scope: Scope
  /** `Mod` is ⌘ on a Mac and Ctrl elsewhere; a platform may have keys of its own. */
  readonly keys: readonly string[] | PerPlatform
  /** Fires while a terminal has the keyboard. Otherwise the key is the program's. */
  readonly inTerminal: boolean
  /** Listed, never rebound: what a focused control does with Enter or Escape. */
  readonly fixed?: true
  readonly when?: readonly When[]
}

const LIST = [
  { id: 'palette', scope: 'global', keys: ['Mod+K'], inTerminal: false },
  { id: 'project', scope: 'global', keys: ['Mod+P'], inTerminal: false },
  { id: 'terminal', scope: 'global', keys: ['Mod+T'], inTerminal: false },
  { id: 'chat', scope: 'global', keys: ['Mod+N'], inTerminal: false },
  { id: 'settings', scope: 'global', keys: ['Mod+,'], inTerminal: true },
  { id: 'next', scope: 'global', keys: ['Mod+Shift+N'], inTerminal: true },
  { id: 'focus', scope: 'global', keys: ['Mod+Shift+F'], inTerminal: true },
  { id: 'shortcuts', scope: 'global', keys: ['Mod+/', '?'], inTerminal: true },
  { id: 'rename', scope: 'global', keys: ['F2'], inTerminal: false },
  { id: 'goToTab', scope: 'global', keys: ['Mod+1', 'Mod+2', 'Mod+3', 'Mod+4', 'Mod+5', 'Mod+6', 'Mod+7', 'Mod+8', 'Mod+9'], inTerminal: false, fixed: true },
  { id: 'reopenTab', scope: 'global', keys: ['Mod+Shift+T'], inTerminal: false },

  { id: 'openCard', scope: 'board', keys: ['Enter', 'Space'], inTerminal: false, fixed: true },
  { id: 'moveCard', scope: 'board', keys: ['Mod+M'], inTerminal: false },
  { id: 'archiveCard', scope: 'board', keys: ['Delete'], inTerminal: false },
  { id: 'comment', scope: 'board', keys: ['Mod+Enter'], inTerminal: false },

  { id: 'splitRight', scope: 'terminal', keys: ['Mod+Shift+D'], inTerminal: true },
  { id: 'splitDown', scope: 'terminal', keys: ['Mod+Shift+E'], inTerminal: true },
  /* Plain Ctrl+C and Ctrl+V stay the program's off a Mac: interrupt, and
     Claude Code's own paste-an-image. */
  { id: 'copy', scope: 'terminal', keys: { mac: ['Mod+C'], other: ['Ctrl+Shift+C', 'Ctrl+Insert'] }, inTerminal: true },
  { id: 'paste', scope: 'terminal', keys: { mac: ['Mod+V'], other: ['Ctrl+Shift+V', 'Shift+Insert'] }, inTerminal: true },
  /* Ctrl on a Mac too, as in Warp, whose key this is. */
  { id: 'compose', scope: 'terminal', keys: ['Ctrl+G'], inTerminal: true },
  { id: 'previousCommand', scope: 'terminal', keys: ['Alt+ArrowUp'], inTerminal: true, when: ['terminalFocus'] },
  { id: 'nextCommand', scope: 'terminal', keys: ['Alt+ArrowDown'], inTerminal: true, when: ['terminalFocus'] },
  /* The command line under a terminal answers to the shell's own keys. */
  { id: 'clearBlocks', scope: 'terminal', keys: ['Ctrl+L'], inTerminal: false },
  { id: 'searchHistory', scope: 'terminal', keys: ['Ctrl+R'], inTerminal: false },
  { id: 'clearLine', scope: 'terminal', keys: ['Ctrl+C'], inTerminal: false },

  { id: 'attach', scope: 'chat', keys: ['Mod+Shift+A'], inTerminal: false },
  /* Held, not pressed: it records until the key, or one held with it, is let go. */
  { id: 'talk', scope: 'chat', keys: ['Mod+Shift+Space'], inTerminal: false },
  { id: 'stopTurn', scope: 'chat', keys: ['Escape'], inTerminal: false, fixed: true, when: ['turnRunning'] },
  { id: 'findChat', scope: 'chat', keys: ['Mod+F'], inTerminal: false },
  { id: 'steerNow', scope: 'chat', keys: ['Mod+Enter'], inTerminal: false, when: ['composerFocus'] },
  { id: 'steerQueued', scope: 'chat', keys: ['Mod+Shift+Enter'], inTerminal: false, when: ['composerFocus', 'turnRunning'] },
  { id: 'stashDraft', scope: 'chat', keys: ['Mod+Shift+S'], inTerminal: false, when: ['composerFocus'] },
  { id: 'sendBackground', scope: 'chat', keys: ['Mod+Alt+Enter'], inTerminal: false, when: ['composerFocus'] },
  { id: 'recallPrompt', scope: 'chat', keys: ['ArrowUp'], inTerminal: false, fixed: true, when: ['composerFocus'] },
  { id: 'recallNext', scope: 'chat', keys: ['ArrowDown'], inTerminal: false, fixed: true, when: ['composerFocus'] },
  { id: 'nextMessage', scope: 'chat', keys: ['Alt+ArrowDown'], inTerminal: false },
  { id: 'previousMessage', scope: 'chat', keys: ['Alt+ArrowUp'], inTerminal: false },
  { id: 'nextFailed', scope: 'chat', keys: ['Alt+Shift+ArrowDown'], inTerminal: false },
  { id: 'toggleRow', scope: 'chat', keys: ['Alt+Enter'], inTerminal: false },

  { id: 'save', scope: 'file', keys: ['Mod+S'], inTerminal: false },
  { id: 'commit', scope: 'file', keys: ['Mod+Enter'], inTerminal: false },

  { id: 'find', scope: 'browser', keys: ['Mod+F'], inTerminal: false },
] as const satisfies readonly Definition[]

export type ShortcutId = (typeof LIST)[number]['id']
export type Shortcut = Definition & { readonly id: ShortcutId }

export const SHORTCUT_LIST: readonly Shortcut[] = LIST

/* What it does, as the cheat sheet and Settings say it — a key in the catalog,
   so the words follow the language the person chose. */
export const doesOf = (one: Pick<Shortcut, 'id'>): string => t(`workspace.shortcut.${one.id}`)

/** When it acts, past its scope: "When the composer has focus.", or null. */
export const whenOf = (one: Shortcut): string | null =>
  one.when?.length
    ? t('workspace.keyboard.whenSaid', { needs: one.when.map(whenWord).join(t('workspace.keyboard.and')) })
    : null

export const SCOPES: readonly { readonly scope: Scope }[] = [
  { scope: 'global' },
  { scope: 'board' },
  { scope: 'terminal' },
  { scope: 'chat' },
  { scope: 'file' },
  { scope: 'browser' },
]

/** What a scope is called on the screen. */
export const scopeTitle = (scope: Scope): string => t(`workspace.keyboard.scope.${scope}`)

const byId = new Map<string, Shortcut>(SHORTCUT_LIST.map((one) => [one.id, one]))
export const shortcutNamed = (id: ShortcutId): Shortcut => byId.get(id)!

const perPlatform = (keys: Definition['keys']): keys is PerPlatform => 'mac' in keys

export function defaultKeys(id: ShortcutId, onMac = onMacNow()): readonly string[] {
  const { keys } = shortcutNamed(id)
  return perPlatform(keys) ? (onMac ? keys.mac : keys.other) : keys
}

/** The keys it answers to now: the ones recorded for it, or its defaults. */
export function bindingsOf(id: ShortcutId, onMac = onMacNow()): readonly string[] {
  const chosen = shortcutNamed(id).fixed ? undefined : rebound()[id]
  return chosen && chosen.length > 0 ? chosen : defaultKeys(id, onMac)
}

export const isRebound = (id: ShortcutId): boolean => !shortcutNamed(id).fixed && Boolean(rebound()[id]?.length)

/** Whether a press is one of the keys `id` answers to. */
export const matches = (id: ShortcutId, press: Press, onMac = onMacNow()): boolean =>
  bindingsOf(id, onMac).some((spec) => pressMatches(press, comboOf(spec, onMac)))

export const keysLabels = (id: ShortcutId, onMac = onMacNow()): string[] => bindingsOf(id, onMac).map((spec) => labelOf(spec, onMac))

/** The first of its keys, as a title prints it. */
export const keysLabel = (id: ShortcutId, onMac = onMacNow()): string => keysLabels(id, onMac)[0] ?? ''

/** Where a press happened: what decides whether a key is text, the program's, or the window's. */
export type Where = 'terminal' | 'field' | 'elsewhere'

/** Whether it fires inside a terminal: allowed there, or ⌘, which no program receives. */
export const firesInTerminal = (one: Shortcut, onMac = onMacNow()): boolean =>
  one.inTerminal || (onMac && bindingsOf(one.id, onMac).every((spec) => comboOf(spec, onMac).meta))

/** The typing rule and the terminal rule, for one key of one shortcut. */
export function permitted(one: Shortcut, spec: string, where: Where, onMac = onMacNow()): boolean {
  const combo = comboOf(spec, onMac)
  if (where !== 'elsewhere' && bare(combo)) return false
  if (where === 'terminal' && !one.inTerminal && !(onMac && combo.meta)) return false
  return true
}

/** The shortcuts a press fires here, in the list's order. */
export function firing(press: Press, where: Where, onMac = onMacNow()): ShortcutId[] {
  return SHORTCUT_LIST.filter((one) =>
    bindingsOf(one.id, onMac).some((spec) => pressMatches(press, comboOf(spec, onMac)) && permitted(one, spec, where, onMac)),
  ).map((one) => one.id)
}

/* Two scopes can be live at once when one of them is everywhere. */
const overlaps = (left: Scope, right: Scope): boolean => left === right || left === 'global' || right === 'global'

/** The shortcut that already answers to `spec` where `id` would, if any. */
export function clashWith(id: ShortcutId, spec: string, onMac = onMacNow()): Shortcut | null {
  const own = shortcutNamed(id)
  const wanted = comboOf(spec, onMac)
  return (
    SHORTCUT_LIST.find(
      (other) =>
        other.id !== id &&
        overlaps(own.scope, other.scope) &&
        bindingsOf(other.id, onMac).some((one) => sameCombo(comboOf(one, onMac), wanted)),
    ) ?? null
  )
}

/* Keys that walk the window: taking one bare would take it from every button. */
const MOVING = new Set(['Enter', 'Space', 'Tab', 'Escape', 'Backspace', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown'])

/** A press recorded for `id`: kept, refused with the reason, or null while only modifiers are down. */
export type Recorded = { readonly kept: string } | { readonly refused: string } | null

export function recordKeys(id: ShortcutId, press: Press, onMac = onMacNow()): Recorded {
  const spec = specOf(press, onMac)
  if (!spec || shortcutNamed(id).fixed) return null
  const said = labelOf(spec, onMac)
  if (bare(comboOf(spec, onMac)) && MOVING.has(comboOf(spec, onMac).key)) {
    return { refused: t('workspace.keyboard.movesAround', { key: said, mods: onMac ? t('workspace.keyboard.modsMac') : t('workspace.keyboard.mods') }) }
  }
  const other = clashWith(id, spec, onMac)
  if (other) return { refused: t('workspace.keyboard.taken', { key: said, does: doesOf(other) }) }
  keepRebound({ ...rebound(), [id]: [spec] })
  return { kept: spec }
}

/** Back to its defaults, unless another shortcut has taken one of them since. */
export function resetKeys(id: ShortcutId, onMac = onMacNow()): string | null {
  for (const spec of defaultKeys(id, onMac)) {
    const other = clashWith(id, spec, onMac)
    if (other) return t('workspace.keyboard.nowIs', { key: labelOf(spec, onMac), does: doesOf(other) })
  }
  keepRebound(Object.fromEntries(Object.entries(rebound()).filter(([key]) => key !== id)))
  return null
}

export const resetAllKeys = (): void => keepRebound({})

const MODIFIERS: Readonly<Record<string, 'ctrl' | 'meta' | 'alt' | 'shift'>> = { Control: 'ctrl', Meta: 'meta', Alt: 'alt', Shift: 'shift' }

/** Whether letting go of a key ends a hold of `id`: its own key, or a modifier it is held with. */
export function letsGo(id: ShortcutId, release: Pick<Press, 'key' | 'code'>, onMac = onMacNow()): boolean {
  const modifier = MODIFIERS[release.key]
  return bindingsOf(id, onMac).some((spec) => {
    const combo = comboOf(spec, onMac)
    return modifier ? combo[modifier] : keyOf(release) === combo.key
  })
}
