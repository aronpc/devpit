import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { PluginList, PluginManifest } from '../gen/bindings'
import { Sidebar } from './Sidebar'
import { PluginsProvider } from './usePlugins'

afterEach(cleanup)

/* The rest of the column is tested where it lives. */
vi.mock('./SessionRows', () => ({ SessionRows: () => null }))
vi.mock('./Threads', () => ({ Threads: () => null }))
vi.mock('./useKit', () => ({ useKit: () => ({ skills: 4, servers: 2 }) }))

const manifest = (id: string, name: string): PluginManifest => ({
  id,
  name,
  version: '1.0.0',
  description: '',
  surfaces: [{ type: 'pane', many: true }],
  data: { extensions: [`.${id}`], maxBytes: 1024 },
  permissions: ['dataOwn'],
})

let catalogue: PluginList = { plugins: [] }

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve(call()).then((data) => ({ data, error: null, loading: false })),
  commands: { pluginList: () => catalogue, maintenanceRead: () => ({ projectAgent: true }), lessonsProposed: () => ({ proposals: [], decisions: [] }) },
}))

const shell = {
  project: { id: 'p', name: 'demo' },
  active: null,
  open: [],
  show: vi.fn(),
  close: vi.fn(),
  closeNow: vi.fn(),
  openPrefs: vi.fn(),
  signOut: vi.fn(),
  account: { initials: 'JH', name: 'Jhol', email: null },
  threads: [],
  profiles: [],
  chatDoing: {},
  mark: vi.fn(),
}
vi.mock('./useShell', () => ({ useShell: () => shell }))

beforeEach(() => {
  shell.show.mockClear()
  catalogue = {
    plugins: [
      { manifest: manifest('excalidraw', 'Excalidraw'), installed: true, enabled: true },
      { manifest: manifest('notes', 'Notes'), installed: true, enabled: false },
    ],
  }
})

describe('the Resources menu', () => {
  it('holds skills, servers and the workspace folder, and no capability', async () => {
    render(
      <PluginsProvider>
        <Sidebar onSearch={vi.fn()} onSignIn={vi.fn()} />
      </PluginsProvider>,
    )
    /* Wait for the capability to reach the sidebar, so its absence from the menu means something. */
    await waitFor(() => expect(screen.queryByText('Excalidraw', { selector: '.act__label' })).not.toBeNull())
    const menu = screen.getByText('Resources').closest('.newmenu')!
    const labels = [...menu.querySelectorAll('.newmenu__label')].map((label) => label.textContent)
    expect(labels).toEqual(['Skills', 'MCPs', 'Workspace folder'])
  })
})
