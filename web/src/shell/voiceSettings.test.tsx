import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Transcribing } from '../gen/bindings'

const downloaded = vi.fn()
const tested = vi.fn()
let format: string | null = 'audio/webm'
const settings: Transcribing = { engine: 'local', model: '', language: '', url: '', keySet: false, local: 'whisper-cli' } as Transcribing

vi.mock('./live', () => ({
  ask: async (call: () => unknown) => ({ data: await call(), error: null, loading: false }),
  commands: {
    transcribeRead: () => settings,
    transcribeModelDownload: () => (downloaded(), { ...settings, model: '/home/me/.devpit/models/ggml-base.bin' }),
    transcribeTest: (...args: unknown[]) => (tested(...args), { text: 'testing one two', note: null }),
  },
}))
let missing: string | null = null
vi.mock('./recorder', async (actual) => ({
  ...(await actual<typeof import('./recorder')>()),
  formatHere: () => format,
  microphones: async () => [
    { id: 'u120', label: 'U-120 Analog Stereo' },
    { id: 'pixy', label: 'EMEET PIXY Mono' },
  ],
  record: async () => ({ missing, stop: async () => new Blob(['x'], { type: 'audio/webm' }), cancel: () => {} }),
  refusal: () => 'The microphone is blocked.',
  systemLanguage: () => 'pt',
  base64Of: async () => 'eA==',
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: async () => null }))

const { VoiceSettings } = await import('./VoiceSettings')

afterEach(() => {
  cleanup()
  format = 'audio/webm'
  missing = null
  localStorage.clear()
})

describe('voice in Settings', () => {
  it("downloads whisper.cpp's base model when none is chosen, and uses it", async () => {
    render(<VoiceSettings />)
    fireEvent.click(await screen.findByRole('button', { name: 'Download base model' }))
    expect(await screen.findByText('/home/me/.devpit/models/ggml-base.bin')).toBeTruthy()
    expect(downloaded).toHaveBeenCalledOnce()
    expect(screen.queryByRole('button', { name: 'Download base model' })).toBeNull()
  })

  it('tests the microphone and the engine at once, and says what it heard', async () => {
    render(<VoiceSettings />)
    fireEvent.click(await screen.findByRole('button', { name: 'Test the microphone' }))
    expect(await screen.findByText('Heard: “testing one two”', {}, { timeout: 5000 })).toBeTruthy()
    expect(tested).toHaveBeenCalledWith('audio/webm', 'eA==', 'pt')
  })

  it('says why when this window cannot record', async () => {
    format = null
    render(<VoiceSettings />)
    fireEvent.click(await screen.findByRole('button', { name: 'Test the microphone' }))
    expect(await screen.findByText(/GStreamer/)).toBeTruthy()
  })

  it('lists the microphones and keeps the one chosen', async () => {
    render(<VoiceSettings />)
    const pick = (await screen.findByRole('combobox', { name: 'Microphone' })) as HTMLSelectElement
    await screen.findByRole('option', { name: 'EMEET PIXY Mono' })
    expect([...pick.options].map((one) => one.text)).toEqual(['System default', 'U-120 Analog Stereo', 'EMEET PIXY Mono'])
    fireEvent.change(pick, { target: { value: 'pixy' } })
    cleanup()
    render(<VoiceSettings />)
    expect(((await screen.findByRole('combobox', { name: 'Microphone' })) as HTMLSelectElement).value).toBe('pixy')
  })

  it('says so when the chosen microphone is gone and the default was heard', async () => {
    localStorage.setItem('devpit:microphone', JSON.stringify({ id: 'logi', label: 'Logi USB Headset Mono' }))
    missing = 'Logi USB Headset Mono'
    render(<VoiceSettings />)
    expect(await screen.findByRole('option', { name: 'Logi USB Headset Mono — not connected' })).toBeTruthy()
    fireEvent.click(await screen.findByRole('button', { name: 'Test the microphone' }))
    expect(await screen.findByText(/Logi USB Headset Mono is not connected: recording from the system’s default microphone\. Heard: “testing one two”/, {}, { timeout: 5000 })).toBeTruthy()
  })
})
