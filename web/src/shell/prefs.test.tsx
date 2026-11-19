import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { OpenApp, Project } from '../gen/bindings'
import type { PrefsPane } from './shape'
import { PrefsSide, rowFor } from './PrefsSide'
import { matching, prefsNav } from './prefsNav'
import { OpenIn } from './OpenIn'
import { ProjectRows } from './ProjectRows'
import { RemoveProject } from './RemoveProject'

afterEach(cleanup)

const found = (query: string): PrefsPane[] => matching(query).flatMap((group) => group.items.map((item) => item.id))

const reveal = vi.fn()
const opened = vi.fn()
let installed: OpenApp[] = []

vi.mock('./live', () => ({
  ask: (call: () => unknown) => Promise.resolve({ data: call(), error: null, loading: false }),
  commands: {
    pathReveal: (path: string) => reveal(path),
    appsList: () => installed,
    appsOpen: (id: string, path: string) => opened(id, path),
  },
}))

const project = (over: Partial<Project> = {}): Project => ({
  id: 'p1',
  name: 'devpit',
  rootPath: '/home/me/devpit',
  group: null,
  accent: '#6f8fbf',
  worktrees: [],
  unreadable: null,
    orchestrator: null,
  origin: null,
  lastOpenedAt: null,
  icon: null,
  color: null,
  ...over,
})

const shell = {
  projects: [project()] as Project[],
  project: null as Project | null,
  projectsError: null as string | null,
  setProject: vi.fn(),
  renameProject: vi.fn(() => Promise.resolve(null)),
}

vi.mock('./useShell', () => ({ useShell: () => shell }))

describe('the settings sidebar', () => {
  it('puts every pane under exactly one heading', () => {
    /* A pane in two groups is a pane you find twice and change in one of
       them; a pane in none is a pane that is unreachable. */
    const ids = prefsNav().flatMap((group) => group.items.map((item) => item.id))
    expect(new Set(ids).size).toBe(ids.length)
    expect(ids).toHaveLength(25)
  })

  it('draws the headings, and opens the pane you click', () => {
    const onSelect = vi.fn()
    render(<PrefsSide pane="projects" onBack={vi.fn()} onSelect={onSelect} />)
    expect(screen.getByText('Workspace')).toBeTruthy()
    expect(screen.getByText('AI capabilities')).toBeTruthy()
    fireEvent.click(screen.getByText('Skills'))
    expect(onSelect).toHaveBeenCalledWith('skills')
  })

  it('filters the list down to what was typed', () => {
    render(<PrefsSide pane="projects" onBack={vi.fn()} onSelect={vi.fn()} />)
    fireEvent.change(screen.getByLabelText('Search settings'), { target: { value: 'theme' } })
    expect(screen.getByText('Appearance')).toBeTruthy()
    expect(screen.queryByText('Projects')).toBeNull()
    /* A heading with nothing left under it is a heading about nothing. */
    expect(screen.queryByText('Workspace')).toBeNull()
  })

  it('says so when nothing matches, instead of showing an empty column', () => {
    render(<PrefsSide pane="projects" onBack={vi.fn()} onSelect={vi.fn()} />)
    fireEvent.change(screen.getByLabelText('Search settings'), { target: { value: 'zzz' } })
    expect(screen.getByText(/Nothing here matches/)).toBeTruthy()
  })

  it('finds a pane by what it is about, not only by its name', () => {
    /* Nobody looking for the update switch types "General". */
    expect(found('updates')).toEqual(['general'])
    expect(found('tokens')).toEqual(['usage'])
    /* The desktop-wide key is General's; the window's own are Keyboard's. */
    expect(found('shortcut')).toEqual(['general', 'keyboard'])
  })

  it('opens each pane General was split into', () => {
    const onSelect = vi.fn()
    render(<PrefsSide pane="general" onBack={vi.fn()} onSelect={onSelect} />)
    for (const [label, id] of [['Notifications', 'notifications'], ['Remote', 'remote'], ['Sessions', 'sessions'], ['Voice', 'voice']]) {
      fireEvent.click(screen.getByText(label!))
      expect(onSelect).toHaveBeenLastCalledWith(id)
    }
  })

  it('finds a row that left General by its own name', () => {
    expect(found('island')).toEqual(['notifications'])
    expect(found('focus mode')).toEqual(['notifications'])
    expect(found('hand off a full session')).toEqual(['sessions'])
    expect(found('ask before stopping')).toEqual(['sessions'])
    expect(found('voice messages')).toEqual(['voice'])
    expect(found('allow remote mcp')).toEqual(['remote'])
    expect(found('error reports')).toEqual(['general'])
  })

  it('still finds, by every word that found General, the pane that holds its row', () => {
    /* General's words before the split, each with where its row went. */
    const went: Record<string, PrefsPane> = {
      updates: 'general', privacy: 'general', local: 'general', automatic: 'general', login: 'general',
      startup: 'general', tray: 'general', shortcut: 'general', open: 'general',
      island: 'notifications', pause: 'notifications', quiet: 'notifications', reminders: 'notifications', focus: 'notifications',
      terminal: 'sessions', stop: 'sessions',
      voice: 'voice', microphone: 'voice', whisper: 'voice', transcribe: 'voice',
      remote: 'remote', phone: 'remote', tailscale: 'remote', pair: 'remote', devices: 'remote',
      calendar: 'remote', ics: 'remote', google: 'remote', subscribe: 'remote',
    }
    for (const [word, pane] of Object.entries(went)) expect(found(word), word).toContain(pane)
  })

  it('is the whole list again once the field is emptied', () => {
    expect(matching('   ')).toEqual(prefsNav())
  })
})

