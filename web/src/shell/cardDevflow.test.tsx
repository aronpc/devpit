import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { DevflowCardEvidence } from '../gen/bindings'

vi.mock('./window', () => ({ onCarried: () => () => {} }))
const shell = { project: { id: 'prj_api' }, show: vi.fn() }
vi.mock('./useShell', () => ({ useShell: () => shell }))

let evidence: DevflowCardEvidence | null = null
vi.mock('./live', () => ({
  ask: async (call: () => Promise<unknown>) => ({ data: await call(), error: null, loading: false }),
  commands: { devflowCardEvidence: async () => evidence },
}))

const { CardDevflow, exploitSaid, reviewerSaid } = await import('./CardDevflow')

afterEach(() => {
  cleanup()
  evidence = null
})

describe("a card's Devflow", () => {
  it('is absent from a card Devflow never ran on', async () => {
    evidence = { cardId: 'c1', gate: null, reviewers: [], verdict: null, folder: '/a', artifacts: [], exploits: [] }
    const { container } = render(<CardDevflow cardId="c1" />)
    await waitFor(() => expect(container.innerHTML).toBe(''))
  })

  it('lists the newest gate, each reviewer with time and cost, and opens a file the run kept', async () => {
    evidence = {
      cardId: 'c1',
      gate: { passed: false, run: 'r', at: 'r', withEvidence: 1, criteria: 2, missing: ['criterion 2 has no test'] },
      reviewers: [
        { agent: 'claude', persona: 'devflow-security', status: 'answered', findings: 2, durationMs: 61000, costUsd: 0.42, reason: null },
        { agent: 'codex', persona: 'devflow-spec', status: 'skipped', findings: 0, durationMs: null, costUsd: null, reason: 'not signed in' },
      ],
      verdict: 'request_changes',
      folder: '/home/me/artifacts',
      artifacts: ['devflow/c1/20261011T150000Z/gate.json'],
      exploits: [],
    }
    render(<CardDevflow cardId="c1" />)
    expect(await screen.findByText('Gate not passed')).toBeTruthy()
    expect(screen.getByText(/1 of 2 criteria with evidence · criterion 2 has no test/)).toBeTruthy()
    expect(screen.getByText('2 findings · 1 min 1 s · $0.42')).toBeTruthy()
    expect(screen.getByText('skipped — not signed in')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'gate.json' }))
    expect(shell.show).toHaveBeenCalledWith('file', expect.objectContaining({ path: '/home/me/artifacts/devflow/c1/20261011T150000Z/gate.json' }))
    expect(reviewerSaid({ ...evidence.reviewers[0]!, findings: 1, durationMs: null, costUsd: null })).toBe('1 finding')
    expect(screen.queryByText('Decisions')).toBeNull()
  })

  it('shows each exploit the review confirmed, where it is and the test the gate wants green', async () => {
    evidence = {
      cardId: 'c1',
      gate: null,
      reviewers: [],
      verdict: 'request_changes',
      folder: '/a',
      artifacts: [],
      exploits: [{ test: 'tests/attack.rs::a_path_outside_the_root_is_refused', file: 'src/files.rs', line: 42, title: 'Path traversal' }],
    }
    render(<CardDevflow cardId="c1" />)
    expect(await screen.findByText('Exploit confirmed · Path traversal')).toBeTruthy()
    expect(screen.getByText('src/files.rs:42 · tests/attack.rs::a_path_outside_the_root_is_refused')).toBeTruthy()
    expect(exploitSaid({ test: 't', file: null, line: null, title: null })).toBe('t')
  })

  it('says where Decisions stands as the Jev panel does, and nothing of it when it is off', async () => {
    const ran = { cardId: 'c1', gate: { passed: true, run: 'r', at: 'r', withEvidence: 1, criteria: 1, missing: [] }, reviewers: [], verdict: null, folder: '/a', artifacts: [], exploits: [] }
    evidence = { ...ran, decisions: { state: 'failing', reason: 'Decisions refused it (400 Bad Request): no criteria' } }
    render(<CardDevflow cardId="c1" />)
    expect(await screen.findByText('failing: Decisions refused it (400 Bad Request): no criteria')).toBeTruthy()
    cleanup()
    evidence = { ...ran, decisions: { state: 'no_key', reason: 'no key — set one in Settings → Decisions' } }
    render(<CardDevflow cardId="c1" />)
    expect(await screen.findByText('no key — set one in Settings → Decisions')).toBeTruthy()
    cleanup()
    evidence = { ...ran, decisions: { state: 'off', reason: 'decisions.use is false in devflow.yaml' } }
    render(<CardDevflow cardId="c1" />)
    expect(await screen.findByText('Gate passed')).toBeTruthy()
    expect(screen.queryByText('Decisions')).toBeNull()
  })
})
