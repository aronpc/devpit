import { useCallback, useEffect, useState } from 'react'

import type { Credentials, ProfileSignIn, ProviderFound, ProvidersFound } from '../gen/bindings'
import { AgentGlyph } from './AgentGlyph'
import { useCheckEvery } from './checkEvery'
import { ask, commands } from './live'
import { useShell } from './useShell'

/*
 * Settings → Providers: the CLIs devpit holds a chat with, found on this
 * machine without anyone typing a path. Looked for when the pane opens, at
 * the check interval, and on Scan again. "Sign in" types the CLI's own login
 * into a terminal; devpit never handles the credentials.
 */

const SAID: Record<Credentials, string> = {
  saved: 'Signed in',
  missing: 'Signed out',
  expired: 'Sign-in expired',
  unknown: 'Sign-in not readable',
}

/** Whether a sign-in is worth offering: anything but one known to hold. */
const needsSignIn = (state: Credentials): boolean => state !== 'saved'

export function ProviderScan(): React.JSX.Element {
  const { project, show, closePrefs } = useShell()
  const every = useCheckEvery()
  const [found, setFound] = useState<ProvidersFound | null>(null)
  const [scanning, setScanning] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const scan = useCallback(
    (again: boolean) =>
      ask(() => commands.providersDetect(again)).then((answer) => {
        setError(answer.error)
        if (answer.data) setFound(answer.data)
      }),
    [],
  )

  useEffect(() => {
    void scan(false)
    const timer = window.setInterval(() => void scan(false), every)
    return () => window.clearInterval(timer)
  }, [scan, every])

  const again = (): void => {
    setScanning(true)
    void scan(true).finally(() => setScanning(false))
  }

  /* The login runs in a terminal of the open project, where the person finishes it. */
  const signIn = (id: string, label: string): void => {
    if (!project) return setError('Open a project first: the sign-in runs in one of its terminals.')
    show('term', { title: `Sign in · ${label}`, login: id })
    closePrefs()
  }

  const providers = found?.providers ?? []
  return (
    <div className="card2">
      <div className="card2__top">
        <div style={{ flex: '1', minWidth: '0' }}>
          <div className="card2__t">
            Providers{' '}
            <span className="git__count">{providers.filter((one) => one.path).length} installed</span>
          </div>
          <div className="card2__d">
            The CLIs devpit holds a chat with, found on your PATH and in the usual install folders.
            Each one signs in with its own login, in a terminal.
          </div>
        </div>
        <button className="card2__go" disabled={scanning} onClick={again}>
          {scanning ? 'Scanning…' : 'Scan again'}
        </button>
      </div>
      {error && <p className="acc__note">{error}</p>}
      {providers.map((one) => (
        <ProviderLine
          key={one.id}
          one={one}
          profiles={(found?.profiles ?? []).filter((profile) => profile.base === one.id)}
          onSignIn={signIn}
        />
      ))}
    </div>
  )
}

function ProviderLine({
  one,
  profiles,
  onSignIn,
}: {
  one: ProviderFound
  profiles: readonly ProfileSignIn[]
  onSignIn: (id: string, label: string) => void
}): React.JSX.Element {
  const [copied, setCopied] = useState(false)
  const installed = one.path !== null
  const copy = (): void => {
    void navigator.clipboard
      .writeText(one.install)
      .then(() => setCopied(true))
      .catch(() => setCopied(false))
  }
  return (
    <>
      <div className={`prov${installed ? '' : ' prov--off'}`} data-provider={one.id}>
        <span className="prov__ico">
          <AgentGlyph agent={one.id} base={one.id} />
          <span className="prov__dot" style={{ background: installed ? 'var(--success)' : 'var(--ghost)' }}></span>
        </span>
        <span className="prov__body">
          <span className="prov__top">
            <span className="prov__n">{one.label}</span>
            <span className="prov__v">{installed ? (one.version ?? 'version unknown') : 'not installed'}</span>
            <span className="pscan__tag">{one.chat ? 'Chat' : 'Terminal only'}</span>
          </span>
          <span className="prov__sub">
            {installed ? (needsSignIn(one.signedIn) ? one.login : one.path) : one.install}
          </span>
        </span>
        {installed ? (
          <>
            <span className="pscan__state" data-state={one.signedIn}>{SAID[one.signedIn]}</span>
            {needsSignIn(one.signedIn) && (
              <button className="acc__act" onClick={() => onSignIn(one.id, one.label)}>Sign in</button>
            )}
          </>
        ) : (
          <button className="acc__act" onClick={copy} aria-label={`Copy how to install ${one.label}`}>
            {copied ? 'Copied' : 'Copy'}
          </button>
        )}
      </div>
      {installed &&
        profiles.map((profile) => (
          <div className="pscan__prof" key={profile.id}>
            <span className="prov__n">{profile.label}</span>
            <span className="pscan__state" data-state={profile.signedIn}>{SAID[profile.signedIn]}</span>
            {needsSignIn(profile.signedIn) && (
              <button className="acc__act" onClick={() => onSignIn(profile.id, profile.label)}>Sign in</button>
            )}
          </div>
        ))}
    </>
  )
}
