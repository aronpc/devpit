import enWorkspace from './en/workspace.json'
import ptBrWorkspace from './pt-BR/workspace.json'
import enCommon from './en/common.json'
import enIsland from './en/island.json'
import enRemote from './en/remote.json'
import enBoard from './en/board.json'
import enSettings from './en/settings.json'
import enDevflow from './en/devflow.json'
import enSessions from './en/sessions.json'
import ptBrCommon from './pt-BR/common.json'
import ptBrIsland from './pt-BR/island.json'
import ptBrRemote from './pt-BR/remote.json'
import ptBrBoard from './pt-BR/board.json'
import ptBrSettings from './pt-BR/settings.json'
import ptBrDevflow from './pt-BR/devflow.json'
import ptBrSessions from './pt-BR/sessions.json'

/* One entry is a sentence, or the two plural forms of one. Flat keys —
   `board.emptyLane` — because the guard that compares catalogs (T7) reads
   them with serde_json, and a flat key is one string on both sides. */
type Entry = string | { one: string; other: string; [form: string]: string }
type Catalog = Record<string, Entry>

/* English is the base: every key is born in its catalog (AGENTS.md). */
const catalogs: Record<string, Catalog> = {
  en: { ...enCommon, ...enIsland, ...enRemote, ...enSettings, ...enBoard, ...enDevflow, ...enSessions, ...enWorkspace },
  'pt-BR': { ...ptBrCommon, ...ptBrIsland, ...ptBrRemote, ...ptBrSettings, ...ptBrBoard, ...ptBrDevflow, ...ptBrSessions, ...ptBrWorkspace },
}

export interface TranslateArgs {
  count?: number
  [name: string]: string | number | undefined
}

let locale = 'en'
const listeners = new Set<() => void>()

/** The locale the core resolved for this person: `en` or `pt-BR`. */
export function currentLocale(): string {
  return locale
}

/** Takes the resolved locale and tells whoever renders in it. */
export function setLocale(next: string): void {
  if (next === locale) return
  locale = next
  for (const hear of [...listeners]) hear()
}

export function subscribe(hear: () => void): () => void {
  listeners.add(hear)
  return () => listeners.delete(hear)
}

export function t(key: string, args?: TranslateArgs): string {
  const { entry, from } = find(key)
  if (entry === undefined) return fill(missing(key), args)
  const said =
    typeof entry === 'object'
      ? typeof args?.count === 'number'
        ? entry[pluralOf(args.count, from)] ?? entry.other
        : entry.other
      : entry
  return fill(said, args)
}

/* The entry, and the language it was found in: a sentence that fell back to
   English pluralises in English, not in the language the person chose. */
function find(key: string): { entry: Entry | undefined; from: string } {
  const own = catalogs[locale]
  return own && key in own
    ? { entry: own[key], from: locale }
    : { entry: catalogs.en[key], from: 'en' }
}

/** Which plural form `count` takes in the language the sentence is in. */
function pluralOf(count: number, from: string): string {
  return new Intl.PluralRules(from).select(count)
}

/* `{name}` becomes the arg of that name. A placeholder the sentence does not
   carry is simply not there to fill — pt-BR can carry one English does not. */
function fill(said: string, args: TranslateArgs | undefined): string {
  if (!args) return said
  return said.replace(/\{(\w+)\}/g, (_, name: string) =>
    args[name] === undefined ? '' : String(args[name]),
  )
}

/* A key in no catalog is a bug, said loudly in dev where a bug can be seen.
   The last segment, readably, is said in the meantime: the screen never shows
   a raw dotted key, and never shows nothing. */
function missing(key: string): string {
  if (import.meta.env.DEV) console.warn(`i18n: "${key}" is in no catalog`)
  const last = key.split('.').filter(Boolean).at(-1) ?? key
  /* `emptyLane` reads as `empty lane`; the first word keeps its own case. */
  return last.replace(/([a-z0-9])([A-Z])/g, (_, was: string, capital: string) => `${was} ${capital.toLowerCase()}`)
}
