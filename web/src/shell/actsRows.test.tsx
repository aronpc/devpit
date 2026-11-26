import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { Part } from '../gen/bindings'
import { acts } from './acts'
import { shownRows } from './activity'
import { Acts } from './ActsRows'

afterEach(cleanup)

const read = (id: string, file: string): Part[] => [
  { kind: 'tool_call', id, name: 'Read', input: JSON.stringify({ file_path: file }), state: 'ok', parent: null },
  { kind: 'tool_result', call_id: id, output: 'contents', is_error: false, parent: null },
]

describe('what the agent did, folded', () => {
  it('shows a run of reads as one row with the count', () => {
    render(<Acts parts={[...read('a', 'a.rs'), ...read('b', 'b.rs'), ...read('c', 'c.rs')]} live />)
    expect(screen.getByText('3 files')).toBeTruthy()
    // Folded: the files themselves are one click away, not on screen.
    expect(screen.queryByText('a.rs')).toBeNull()
  })

  it('opens to the individual calls', () => {
    render(<Acts parts={[...read('a', 'a.rs'), ...read('b', 'b.rs'), ...read('c', 'c.rs')]} live />)
    fireEvent.click(screen.getByText('3 files'))
    expect(screen.getByText('a.rs')).toBeTruthy()
    expect(screen.getByText('c.rs')).toBeTruthy()
  })

  it('leaves the call still running outside the fold', () => {
    const running: Part = { kind: 'tool_call', id: 'd', name: 'Read', input: JSON.stringify({ file_path: 'd.rs' }), state: 'running', parent: null }
    render(<Acts parts={[...read('a', 'a.rs'), ...read('b', 'b.rs'), ...read('c', 'c.rs'), running]} live />)
    expect(screen.getByText('3 files')).toBeTruthy()
    expect(screen.getAllByText('d.rs').length).toBeGreaterThan(0)
  })
})

describe('a single tool', () => {
  it('is its own line, with no fold around it', () => {
    const { container } = render(<Acts parts={read('a', 'a.rs')} live={false} />)
    expect(container.querySelector('.acts__h')).toBeNull()
    expect(container.querySelectorAll('.arow')).toHaveLength(1)
    expect(screen.getByText('a.rs')).toBeTruthy()
  })
})

describe('a subagent', () => {
  it('takes its first part without breaking the row that started it', () => {
    const call: Part = { kind: 'tool_call', id: 'sub', name: 'Task', input: JSON.stringify({ description: 'look around' }), state: 'running', parent: null }
    const { rerender } = render(<Acts parts={[call]} live />)
    const child: Part = { kind: 'tool_call', id: 'c1', name: 'Read', input: JSON.stringify({ file_path: 'deep.rs' }), state: 'running', parent: 'sub' }
    rerender(<Acts parts={[call, child]} live />)
    // The row is still on screen: a hook-order error unmounts the whole tree.
    expect(screen.getByText('look around')).toBeTruthy()
  })
})

describe('a row opened to its details', () => {
  const mcp: Part[] = [
    { kind: 'tool_call', id: 'm', name: 'mcp__figma__get_design_context', input: JSON.stringify({ nodeId: '1:2', clientFrameworks: 'react' }), state: 'ok', parent: null },
    { kind: 'tool_result', call_id: 'm', output: '{"frame":"Login"}', is_error: false, parent: null },
  ]

  it('names the MCP server and shows the whole input and the output, each copyable', () => {
    render(<Acts parts={mcp} live />)
    expect(screen.getByText('figma')).toBeTruthy()
    fireEvent.click(screen.getByText('Get design context'))
    expect(screen.getByText(/"clientFrameworks": "react"/)).toBeTruthy()
    expect(screen.getByText('{"frame":"Login"}')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Copy input' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Copy output' })).toBeTruthy()
  })

  it('shows the rest of a command\'s input beside the command', () => {
    const bash: Part = { kind: 'tool_call', id: 'b', name: 'Bash', input: JSON.stringify({ command: 'sleep 4', run_in_background: true }), state: 'ok', parent: null }
    render(<Acts parts={[bash]} live />)
    fireEvent.click(screen.getByText('sleep 4'))
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeTruthy()
    expect(screen.getByText(/"run_in_background": true/)).toBeTruthy()
  })
})

describe('a thought', () => {
  it('says how long it took', () => {
    render(<Acts parts={[{ kind: 'thinking', text: '', parent: null, duration_ms: 12400 }]} live />)
    expect(screen.getByText('Thought for 12s')).toBeTruthy()
  })
})

describe('subagents started together', () => {
  const agent = (id: string, description: string): Part[] => [
    { kind: 'tool_call', id, name: 'Agent', input: JSON.stringify({ description }), state: 'ok', parent: null },
    { kind: 'tool_result', call_id: id, output: 'Async agent launched', is_error: false, parent: null },
  ]
  const task = (callId: string, status: string, ms: number): Part => ({
    kind: 'task', task_id: `t-${callId}`, call_id: callId, task_kind: 'local_agent', description: null, status, summary: null, duration_ms: ms,
  })

  it('are one row that opens to each with its own state and time', () => {
    render(<Acts parts={[...agent('a', 'Read the docs'), ...agent('b', 'Run the tests'), task('a', 'running', 3000), task('b', 'completed', 22611)]} live />)
    fireEvent.click(screen.getByText('2 subagents · 1 working'))
    expect(screen.getByText('Read the docs')).toBeTruthy()
    expect(screen.getByText('Working · 3s')).toBeTruthy()
    expect(screen.getByText('Done · 23s')).toBeTruthy()
  })

  it('say a stopped or failed one did not succeed, and keep it when successful work is hidden', () => {
    const parts = [...agent('a', 'Read the docs'), ...agent('b', 'Run the tests'), task('a', 'stopped', 5000), task('b', 'failed', 7000)]
    render(<Acts parts={parts} live />)
    fireEvent.click(screen.getByText('2 subagents'))
    expect(screen.getByText('Stopped · 5s')).toBeTruthy()
    expect(screen.getByText('Failed · 7s')).toBeTruthy()
    expect(shownRows(acts(parts), 'failed').map((row) => row.id)).toEqual(['a', 'b'])
  })
})
