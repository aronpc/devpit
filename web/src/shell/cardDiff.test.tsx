import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CardFindings } from '../gen/bindings'

const findingDismiss = vi.fn((..._asked: unknown[]) => Promise.resolve({ status: 'ok', data: null }))
const cardFindings = vi.fn()
const cardDiff = vi.fn()

vi.mock('./live', () => ({
  ask: async (call: () => Promise<{ data: unknown }>) => ({ data: (await call()).data, error: null }),
  commands: {
    cardFindings: () => cardFindings(),
    cardDiff: () => cardDiff(),
    findingDismiss: (runId: string, at: number, dismissed: boolean) => findingDismiss(runId, at, dismissed),
  },
}))

import { CardDiff } from './CardDiff'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

const REVIEW: CardFindings = {
  reviews: [
    {
      runId: 'devflow:card_8/20261127T195214Z',
      found: {
        findings: [{ file: 'app/Shipping.php', line: 93, severity: 'blocking', why: '[spec] The mode changes nothing' }],
        standing: 'current',
        atRevision: '78b4e267d5',
        now: '78b4e267d5',
        rubric: 'devflow-review',
      },
      dismissed: [],
    },
  ],
}

describe("a card's review findings", () => {
  it('can be called a false positive on a card with no checkout of its own', async () => {
    cardFindings.mockResolvedValue({ data: REVIEW })
    render(<CardDiff cardId="card_8" checkout={false} />)
    fireEvent.click(await screen.findByRole('button', { name: 'False positive' }))
    await waitFor(() => expect(findingDismiss).toHaveBeenCalledWith('devflow:card_8/20261127T195214Z', 0, true))
    expect(await screen.findByText('A false positive, you said')).toBeTruthy()
    expect(cardDiff).not.toHaveBeenCalled()
  })

  it('draws nothing on a card with no checkout and no review', async () => {
    cardFindings.mockResolvedValue({ data: { reviews: [] } })
    const { container } = render(<CardDiff cardId="card_8" checkout={false} />)
    await waitFor(() => expect(cardFindings).toHaveBeenCalled())
    expect(container.textContent).toBe('')
  })
})
