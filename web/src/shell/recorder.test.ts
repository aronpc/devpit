import { afterEach, describe, expect, it, vi } from 'vitest'

import { formatHere, record, refusal, resampled, systemLanguage, WAV, WAV_RATE, wavOf } from './recorder'

/* A MediaRecorder that hands over what it is told to, when it stops. */
class FakeRecorder {
  static gives: number[] = []
  state = 'inactive'
  ondataavailable: ((event: { data: Blob }) => void) | null = null
  onstop: (() => void) | null = null
  start(): void {
    this.state = 'recording'
  }
  stop(): void {
    this.state = 'inactive'
    for (const size of FakeRecorder.gives) this.ondataavailable?.({ data: new Blob(['x'.repeat(size)]) })
    this.onstop?.()
  }
}

/* Every microphone opened, and whether it was closed again: a light left on is a stream never stopped. */
let streams: { stopped: boolean }[] = []
const lit = (): number => streams.filter((one) => !one.stopped).length

/* A system with these microphones plugged in: asking for any other by id fails as a browser does,
   and one named busy is there but will not open. */
const microphone = (
  plugged: { deviceId: string; label: string }[] = [],
  { busy, listing }: { busy?: string; listing?: Error } = {},
): ReturnType<typeof vi.fn> => {
  const getUserMedia = vi.fn((asked: { audio: { deviceId?: { exact: string } } }) => {
    const wanted = asked.audio.deviceId?.exact
    if (wanted && wanted === busy) return Promise.reject(new DOMException('busy', 'NotReadableError'))
    if (wanted && !plugged.some((one) => one.deviceId === wanted)) return Promise.reject(new DOMException('gone', 'OverconstrainedError'))
    const one = { stopped: false }
    streams.push(one)
    return Promise.resolve({ getTracks: () => [{ stop: () => (one.stopped = true) }] })
  })
  const enumerateDevices = () => (listing ? Promise.reject(listing) : Promise.resolve(plugged.map((one) => ({ ...one, kind: 'audioinput' }))))
  Object.defineProperty(navigator, 'mediaDevices', { value: { getUserMedia, enumerateDevices }, configurable: true })
  vi.stubGlobal('MediaRecorder', FakeRecorder)
  return getUserMedia
}

afterEach(() => {
  vi.unstubAllGlobals()
  FakeRecorder.gives = []
  streams = []
})

const tone = (hz: number, rate: number, seconds = 1): Float32Array =>
  Float32Array.from({ length: rate * seconds }, (_, i) => 0.5 * Math.sin((2 * Math.PI * hz * i) / rate))
/* Its strength away from the ends, where the filter has nothing on one side. */
const strength = (samples: Float32Array): number => {
  const middle = samples.slice(200, -200)
  return Math.sqrt(middle.reduce((sum, one) => sum + one * one, 0) / middle.length)
}

