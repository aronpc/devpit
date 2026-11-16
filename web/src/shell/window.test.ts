import { afterEach, describe, expect, it, vi } from 'vitest'

/* `listen`, whose registration the test holds back until it lets it land —
   the state a real one is in while the call is in flight. */
let land: (unlisten: () => void) => void = () => {}
let lose: (reason: unknown) => void = () => {}
const listen = vi.fn(
  () =>
    new Promise<() => void>((resolve, reject) => {
      land = resolve
      lose = reject
    }),
)
vi.mock('@tauri-apps/api/event', () => ({ listen }))

/* All the microtasks there are, so a promise that stays pending shows as one. */
const flushed = (): Promise<void> => new Promise((done) => setTimeout(done, 0))

afterEach(() => {
  delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__
})

describe('when the shared listener is in place', () => {
  it('is heard of only once the registration has landed', async () => {
    ;(window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {}
    const { onCarried, whenHeard } = await import('./window')
    const drop = onCarried<string>('island:open-pane', () => {})
    let heard = false
    void whenHeard('island:open-pane').then(() => {
      heard = true
    })
    await flushed()
    expect(heard).toBe(false)
    land(() => {})
    await flushed()
    expect(heard).toBe(true)
    drop()
  })

  it('is heard of when the registration failed, so no waiter hangs', async () => {
    ;(window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {}
    const { onCarried, whenHeard } = await import('./window')
    const drop = onCarried<string>('island:open-pane', () => {})
    let heard = false
    const waiting = whenHeard('island:open-pane').then(() => {
      heard = true
    })
    lose(new Error('never registered'))
    await waiting
    expect(heard).toBe(true)
    drop()
  })

  it('is heard of straight away when there is no listener to wait for', async () => {
    ;(window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {}
    const { whenHeard } = await import('./window')
    let heard = false
    void whenHeard('island:open-chat').then(() => {
      heard = true
    })
    await flushed()
    expect(heard).toBe(true)
  })

  it('is heard of straight away outside Tauri', async () => {
    const { whenHeard } = await import('./window')
    let heard = false
    void whenHeard('island:open-pane').then(() => {
      heard = true
    })
    await flushed()
    expect(heard).toBe(true)
  })
})
