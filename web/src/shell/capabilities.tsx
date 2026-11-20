import type { PluginManifest, PluginState, Surface } from '../gen/bindings'
import { DRAWINGS, PLUGIN_ID as EXCALIDRAW } from '../plugins/excalidraw/drawings'
import { NOTES, PLUGIN_ID as NOTES_ID } from '../plugins/notes/noteFiles'
import type { PaneName } from './paneList'
import type { Tab } from './strip'

/*
 * A plugin, as the person reads it: a capability.
 *
 * The contract keeps the word plugin and everything drawn says capability.
 * This is where the two meet: how one looks, what it adds, where it opens.
 */

/** Where a capability's main surface opens: a pane kind, and the tab it lands on. */
export interface Opens {
  readonly kind: PaneName
  readonly tab?: Partial<Tab>
}

/* Only a capability listed here gets a sidebar item and an Open action. */
const OPENS: Readonly<Record<string, Opens>> = {
  [EXCALIDRAW]: { kind: 'drawing', tab: DRAWINGS },
  [NOTES_ID]: { kind: 'note', tab: NOTES },
}

export const opensFor = (pluginId: string): Opens | undefined => OPENS[pluginId]

/** The capability a pane kind belongs to; undefined for the app's own panes. */
export function ownerOf(kind: PaneName): string | undefined {
  return Object.entries(OPENS).find(([, opens]) => opens.kind === kind)?.[0]
}

/** How a pane kind is reached, when it belongs to a capability. */
export const openerOf = (kind: PaneName): Opens | undefined => Object.values(OPENS).find((opens) => opens.kind === kind)

/** A capability's pane is called what the person turned on, not what the pane holds. */
export function capabilityName(kind: PaneName, plugins: readonly PluginState[] | null): string | undefined {
  const owner = ownerOf(kind)
  return owner === undefined ? undefined : plugins?.find((one) => one.manifest.id === owner)?.manifest.name
}

/* Keyed by the contract's surface types, so a new surface cannot ship without words. */
const ADDS: Readonly<Record<Surface['type'], string>> = {
  pane: 'Opens beside your work',
  cardPin: 'Pins to cards',
}

/** What switching a capability on adds, once per kind of surface its manifest declares. */
export function addsInWords(manifest: PluginManifest): string[] {
  return [...new Set(manifest.surfaces.map((surface) => ADDS[surface.type]))]
}

/** On means installed and switched on: a switch left on by an uninstall is not on. */
export const isOn = (plugin: PluginState): boolean => plugin.installed && plugin.enabled

export interface Live {
  readonly manifest: PluginManifest
  readonly opens: Opens
}

/** The capabilities that are on and have somewhere to open, in catalogue order. */
export function liveCapabilities(plugins: readonly PluginState[] | null): Live[] {
  return (plugins ?? []).flatMap((one) => {
    const opens = opensFor(one.manifest.id)
    return isOn(one) && opens ? [{ manifest: one.manifest, opens }] : []
  })
}

/** Null rather than zero when nothing was read, for the same reason as `counted`. */
export function enabledCount(plugins: readonly PluginState[] | null): number | null {
  return plugins === null ? null : plugins.filter(isOn).length
}

/** "1 installed · 1 on"; null until the catalogue is read, so nothing claims zero. */
/** Which capabilities the pane shows. */
export type CapabilityFilter = 'all' | 'on' | 'installed' | 'available'

/** Each tab names itself through the catalog, read when it is drawn. */
export const CAPABILITY_FILTERS: readonly { readonly id: CapabilityFilter; readonly labelKey: string }[] = [
  { id: 'all', labelKey: 'workspace.plugins.filterAll' },
  { id: 'on', labelKey: 'workspace.plugins.filterOn' },
  { id: 'installed', labelKey: 'workspace.plugins.filterInstalled' },
  { id: 'available', labelKey: 'workspace.plugins.filterAvailable' },
]

/** The capabilities a search and a filter keep: the query is looked for in the name and the description, in any case. */
export function shownCapabilities(
  plugins: readonly PluginState[],
  query: string,
  filter: CapabilityFilter,
): readonly PluginState[] {
  const wanted = query.trim().toLowerCase()
  return plugins.filter((plugin) => {
    const kept =
      filter === 'all' ||
      (filter === 'on' && plugin.installed && plugin.enabled) ||
      (filter === 'installed' && plugin.installed) ||
      (filter === 'available' && !plugin.installed)
    const text = `${plugin.manifest.name} ${plugin.manifest.description}`.toLowerCase()
    return kept && (wanted === '' || text.includes(wanted))
  })
}

export function summary(plugins: readonly PluginState[] | null): string | null {
  if (plugins === null) return null
  return `${plugins.filter((one) => one.installed).length} installed · ${enabledCount(plugins)} on`
}

/* The same marks the tabs carry, so a capability looks like itself in both
   places. One this build has no glyph for falls back to a block, which is a
   placeholder and reads as one — the sidebar showed three of them. */
const GLYPHS: Readonly<Record<string, React.JSX.Element>> = {
  [EXCALIDRAW]: <path d="M12 19l7-7 3 3-7 7-3-3ZM18 13l-1.5-7.5L2 2l3.5 14.5L13 18ZM2 2l7.6 7.6" />,
  [NOTES_ID]: (
    <>
      <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9Z" />
      <path d="M14 3v6h6M8 13h8M8 17h5" />
    </>
  ),
}

/** A capability's mark. One this build has no glyph for is drawn as a block. */
export function CapabilityGlyph({ id, size }: { id: string; size: number }): React.JSX.Element {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {GLYPHS[id] ?? <rect x="4" y="4" width="16" height="16" rx="3" />}
    </svg>
  )
}

/** The mark of Capabilities itself, as in the prototype. */
export const CAPABILITIES_ICON = (size: number): React.JSX.Element => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M12 2 4 6v6c0 5 3.4 9.1 8 10 4.6-.9 8-5 8-10V6Z" />
    <path d="m9 12 2 2 4-4" />
  </svg>
)
