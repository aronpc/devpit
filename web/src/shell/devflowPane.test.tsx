import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { DevflowAsk, DevflowConfig, DevflowDecisions, DevflowScreen, DevflowStage } from '../gen/bindings'

const heard: Record<string, (payload: unknown) => void> = {}
vi.mock('./window', () => ({
  onCarried: (name: string, then: (payload: unknown) => void) => {
    heard[name] = then
    return () => delete heard[name]
  },
}))

const shell = { project: { id: 'prj_api', name: 'api' }, show: vi.fn(), openPrefs: vi.fn(), active: { kind: 'devflow' } }
vi.mock('./useShell', () => ({ useShell: () => shell }))
vi.mock('./useKnownAgents', () => ({
  useKnownAgents: () => [{ id: 'claude', label: 'Claude Code', launch: 'claude', installed: true }],
}))

const stage = (name: string, more: Partial<DevflowStage> = {}): DevflowStage => ({
  name,
  uses: `builtin/${name}`,
  own: false,
  publish: null,
  e2eDir: null,
  command: null,
  enabled: true,
  approve: name === 'briefing' || name === 'plan' ? 'person' : null,
  checks: [],
  gate: name === 'completion' ? 'devflow-done' : null,
  rubric: null,
  skill: { name: `devflow-${name}`, layer: 'built-in', file: null, refused: null },
  ...more,
})

const config = (more: Partial<DevflowConfig> = {}): DevflowConfig => ({
  enabled: false,
  version: 2,
  teams: [],
  locked: [],
  stagesFrom: 'project',
  stages: ['briefing', 'context', 'plan', 'execution', 'review', 'completion'].map((name) =>
    stage(name, name === 'execution' ? { checks: [{ name: 'unit', run: 'make test', report: 'none', evidence: [], source: 'Makefile:1' }] } : {}),
  ),
  reviewers: [{ agent: 'claude', persona: 'devflow-security', model: null }],
  blockOn: ['critical', 'high'],
  consolidate: 'jev',
  decisionsUse: false,
  decisionsMode: 'shadow',
  ...more,
})

const base = (more: Partial<DevflowScreen>): DevflowScreen => ({
  projectId: 'prj_api',
  state: 'missing',
  globallyOn: true,
  file: '/work/api/.devpit/devflow/devflow.yaml',
  yaml: null,
  config: null,
  error: null,
  waiting: [],
  detected: null,
  preset: 'recommended',
  ...more,
})

const missing = (): DevflowScreen =>
  base({
    detected: {
      found: [
        { kind: 'test', command: 'make test', source: 'Makefile:1', unavailable: null },
        { kind: 'lint', command: 'make lint', source: 'Makefile:4', unavailable: '`phpstan` is not installed' },
      ],
      e2eTools: [],
      clis: [{ name: 'claude', installed: true, signedIn: true }],
      decisionsKey: false,
      glossary: 'CONTEXT.md',
      adrs: [],
      open: [],
      hints: [{ label: 'Ticket keys', items: ['LED'] }],
      recommended: config(),
    },
  })
const disabled = (): DevflowScreen => base({ state: 'disabled', yaml: 'version: 1\n# the file\n', config: config() })
const active = (yaml = 'version: 1\nenabled: true\n', more: Partial<DevflowConfig> = {}): DevflowScreen =>
  base({ state: 'active', yaml, config: config({ enabled: true, ...more }) })

let current: DevflowScreen = missing()
let standing: DevflowDecisions | null = null
const called = vi.fn()
const looked = vi.fn()
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => {
    try {
      return { data: await call(), error: null, loading: false }
    } catch (why) {
      return { data: null, error: String(why), loading: false }
    }
  },
  commands: {
    devflowScreen: async () => {
      looked()
      return current
    },
    devflowCreate: async (...args: unknown[]) => {
      called('create', ...args)
      current = disabled()
      return current
    },
    devflowPreview: async (...args: unknown[]) => {
      called('preview', ...args)
      return { yaml: 'version: 1\n# commented\n', files: [{ path: '.devpit/devflow/devflow.yaml', change: 'create' }] }
    },
    devflowEdit: async (...args: unknown[]) => {
      called('edit', ...args)
      current = active('version: 1\nenabled: true\nstages:\n  context: { enabled: false }\n')
      return current
    },
    devflowAsk: async (...args: unknown[]) => {
      called('ask', ...args)
      const waiting: DevflowAsk = { id: 'dfa_1', projectId: 'prj_api', project: 'api', change: 'enable', askedAt: 1, detail: null }
      current = { ...current, waiting: [waiting] }
      return { asks: [waiting] }
    },
    devflowAnswer: async (...args: unknown[]) => {
      called('answer', ...args)
      current = active()
      return { asks: [] }
    },
    devflowSkillText: async () => ({ skill: { name: 'devflow-plan', layer: 'built-in', file: null, refused: null }, text: '# Devflow: plan' }),
    devflowCalibration: async () => ({ projectId: 'prj_api', rubrics: [], decisions: standing }),
  },
}))

