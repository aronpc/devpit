import { afterEach, describe, expect, it } from 'vitest'

import { modes } from './chat'
import { conversationKey, opening, remember, remembered, reopened } from './chatModes'

afterEach(() => localStorage.clear())

const full = modes()[modes().length - 1].id

describe('chat modes', () => {
  it('remembers the last pick per profile', () => {
    remember('claude2', { permission: full })
    remember('claude2', { effort: 'high' })
    expect(remembered('claude2')).toEqual({ permission: full, effort: 'high' })
    expect(remembered('claude')).toEqual({})
  })

  it('reopens a conversation in the mode it ran with, over the profile’s last pick', () => {
    expect(opening({ permission: modes()[0].id }, { permission: full }).permission).toBe(modes()[0].id)
  })

  it('opens a new conversation in the profile’s last pick', () => {
    expect(opening({}, { permission: full, effort: 'high' })).toEqual({ permission: full, effort: 'high' })
  })

  it('keeps a pick not yet sent over what the last turn ran with', () => {
    remember(conversationKey('conv_1'), { permission: full })
    expect(reopened('conv_1', { permission: modes()[0].id }, 'claude2').permission).toBe(full)
    expect(reopened('conv_2', { permission: modes()[0].id }, 'claude2').permission).toBe(modes()[0].id)
  })

  it('drops a mode this build does not offer', () => {
    expect(opening({ permission: 'yolo' }, {}).permission).toBeUndefined()
  })
})
