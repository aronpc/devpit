import { afterEach, describe, expect, it } from 'vitest'

import type { PluginManifest } from '../gen/bindings'
import { setLocale, t } from '../i18n'
import {
  CAPABILITY_FILTERS,
  addsInWords,
  capabilityName,
  enabledCount,
  isOn,
  liveCapabilities,
  openerOf,
  shownCapabilities,
  summary,
} from './capabilities'
import { uninstalledInWords, uninstallLabel } from './CapabilityDialogs'

const excalidraw: PluginManifest = {
  id: 'excalidraw',
  name: 'Excalidraw',
  version: '0.18.1',
  description: '',
  surfaces: [{ type: 'pane', many: true }, { type: 'cardPin' }],
  data: { extensions: ['.excalidraw'], maxBytes: 33554432 },
  permissions: ['dataOwn'],
}

describe('addsInWords', () => {
  it('says what each surface adds, in words a person uses', () => {
    expect(addsInWords(excalidraw)).toEqual(['Opens beside your work', 'Pins to cards'])
    expect(addsInWords({ ...excalidraw, surfaces: [{ type: 'cardPin' }] })).toEqual(['Pins to cards'])
  })

  it('never says pane, and says a surface once however often it is declared', () => {
    const twice = { ...excalidraw, surfaces: [{ type: 'pane', many: true }, { type: 'pane', many: false }] } as PluginManifest
    expect(addsInWords(twice)).toEqual(['Opens beside your work'])
    expect(addsInWords(excalidraw).join(' ')).not.toMatch(/\bpane\b/i)
  })
})

describe('isOn', () => {
  it('needs both installed and switched on', () => {
    expect(isOn({ manifest: excalidraw, installed: true, enabled: true })).toBe(true)
    expect(isOn({ manifest: excalidraw, installed: true, enabled: false })).toBe(false)
    expect(isOn({ manifest: excalidraw, installed: false, enabled: true })).toBe(false)
  })
})

describe('summary', () => {
  it('says nothing before the catalogue is read', () => {
    expect(summary(null)).toBeNull()
  })

  it('says how many are installed and how many are on', () => {
    expect(summary([])).toBe('0 installed · 0 on')
    expect(
      summary([
        { manifest: excalidraw, installed: true, enabled: true },
        { manifest: { ...excalidraw, id: 'notes' }, installed: true, enabled: false },
        { manifest: { ...excalidraw, id: 'sheets' }, installed: false, enabled: true },
      ]),
    ).toBe('2 installed · 1 on')
  })
})

describe('enabledCount', () => {
  it('says nothing before the catalogue is read', () => {
    expect(enabledCount(null)).toBeNull()
  })

  it('counts only the plugins that are installed and on', () => {
    expect(enabledCount([])).toBe(0)
    expect(enabledCount([{ manifest: excalidraw, installed: true, enabled: false }])).toBe(0)
    /* A switch left on is not on once it is uninstalled. */
    expect(enabledCount([{ manifest: excalidraw, installed: false, enabled: true }])).toBe(0)
    expect(
      enabledCount([
        { manifest: excalidraw, installed: true, enabled: true },
        { manifest: { ...excalidraw, id: 'notes' }, installed: true, enabled: false },
      ]),
    ).toBe(1)
  })
})

describe('where a capability opens', () => {
  it('lists only what is installed, on, and has somewhere to open', () => {
    /* An id no OPENS entry names: `notes` used to stand for this and now has
       a pane of its own, which is the point — this asks about a capability
       with nowhere to open, not about any particular one. */
    const nowhere = { ...excalidraw, id: 'nowhere', name: 'Nowhere' }
    const live = liveCapabilities([
      { manifest: excalidraw, installed: true, enabled: true },
      { manifest: nowhere, installed: true, enabled: true },
    ])
    expect(live.map((one) => one.manifest.id)).toEqual(['excalidraw'])
    expect(liveCapabilities([{ manifest: excalidraw, installed: true, enabled: false }])).toEqual([])
    expect(liveCapabilities([{ manifest: excalidraw, installed: false, enabled: true }])).toEqual([])
  })

  it('names a pane after the capability that owns it, and leaves the app’s own panes alone', () => {
    const plugins = [{ manifest: { ...excalidraw, name: 'Whiteboard' }, installed: true, enabled: true }]
    expect(capabilityName('drawing', plugins)).toBe('Whiteboard')
    expect(capabilityName('board', plugins)).toBeUndefined()
    expect(openerOf('drawing')?.kind).toBe('drawing')
    expect(openerOf('board')).toBeUndefined()
  })
})

describe('the words around uninstalling', () => {
  it('puts the deletion on the final button only when the box is ticked', () => {
    expect(uninstallLabel(false, 3)).toBe('Uninstall')
    expect(uninstallLabel(true, 3)).toBe('Uninstall and delete 3 files')
    expect(uninstallLabel(true, 1)).toBe('Uninstall and delete 1 file')
  })

  it('never names a count it does not have', () => {
    expect(uninstallLabel(true, null)).toBe('Uninstall and delete its data')
    expect(uninstallLabel(true, 0)).toBe('Uninstall and delete its data')
  })

  it('reports the backend’s count once the files are deleted, and that they stay otherwise', () => {
    expect(uninstalledInWords('Excalidraw', true, 2)).toBe('Excalidraw uninstalled and 2 files deleted.')
    expect(uninstalledInWords('Excalidraw', false, 0)).toMatch(/files stay/)
  })
})

describe('shownCapabilities', () => {
  const plugin = (id: string, name: string, description: string, installed: boolean, enabled: boolean) => ({
    manifest: { ...excalidraw, id, name, description },
    installed,
    enabled,
  })
  const catalogue = [
    plugin('excalidraw', 'Excalidraw', 'Hand-drawn diagrams', true, true),
    plugin('mermaid', 'Mermaid', 'Diagrams from text', true, false),
    plugin('notes', 'Notes', 'Plain notes', false, false),
  ]
  const ids = (kept: readonly { manifest: PluginManifest }[]) => kept.map((one) => one.manifest.id)

  it('keeps what the filter asks for', () => {
    expect(ids(shownCapabilities(catalogue, '', 'all'))).toEqual(['excalidraw', 'mermaid', 'notes'])
    expect(ids(shownCapabilities(catalogue, '', 'on'))).toEqual(['excalidraw'])
    expect(ids(shownCapabilities(catalogue, '', 'installed'))).toEqual(['excalidraw', 'mermaid'])
    expect(ids(shownCapabilities(catalogue, '', 'available'))).toEqual(['notes'])
  })

  it('searches the name and the description, in any case, within the filter', () => {
    expect(ids(shownCapabilities(catalogue, '  DIAGRAMS ', 'all'))).toEqual(['excalidraw', 'mermaid'])
    expect(ids(shownCapabilities(catalogue, 'diagrams', 'on'))).toEqual(['excalidraw'])
    expect(ids(shownCapabilities(catalogue, 'nothing', 'all'))).toEqual([])
  })
})

/* The filter tabs name themselves in the language the person chose. */
describe('the capability filters', () => {
  afterEach(() => setLocale('en'))

  it('read in pt-BR when pt-BR is chosen', () => {
    setLocale('pt-BR')
    expect(CAPABILITY_FILTERS.map((one) => t(one.labelKey))).toEqual(['Todas', 'Ativas', 'Instaladas', 'Não instaladas'])
  })

  it('read in English when English is chosen', () => {
    expect(CAPABILITY_FILTERS.map((one) => t(one.labelKey))).toEqual(['All', 'On', 'Installed', 'Not installed'])
  })
})
