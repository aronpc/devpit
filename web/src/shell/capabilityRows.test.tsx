import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { PluginList, PluginManifest } from '../gen/bindings'
import { DRAWINGS } from '../plugins/excalidraw/drawings'
import { PluginsPane } from './PluginsPane'
import { Sidebar } from './Sidebar'
import { PluginsProvider } from './usePlugins'

afterEach(cleanup)

/* The rest of the column is tested where it lives. */
vi.mock('./SessionRows', () => ({ SessionRows: () => null }))
vi.mock('./Threads', () => ({ Threads: () => null }))
vi.mock('./useKit', () => ({ useKit: () => ({ skills: 4, servers: 2 }) }))

const excalidraw: PluginManifest = {
  id: 'excalidraw',
  name: 'Excalidraw',
  version: '0.18.1',
  description: '',
  surfaces: [{ type: 'pane', many: true }, { type: 'cardPin' }],
  data: { extensions: ['.excalidraw'], maxBytes: 33554432 },
  permissions: ['dataOwn'],
}

let catalogue: PluginList = { plugins: [] }
const listed = vi.fn()

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve(call()).then((data) => ({ data, error: null, loading: false })),
  commands: {
    pluginList: () => {
      listed()
      return catalogue
    },
    pluginDataList: () => ({ files: [] }),
    maintenanceRead: () => ({ projectAgent: true }),
    pluginUninstall: () => {
      catalogue = { plugins: catalogue.plugins.map((one) => ({ ...one, installed: false, enabled: false })) }
      return { ...catalogue, removedFiles: 0 }
    },
  },
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
  listed.mockClear()
  catalogue = { plugins: [] }
})

const sidebar = () =>
  render(
    <PluginsProvider>
      <Sidebar onSearch={vi.fn()} onSignIn={vi.fn()} />
    </PluginsProvider>,
  )

/* A top-level item, never one inside a popover menu. */
const item = (label: string): HTMLElement | null => {
  const button = screen.queryByText(label, { selector: '.act__label' })?.closest('button') ?? null
  return button?.closest('[role="menu"]') ? null : button
}

describe('Capabilities in the sidebar', () => {
  it('is always there, even with nothing on, and opens the Capabilities pane', async () => {
    sidebar()
    await waitFor(() => expect(listed).toHaveBeenCalled())
    fireEvent.click(item('Capabilities')!)
    expect(shell.show).toHaveBeenCalledWith('plugins')
  })

  it('sits after Files, with the capabilities that are on under it, and Resources last', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: true, enabled: true }] }
    sidebar()
    await waitFor(() => expect(item('Excalidraw')).not.toBeNull())
    const nav = document.querySelectorAll('.side__scroll > .act > .act__label, .side__scroll > .newmenu > .act > .act__label')
    expect([...nav].map((label) => label.textContent)).toEqual([
      'Board',
      'Devflow',
      'Ask the project',
      'Project agent',
      'Files',
      'Capabilities',
      'Excalidraw',
      'Resources',
    ])
  })

  /* The runs are the board's, and the board is where they are asked for:
     a second door in the sidebar is what this test exists to keep shut. */
  it('does not carry Checks: the runs open from the board itself', async () => {
    sidebar()
    expect(item('Checks')).toBeNull()
  })

  it('counts the capabilities that are on beside it', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: true, enabled: true }] }
    sidebar()
    await waitFor(() => expect(item('Capabilities')?.querySelector('.act__n')?.textContent).toBe('1 on'))
  })

  it('gives a capability that is on its own item, opening its surface', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: true, enabled: true }] }
    sidebar()
    await waitFor(() => expect(item('Excalidraw')).not.toBeNull())
    fireEvent.click(item('Excalidraw')!)
    expect(shell.show).toHaveBeenCalledWith('drawing', DRAWINGS)
  })

  it('names that item as the manifest does', async () => {
    /* A name nothing in the app spells, so the item can only have read it. */
    catalogue = { plugins: [{ manifest: { ...excalidraw, name: 'Whiteboard' }, installed: true, enabled: true }] }
    sidebar()
    await waitFor(() => expect(item('Whiteboard')).not.toBeNull())
    expect(item('Excalidraw')).toBeNull()
  })

  it('has no item for a capability that is off', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: true, enabled: false }] }
    sidebar()
    await waitFor(() => expect(listed).toHaveBeenCalled())
    await act(async () => {})
    expect(item('Excalidraw')).toBeNull()
    expect(item('Capabilities')).not.toBeNull()
  })

  it('has no item and no count for a capability that is not installed, whatever its switch says', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: false, enabled: true }] }
    sidebar()
    await waitFor(() => expect(listed).toHaveBeenCalled())
    await act(async () => {})
    expect(item('Excalidraw')).toBeNull()
    /* No count at all, as with nothing on: a count of zero is not drawn. */
    expect(item('Capabilities')?.querySelector('.act__n')).toBeNull()
  })

  it('loses the item once the capability is uninstalled from the Capabilities pane', async () => {
    catalogue = { plugins: [{ manifest: excalidraw, installed: true, enabled: true }] }
    render(
      <PluginsProvider>
        <Sidebar onSearch={vi.fn()} onSignIn={vi.fn()} />
        <PluginsPane />
      </PluginsProvider>,
    )
    await waitFor(() => expect(item('Excalidraw')).not.toBeNull())

    fireEvent.click(screen.getByRole('button', { name: 'Uninstall Excalidraw' }))
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Continue' }))
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Uninstall' }))

    await waitFor(() => expect(item('Excalidraw')).toBeNull())
    expect(item('Capabilities')?.querySelector('.act__n')).toBeNull()
  })
})