describe('a project row', () => {
  it('says when it was last opened, which is what the list is ordered by', () => {
    shell.projects = [project({ lastOpenedAt: Date.now() / 1000 - 3 * 3600 })]
    render(<ProjectRows onRemove={vi.fn()} />)
    const worded = new Intl.RelativeTimeFormat('en', { numeric: 'auto' }).format(-3, 'hour')
    expect(screen.getByText(worded)).toBeTruthy()
  })

  it('says so for one that has never been opened', () => {
    shell.projects = [project()]
    render(<ProjectRows onRemove={vi.fn()} />)
    expect(screen.getByText('never opened')).toBeTruthy()
  })

  it('names the remote, so two clones are not two strangers', () => {
    shell.projects = [project({ origin: 'git@github.com:owner/devpit.git' })]
    render(<ProjectRows onRemove={vi.fn()} />)
    expect(screen.getByText('github.com/owner/devpit')).toBeTruthy()
  })

  /* The bug this replaces: a project whose git could not be read showed the
     error *instead of* its path — losing the one thing you need to fix it. */
  it('keeps the path when the repository cannot be read', () => {
    shell.projects = [project({ unreadable: 'not a git repository' })]
    render(<ProjectRows onRemove={vi.fn()} />)
    expect(screen.getByText('/home/me/devpit')).toBeTruthy()
    expect(screen.getByText('not a git repository')).toBeTruthy()
  })

  it('renames through the backend, not only on the screen', () => {
    shell.projects = [project()]
    shell.renameProject.mockClear()
    render(<ProjectRows onRemove={vi.fn()} />)
    fireEvent.click(screen.getByText('Rename'))
    const field = screen.getByRole('textbox')
    fireEvent.change(field, { target: { value: 'API' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(shell.renameProject).toHaveBeenCalledWith('p1', 'API')
  })

  it('hands the folder to the desktop', () => {
    shell.projects = [project()]
    reveal.mockClear()
    render(<ProjectRows onRemove={vi.fn()} />)
    fireEvent.click(screen.getByText('Reveal'))
    expect(reveal).toHaveBeenCalledWith('/home/me/devpit')
  })
})

const app = (over: Partial<OpenApp> = {}): OpenApp => ({
  id: 'vscode',
  label: 'VS Code',
  command: 'code',
  installed: true,
  ...over,
})

describe('opening a folder somewhere else', () => {
  it('is absent when nothing is configured', () => {
    /* A button that opens an empty menu reads as broken, not as unused. */
    const { container } = render(<OpenIn apps={[]} path="/p" />)
    expect(container.innerHTML).toBe('')
  })

  it('is absent when the only app is not on this machine', () => {
    const { container } = render(<OpenIn apps={[app({ installed: false })]} path="/p" />)
    expect(container.innerHTML).toBe('')
  })

  /* One app is not a menu. It is that app. */
  it('opens straight away when there is only one', () => {
    opened.mockClear()
    render(<OpenIn apps={[app()]} path="/home/me/devpit" />)
    fireEvent.click(screen.getByText(/Open in VS Code/))
    expect(opened).toHaveBeenCalledWith('vscode', '/home/me/devpit')
  })

  it('offers the usable ones, and only those, when there are several', () => {
    opened.mockClear()
    render(
      <OpenIn
        apps={[app(), app({ id: 'zed', label: 'Zed', command: 'zed' }), app({ id: 'x', label: 'Gone', installed: false })]}
        path="/home/me/devpit"
      />,
    )
    fireEvent.click(screen.getByText('Open in'))
    expect(screen.queryByText('Gone')).toBeNull()
    fireEvent.click(screen.getByText('Zed'))
    expect(opened).toHaveBeenCalledWith('zed', '/home/me/devpit')
  })
})

describe('removing a project', () => {
  /* The box said the board and the settings would go, and nothing went: the
     dialog never told anyone whether it had been ticked. */
  it('reports whether the workspace was to be deleted', () => {
    shell.projects = [project()]
    const onConfirm = vi.fn()
    render(<RemoveProject project="p1" onClose={vi.fn()} onConfirm={onConfirm} />)

    fireEvent.click(screen.getByText('Remove'))
    expect(onConfirm).toHaveBeenLastCalledWith(false)

    fireEvent.click(screen.getByText(/Also delete/))
    fireEvent.click(screen.getByText('Remove and delete'))
    expect(onConfirm).toHaveBeenLastCalledWith(true)
  })
})

describe('a search that lands on its row', () => {
  const pane = (): HTMLElement => {
    const root = document.createElement('div')
    root.innerHTML = `
      <section class="prefs__in">
        <div class="pref"><span class="pref__t">Island</span><span class="pref__d">Quiet while you speak</span></div>
        <div class="pref"><span class="pref__t">Voice messages</span><details><p>a whisper on this machine</p></details></div>
      </section>
      <section class="prefs__in" hidden><div class="pref"><span class="pref__t">Speak hidden</span></div></section>`
    return root
  }

  it('prefers the row titled with the words over one that only mentions them', () => {
    expect(rowFor('voice', pane())?.textContent).toContain('Voice messages')
    expect(rowFor('speak', pane())?.textContent).toContain('Island')
  })

  it('finds words folded into a row, and nothing in a pane that is not shown', () => {
    expect(rowFor('whisper', pane())?.textContent).toContain('Voice messages')
    expect(rowFor('hidden', pane())).toBeNull()
    expect(rowFor('  ', pane())).toBeNull()
  })
})
