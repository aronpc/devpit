import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ComposerTools } from './ComposerTools'
import type { Chat } from './useChat'

let heard: { text: string | null; note: string | null } = { text: 'fix the invoice total', note: null }
const pasted = vi.fn()
vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null }),
  commands: {
    chatPaste: (...args: unknown[]) => (pasted(...args), { name: 'voice-1.webm', path: '/h/pasted/voice-1.webm', kind: 'webm' }),
    chatTranscribe: () => heard,
  },
}))
vi.mock('./useShell', () => ({ useShell: () => ({ project: { id: 'p1' } }) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(() => Promise.resolve(['/w/a.txt', '/w/b.png'])) }))
/* Each recording the composer opened, and a way to hold the microphone's opening back. */
const opened: { cancel: ReturnType<typeof vi.fn> }[] = []
let gate: Promise<void> | null = null
const recordCalls = vi.fn()
vi.mock('./recorder', async (actual) => ({
  ...(await actual<typeof import('./recorder')>()),
  formatHere: () => 'audio/webm;codecs=opus',
  record: async () => {
    recordCalls()
    if (gate) await gate
    const one = { missing: null, stop: () => Promise.resolve(new Blob(['x'], { type: 'audio/webm' })), cancel: vi.fn() }
    opened.push(one)
    return one
  },
  base64Of: () => Promise.resolve('eA=='),
}))

afterEach(() => {
  cleanup()
  pasted.mockClear()
  recordCalls.mockClear()
  opened.length = 0
  gate = null
  heard = { text: 'fix the invoice total', note: null }
})

const chatWith = () => ({ attach: vi.fn(), keep: vi.fn(), detach: vi.fn() }) as unknown as Chat & { attach: ReturnType<typeof vi.fn>; keep: ReturnType<typeof vi.fn>; detach: ReturnType<typeof vi.fn> }