describe('recording a voice message', () => {
  it('records nothing where the window has no recorder', () => {
    expect(formatHere()).toBeNull()
  })

  it('hears in the system language by its first part', () => {
    const was = navigator.language
    Object.defineProperty(navigator, 'language', { value: 'pt-BR', configurable: true })
    expect(systemLanguage()).toBe('pt')
    Object.defineProperty(navigator, 'language', { value: was, configurable: true })
  })

  it('says how to unblock a refused microphone', () => {
    expect(refusal(new DOMException('no', 'NotAllowedError'))).toMatch(/privacy settings/)
    expect(refusal(new DOMException('no', 'NotFoundError'))).toBe('No microphone was found.')
  })

  /* WebKitGTK feeds its encoder at the rate the track declares, and a track
     asked for no rate declares 0: the recording comes back empty. */
  it('asks the microphone for a sample rate', async () => {
    const getUserMedia = microphone()
    await record('audio/mp4')
    expect(getUserMedia).toHaveBeenCalledWith({ audio: expect.objectContaining({ sampleRate: { ideal: 48_000 } }) })
  })

  it('answers with what was heard, and with nothing when the pieces are empty', async () => {
    microphone()
    FakeRecorder.gives = [3, 5]
    const heard = await (await record('audio/mp4')).stop()
    expect(heard?.size).toBe(8)
    expect(heard?.type).toBe('audio/mp4')

    FakeRecorder.gives = [0]
    expect(await (await record('audio/mp4')).stop()).toBeNull()
  })

  describe('where MediaRecorder writes only MP4', () => {
    /* WebKitGTK's MP4 comes out fragmented, and the services refuse it. */
    it('records WAV instead', () => {
      microphone()
      vi.stubGlobal('MediaRecorder', { isTypeSupported: (type: string) => type === 'audio/mp4' })
      vi.stubGlobal('AudioContext', class {})
      expect(formatHere()).toBe(WAV)
    })

    it('keeps WebM where it is written', () => {
      microphone()
      vi.stubGlobal('MediaRecorder', { isTypeSupported: (type: string) => type.startsWith('audio/webm') })
      vi.stubGlobal('AudioContext', class {})
      expect(formatHere()).toBe('audio/webm;codecs=opus')
    })

    it('makes a 16-bit mono WAV of samples at 16 kHz', async () => {
      const samples = new Float32Array(WAV_RATE)
      samples.fill(1, 0, WAV_RATE / 2)
      samples.fill(-1, WAV_RATE / 2)
      const blob = wavOf(samples)
      const bytes = new DataView(await blob.arrayBuffer())
      const text = (at: number): string => String.fromCharCode(...new Uint8Array(bytes.buffer, at, 4))
      expect(blob.type).toBe('audio/wav')
      expect([text(0), text(8), text(12), text(36)]).toEqual(['RIFF', 'WAVE', 'fmt ', 'data'])
      expect(bytes.getUint16(22, true)).toBe(1)
      expect(bytes.getUint32(24, true)).toBe(WAV_RATE)
      expect(bytes.getUint32(40, true)).toBe(WAV_RATE * 2)
      expect(blob.size).toBe(44 + WAV_RATE * 2)
      expect(bytes.getInt16(44, true)).toBe(0x7fff)
      expect(bytes.getInt16(44 + (WAV_RATE - 1) * 2, true)).toBe(-0x8000)
    })

    /* Averaging blocks let a 10 kHz tone through at half strength, folded to
       6 kHz; WebKitGTK's own resampler let it through whole. */
    it('brings 48 and 44.1 kHz to 16 kHz without folding what is above 8 kHz into the speech', () => {
      for (const rate of [48_000, 44_100]) {
        const kept = resampled([tone(1_000, rate)], rate)
        expect(kept.length).toBe(Math.floor(rate / (rate / WAV_RATE)))
        expect(strength(kept)).toBeGreaterThan(0.34)
        for (const above of [10_000, 13_000, 20_000]) expect(strength(resampled([tone(above, rate)], rate))).toBeLessThan(0.01)
      }
    })

    it('joins the pieces as they came, and leaves 16 kHz as it is', () => {
      const whole = tone(1_000, WAV_RATE)
      expect(resampled([whole.slice(0, 4_096), whole.slice(4_096)], WAV_RATE)).toEqual(whole)
    })
  })

  /* Whatever fails once the microphone is open must close it: the system's light stays on otherwise. */
  describe('a microphone opened and then failed', () => {
    it('is closed when the recorder cannot start', async () => {
      microphone()
      vi.stubGlobal(
        'MediaRecorder',
        class {
          constructor() {
            throw new DOMException('no', 'NotSupportedError')
          }
        },
      )
      await expect(record('audio/webm')).rejects.toThrow()
      expect(streams).toHaveLength(1)
      expect(lit()).toBe(0)
    })

    it('is closed when the samples cannot be taken', async () => {
      microphone()
      vi.stubGlobal(
        'AudioContext',
        class {
          constructor() {
            throw new DOMException('no', 'NotSupportedError')
          }
        },
      )
      await expect(record(WAV)).rejects.toThrow()
      expect(lit()).toBe(0)
    })
  })
})
