import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { rereadDetails, toggleDetail, useDetails } from './chatDetails'
import { ChatHead } from './ChatHead'
import { ChatWhere } from './ChatWhere'

vi.mock('./useShell', () => ({
  useShell: () => ({ project: { name: 'devpit', worktrees: [{ branch: 'main' }] } }),
}))
vi.mock('./PaneCorner', () => ({ PaneCorner: ({ children }: { children: React.ReactNode }) => <div>{children}</div> }))

afterEach(() => {
  cleanup()
  localStorage.clear()
  rereadDetails()
})

describe("a conversation's details", () => {
  it('all show until one is hidden, and a hidden one stays hidden after a restart', () => {
    const { result } = renderHook(() => useDetails())
    expect(['card', 'context', 'cost', 'project', 'machine', 'branch'].every((one) => result.current(one as never))).toBe(true)
    act(() => toggleDetail('cost'))
    expect(result.current('cost')).toBe(false)
    rereadDetails()
    const again = renderHook(() => useDetails())
    expect(again.result.current('cost')).toBe(false)
    expect(again.result.current('card')).toBe(true)
  })

  it('are picked from a right-click on the where line', () => {
    const { container } = render(<ChatWhere />)
    expect(screen.getByText('main')).toBeTruthy()
    fireEvent.contextMenu(container.querySelector('.composer__where')!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Branch' }))
    expect(screen.queryByText('main')).toBeNull()
    expect(screen.getByText('devpit')).toBeTruthy()
  })
})

describe("a conversation's header", () => {
  it('hides its cost once Cost is unticked from a right-click', () => {
    const { container } = render(
      <ChatHead tabId="c1" title="Fix the parser" onRename={vi.fn()} onExport={async () => null} card={null} context={null} session={null} cost={1.5} metered orchestrator={null} projectId={null} sending={false} supervised={false} activity="all" onActivity={vi.fn()} />,
    )
    expect(container.querySelector('.pcorner__cost')).not.toBeNull()
    fireEvent.contextMenu(container.querySelector('.chead')!)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Cost' }))
    expect(container.querySelector('.pcorner__cost')).toBeNull()
  })
})
