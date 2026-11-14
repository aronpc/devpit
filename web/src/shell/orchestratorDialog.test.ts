import { describe, expect, it } from 'vitest'

import type { Profile } from '../gen/bindings'
import { chatAccounts } from './OrchestratorDialog'

const profile = (id: string, driver: string, reach: Profile['reach'] = 'runnable'): Profile => ({ id, driver, reach }) as Profile

describe('chatAccounts', () => {
  it('offers every account that holds a chat, and no terminal-only or missing one', () => {
    const all = [profile('claude', 'claude'), profile('codex', 'codex'), profile('opencode', 'acp'), profile('gemini', ''), profile('grok', 'acp', 'missing')]
    expect(chatAccounts(all).map((one) => one.id)).toEqual(['claude', 'codex', 'opencode'])
  })
})