const { DevflowPane, LOOK_EVERY_MS, outsideSaid } = await import('./DevflowPane')

afterEach(() => {
  standing = null
  cleanup()
  called.mockReset()
  shell.show.mockReset()
  current = missing()
})

describe('the Devflow screen', () => {
  it('is off until activated, and the setup it then shows asks nothing first', async () => {
    render(<DevflowPane />)
    expect(await screen.findByText('Devflow is off in this project')).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Use recommended' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Activate Devflow' }))
    expect(await screen.findByText('make test')).toBeTruthy()
    expect(screen.getByText(/Makefile:1/)).toBeTruthy()
    expect(screen.getByText('make lint').closest('.dfcmd')?.getAttribute('title')).toBe('`phpstan` is not installed')
    expect(screen.getAllByText('unavailable')).toHaveLength(1)
    expect(screen.getByRole('button', { name: 'Use recommended' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Customize' })).toBeTruthy()
    expect(called).not.toHaveBeenCalled()
  })

  it('shows a file that is off read-only, with Activate', async () => {
    current = disabled()
    render(<DevflowPane />)
    expect(await screen.findByText('Set up, and off')).toBeTruthy()
    expect(screen.getByLabelText('devflow.yaml').textContent).toContain('# the file')
    expect(screen.queryByRole('switch')).toBeNull()
    expect(screen.getByRole('button', { name: 'Activate' })).toBeTruthy()
  })

  it('edits the stages, the reviewers and Jev of a file that is on', async () => {
    current = active()
    render(<DevflowPane />)
    expect(await screen.findByText('On here')).toBeTruthy()
    expect(screen.getByRole('switch', { name: 'Plan' })).toBeTruthy()
    expect(screen.getByText('Reviewers')).toBeTruthy()
    expect(screen.getByText('Ask Jev at the gates')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Key and provider in Settings → Decisions' })).toBeTruthy()
  })

  it('says on the Jev panel that Decisions has no key, or why it is failing', async () => {
    current = active()
    standing = { state: 'no_key', reason: 'no key — set one in Settings → Decisions' }
    render(<DevflowPane />)
    expect(await screen.findByText('no key — set one in Settings → Decisions')).toBeTruthy()
    cleanup()
    standing = { state: 'failing', reason: 'Decisions refused it (400 Bad Request): no criteria' }
    render(<DevflowPane />)
    /* A second render in one test: past the default second under a loaded suite. */
    expect(await screen.findByText('failing: Decisions refused it (400 Bad Request): no criteria', {}, { timeout: 3000 })).toBeTruthy()
  })

  it('says where an invalid file is refused, and opens it', async () => {
    current = base({ state: 'invalid', yaml: 'version: 1\nenabled: maybe\n', error: { line: 2, field: 'enabled', message: 'expected true or false' } })
    render(<DevflowPane />)
    expect(await screen.findByText('Line 2, `enabled`: expected true or false')).toBeTruthy()
    expect(screen.getByText(/tools are off/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Open in the editor' }))
    expect(shell.show).toHaveBeenCalledWith('file', expect.objectContaining({ path: current.file }))
  })

  it('writes the recommended setup in one click, as the setup tool would, and says to commit it', async () => {
    render(<DevflowPane />)
    fireEvent.click(await screen.findByRole('button', { name: 'Activate Devflow' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Use recommended' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('create', 'prj_api', 'recommended', '{}'))
    expect(await screen.findByText(/commit these files yourself/)).toBeTruthy()
    expect(screen.getByText('Set up, and off')).toBeTruthy()
  })

  it('walks a Custom setup through four steps, previews the commented file, then creates what was chosen', async () => {
    render(<DevflowPane />)
    fireEvent.click(await screen.findByRole('button', { name: 'Activate Devflow' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Customize' }))
    fireEvent.click(screen.getByRole('switch', { name: 'Context' }))
    for (const step of ['Checks', 'Reviewers', 'Jev']) {
      fireEvent.click(screen.getByRole('button', { name: 'Next' }))
      expect(screen.getByRole('button', { name: new RegExp(step) }).getAttribute('aria-current')).toBe('step')
    }
    fireEvent.click(screen.getByRole('button', { name: 'Preview the file' }))
    expect(await screen.findByText(/# commented/)).toBeTruthy()
    const [, , preset, set] = called.mock.calls.find((call) => call[0] === 'preview')!
    expect(preset).toBe('custom')
    const chosen = JSON.parse(set as string) as Record<string, unknown>
    expect(chosen['stages.context.enabled']).toBe(false)
    expect(chosen['stages.execution.checks']).toEqual([{ name: 'unit', run: 'make test', report: 'none' }])
    fireEvent.click(screen.getByRole('button', { name: 'Create' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('create', 'prj_api', 'custom', set))
  })

  it('activates through the confirmation, never by itself', async () => {
    current = disabled()
    render(<DevflowPane />)
    fireEvent.click(await screen.findByRole('button', { name: 'Activate' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('ask', 'prj_api', 'enable'))
    fireEvent.click(await screen.findByRole('button', { name: 'Switch on' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('answer', 'dfa_1', true))
    expect(await screen.findByText('On here')).toBeTruthy()
  })

  it("shows the release checks apart from each card's, and adds one under release", async () => {
    const release = [{ name: 'e2e', run: 'make e2e', report: 'none', evidence: [], source: 'Makefile:9' }]
    current = active(undefined, {
      stages: config().stages.map((one) => (one.name === 'execution' ? { ...one, release } : one)),
    })
    render(<DevflowPane />)
    expect(await screen.findByText('Release checks')).toBeTruthy()
    expect(screen.getByDisplayValue('make e2e')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('New release check name'), { target: { value: 'suite' } })
    fireEvent.change(screen.getByLabelText('New release check command'), { target: { value: 'make suite' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add release check' }))
    await waitFor(() =>
      expect(called).toHaveBeenCalledWith(
        'edit',
        'prj_api',
        'stages.execution.release',
        JSON.stringify([
          { name: 'e2e', run: 'make e2e', report: 'none' },
          { name: 'suite', run: 'make suite', report: 'none' },
        ]),
      ),
    )
  })

  it('writes an edit to the file, and does not mistake its own edit for one made outside', async () => {
    current = active()
    render(<DevflowPane />)
    fireEvent.click(await screen.findByRole('switch', { name: 'Context' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('edit', 'prj_api', 'stages.context.enabled', 'false'))
    await act(async () => heard['devflow:changed']?.('prj_api'))
    expect(screen.queryByText(outsideSaid())).toBeNull()
  })

  it('reads a file changed outside again, and says so', async () => {
    current = active()
    render(<DevflowPane />)
    expect(await screen.findByText('On here')).toBeTruthy()
    current = active('version: 1\nenabled: true\nreview:\n  reviewers: []\n', { reviewers: [] })
    await act(async () => window.dispatchEvent(new Event('focus')))
    expect(await screen.findByText(outsideSaid())).toBeTruthy()
    expect(screen.queryByRole('combobox', { name: 'Persona' })).toBeNull()
  })
})

/* The pane stays mounted behind other tabs, and it read the file every few
   seconds there for nobody. */
describe('the Devflow screen behind another tab', () => {
  afterEach(() => {
    cleanup()
    vi.useRealTimers()
    shell.active = { kind: 'devflow' }
    current = missing()
  })

  const settle = async (ms: number): Promise<void> => {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ms)
    })
  }

  it('does not look at the file on the beat, and looks at once back in front', async () => {
    vi.useFakeTimers()
    current = active()
    const { rerender } = render(<DevflowPane />)
    await settle(0)
    shell.active = { kind: 'board' }
    rerender(<DevflowPane />)
    looked.mockClear()

    await settle(LOOK_EVERY_MS * 3)
    expect(looked).not.toHaveBeenCalled()

    shell.active = { kind: 'devflow' }
    rerender(<DevflowPane />)
    await settle(0)
    expect(looked).toHaveBeenCalledTimes(1)
    await settle(LOOK_EVERY_MS)
    expect(looked).toHaveBeenCalledTimes(2)
  })
})

describe('the stages of a file that is on', () => {
  afterEach(() => {
    cleanup()
    called.mockReset()
    current = missing()
  })

  it('are moved, removed and added as version 2 lists them', async () => {
    current = active()
    render(<DevflowPane />)
    fireEvent.click(await screen.findByRole('button', { name: 'Move context up' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('edit', 'prj_api', 'stages', expect.any(String)))
    const moved = JSON.parse(called.mock.calls.find((call) => call[0] === 'edit')![3] as string) as { uses: string }[]
    expect(moved.slice(0, 2).map((one) => one.uses)).toEqual(['builtin/context', 'builtin/briefing'])
    called.mockReset()
    fireEvent.change(await screen.findByLabelText('The stage uses'), { target: { value: 'project/pr' } })
    fireEvent.change(screen.getByLabelText('Its gate command'), { target: { value: './scripts/pr.sh' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add stage' }))
    await waitFor(() => expect(called).toHaveBeenCalledWith('edit', 'prj_api', 'stages', expect.any(String)))
    const added = JSON.parse(called.mock.calls[0]![3] as string) as Record<string, unknown>[]
    expect(added.at(-1)).toEqual({ uses: 'project/pr', enabled: true, gate: { command: './scripts/pr.sh' } })
  })
})
