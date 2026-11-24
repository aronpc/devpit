import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import type { Account, SignIn } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'

/* Who this install is signed in as, and how it gets there.
 *
 * Kept out of the shell context on purpose: signing in is a small machine with
 * a timer in it, and a hook that owns the timer can clean it up. What the rest
 * of the window sees is the four fields below.
 */
export interface Membership {
  readonly account: Account | null
  /** True until the first answer arrives. The difference between "nobody" and
   *  "not yet" is the difference between a redirect and a wait. */
  readonly loading: boolean
  /** Set once when a stored token turns out to be dead, so the screen can say
   *  "you were signed out" rather than nothing. */
  readonly expired: boolean
  /** The sign-in in progress, if there is one. */
  readonly signingIn: SignIn | null
  /** What went wrong last, in the server's words. */
  readonly failed: string | null
  /** The accounts site this build talks to. */
  readonly origin: string
  signIn: () => Promise<void>
  signOut: () => Promise<void>
  cancelSignIn: () => void
}

export function useAccount(): Membership {
  const [account, setAccount] = useState<Account | null>(null)
  const [loading, setLoading] = useState(true)
  const [expired, setExpired] = useState(false)
  const [signingIn, setSigningIn] = useState<SignIn | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const [origin, setOrigin] = useState('')
  const timer = useRef<number | null>(null)

  const stopPolling = useCallback(() => {
    if (timer.current !== null) {
      window.clearInterval(timer.current)
      timer.current = null
    }
  }, [])

  /* The profile loads itself. Nobody should have to open a pane for the window
     to find out who they are. */
  useEffect(() => {
    let live = true
    void ask(() => commands.accountRead()).then((asked) => {
      if (!live) return
      if (asked.data) {
        setAccount(asked.data.account)
        setExpired(asked.data.expired)
        setOrigin(asked.data.origin)
      }
      setLoading(false)
    })
    return () => {
      live = false
    }
  }, [])

  useEffect(() => stopPolling, [stopPolling])

  const cancelSignIn = useCallback(() => {
    stopPolling()
    setSigningIn(null)
  }, [stopPolling])

  const signIn = useCallback(async () => {
    setFailed(null)
    setExpired(false)

    const started = await ask(() => commands.accountSignIn())
    if (!started.data) {
      setFailed(started.error ?? t('workspace.signIn.serverDown'))
      return
    }
    setSigningIn(started.data)

    /* The server says how often to ask, so a polling loop cannot become a load
       test by being written badly here. */
    stopPolling()
    timer.current = window.setInterval(() => {
      void ask(() => commands.accountPoll()).then((asked) => {
        if (!asked.data) return
        if (asked.data.state === 'signed') {
          stopPolling()
          setSigningIn(null)
          setAccount(asked.data.account)
        }
        if (asked.data.state === 'expired') {
          stopPolling()
          setSigningIn(null)
          setFailed(t('workspace.signIn.expiredTryAgain'))
        }
      })
    }, started.data.intervalSeconds * 1000)
  }, [stopPolling])

  const signOut = useCallback(async () => {
    // The screen forgets first: a network failure on the way out must not
    // leave someone looking at an account they asked to leave.
    cancelSignIn()
    setAccount(null)
    setExpired(false)
    await ask(() => commands.accountSignOut())
  }, [cancelSignIn])

  return useMemo(
    () => ({ account, loading, expired, signingIn, failed, origin, signIn, signOut, cancelSignIn }),
    [account, loading, expired, signingIn, failed, origin, signIn, signOut, cancelSignIn],
  )
}