describe("the composer's files and voice", () => {
  it('attaches the files picked', async () => {
    const chat = chatWith()
    render(<ComposerTools chat={chat} mine onHeard={vi.fn()} />)
    fireEvent.click(screen.getByLabelText('Attach files'))
    await waitFor(() => expect(chat.attach).toHaveBeenCalledWith(['/w/a.txt', '/w/b.png']))
  })

  it('records, keeps the recording and puts what was heard in the composer', async () => {
    const chat = chatWith()
    const onHeard = vi.fn()
    render(<ComposerTools chat={chat} mine onHeard={onHeard} />)
    fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
    fireEvent.pointerUp(screen.getByLabelText('Record a voice message'))
    fireEvent.click(await screen.findByText('Done'))
    await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
    expect(pasted).toHaveBeenCalledWith('p1', 'audio/webm', 'eA==')
    expect(chat.keep).toHaveBeenCalled()
    expect(chat.detach).toHaveBeenCalledWith('/h/pasted/voice-1.webm')
  })

  it('sends the recording as a file, and says why, when it could not be heard', async () => {
    heard = { text: null, note: 'no whisper on this machine' }
    const chat = chatWith()
    render(<ComposerTools chat={chat} mine onHeard={vi.fn()} />)
    fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
    fireEvent.click(await screen.findByText('Done'))
    expect(await screen.findByText(/Sent as a recording: no whisper on this machine/)).toBeTruthy()
    expect(chat.keep).toHaveBeenCalled()
    expect(chat.detach).not.toHaveBeenCalled()
  })

  describe('push to talk', () => {
    const talk = { key: ' ', code: 'Space', ctrlKey: true, shiftKey: true }

    it('records while the key is held and makes it words when it is let go', async () => {
      const chat = chatWith()
      const onHeard = vi.fn()
      render(<ComposerTools chat={chat} mine onHeard={onHeard} />)
      fireEvent.keyDown(window, talk)
      expect(await screen.findByLabelText('Recording')).toBeTruthy()
      fireEvent.keyDown(window, { ...talk, repeat: true })
      expect(pasted).not.toHaveBeenCalled()
      fireEvent.keyUp(window, talk)
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
      expect(pasted).toHaveBeenCalledTimes(1)
    })

    it('ends when a key held with it is let go first', async () => {
      const onHeard = vi.fn()
      render(<ComposerTools chat={chatWith()} mine onHeard={onHeard} />)
      fireEvent.keyDown(window, talk)
      await screen.findByLabelText('Recording')
      fireEvent.keyUp(window, { key: 'Control', ctrlKey: false, shiftKey: true })
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
    })

    it('ends when the window loses focus while held', async () => {
      const onHeard = vi.fn()
      render(<ComposerTools chat={chatWith()} mine onHeard={onHeard} />)
      fireEvent.keyDown(window, talk)
      await screen.findByLabelText('Recording')
      fireEvent.blur(window)
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
    })

    it('ends as it starts when let go before the microphone opened', async () => {
      const onHeard = vi.fn()
      render(<ComposerTools chat={chatWith()} mine onHeard={onHeard} />)
      fireEvent.keyDown(window, talk)
      fireEvent.keyUp(window, talk)
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
      expect(screen.queryByLabelText('Recording')).toBeNull()
    })

    it('records nothing for a chat that is not in front', async () => {
      render(<ComposerTools chat={chatWith()} mine={false} onHeard={vi.fn()} />)
      fireEvent.keyDown(window, talk)
      fireEvent.keyUp(window, talk)
      await new Promise((done) => setTimeout(done, 20))
      expect(screen.queryByLabelText('Recording')).toBeNull()
      expect(pasted).not.toHaveBeenCalled()
    })
  })

  /* Opening the microphone takes a moment: what happens in it must not leave one open. */
  describe('while the microphone opens', () => {
    const held = (): (() => void) => {
      let open = (): void => {}
      gate = new Promise<void>((done) => (open = done))
      return open
    }

    it('opens it once for two quick presses', async () => {
      const open = held()
      render(<ComposerTools chat={chatWith()} mine onHeard={vi.fn()} />)
      fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
      fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
      open()
      expect(await screen.findByLabelText('Recording')).toBeTruthy()
      expect(recordCalls).toHaveBeenCalledTimes(1)
    })

    it('closes what it opens when the composer is gone meanwhile', async () => {
      const open = held()
      const { unmount } = render(<ComposerTools chat={chatWith()} mine onHeard={vi.fn()} />)
      fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
      unmount()
      open()
      await waitFor(() => expect(opened).toHaveLength(1))
      await waitFor(() => expect(opened[0]!.cancel).toHaveBeenCalled())
    })
  })

  /* Another tab in front hides this composer: nothing goes on recording unseen. */
  describe('when another chat comes in front', () => {
    it('makes a held recording words in the chat it was said to', async () => {
      const onHeard = vi.fn()
      const { rerender } = render(<ComposerTools chat={chatWith()} mine onHeard={onHeard} />)
      fireEvent.keyDown(window, { key: ' ', code: 'Space', ctrlKey: true, shiftKey: true })
      await screen.findByLabelText('Recording')
      rerender(<ComposerTools chat={chatWith()} mine={false} onHeard={onHeard} />)
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
      expect(screen.queryByLabelText('Recording')).toBeNull()
    })

    it('stops one started with the button', async () => {
      const onHeard = vi.fn()
      const { rerender } = render(<ComposerTools chat={chatWith()} mine onHeard={onHeard} />)
      fireEvent.pointerDown(screen.getByLabelText('Record a voice message'))
      await screen.findByLabelText('Recording')
      rerender(<ComposerTools chat={chatWith()} mine={false} onHeard={onHeard} />)
      await waitFor(() => expect(onHeard).toHaveBeenCalledWith('fix the invoice total'))
      expect(screen.queryByLabelText('Recording')).toBeNull()
    })
  })
})
