import { convertFileSrc } from '@tauri-apps/api/core'
import { useContext, useEffect, useLayoutEffect, useRef, useState } from 'react'

import { ask, commands } from './live'
import { went } from './problems'
import { answer, toolInput, toolResult, type Bridge, type Rpc } from './mcpAppBridge'
import { AppsScopeContext } from './mcpApps'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/*
 * The page an MCP tool came with, under the answer that called it.
 *
 * It runs on its own scheme with its server's policy, in a sandbox without
 * `allow-same-origin`: its origin is opaque, so it reaches neither this
 * window nor devpit's IPC, and talks only through `postMessage`. A call it
 * makes to its server waits for the person: the CLI would run it without
 * asking anyone.
 */

interface Asking {
  readonly name: string
  readonly args: unknown
  readonly go: (allowed: boolean, always: boolean) => void
}

/* To the page, whatever its origin: a sandboxed page's is opaque, and only
   its own window is ever addressed. */
const post = (frame: React.RefObject<HTMLIFrameElement | null>, message: Rpc | object): void =>
  frame.current?.contentWindow?.postMessage(message, '*')

export function McpApp({ called, input, output, isError }: { called: string; input: string; output: string | null; isError: boolean }): React.JSX.Element | null {
  const scope = useContext(AppsScopeContext)
  const frame = useRef<HTMLIFrameElement>(null)
  const [page, setPage] = useState<{ id: string; bordered: boolean } | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const [height, setHeight] = useState(160)
  const [asking, setAsking] = useState<Asking | null>(null)
  const ready = useRef(false)
  const trusted = useRef(new Set<string>())

  useEffect(() => {
    if (!scope) return
    let gone = false
    void ask(() => commands.mcpAppOpen(scope.profileId, scope.cwd, called)).then((opened) => {
      if (gone) return
      setPage(opened.data ?? null)
      setFailed(opened.error)
    })
    return () => {
      gone = true
    }
  }, [scope, called])

  /* The latest result, for a page that becomes ready after it came. */
  const result = useRef({ output, isError })
  result.current = { output, isError }

  /* The result arrives after the page when the call is still running. */
  useEffect(() => {
    if (ready.current && output !== null) post(frame, toolResult(output, isError))
  }, [output, isError])

  /* Listening before the page can speak: it asks to initialize once, as it
     loads, and a passive effect could run after that and miss it. */
  useLayoutEffect(() => {
    if (!scope || !page) return
    let parsed: unknown = {}
    try {
      parsed = JSON.parse(input)
    } catch {
      parsed = {}
    }
    const run = async (name: string, args: unknown): Promise<unknown> => {
      const said = await ask(() => commands.mcpAppCall(scope.profileId, scope.cwd, called, name, JSON.stringify(args ?? {})))
      if (said.error || said.data === null) throw new Error(said.error ?? 'the call did not answer')
      return JSON.parse(said.data)
    }
    const bridge: Bridge = {
      input: parsed,
      theme: document.documentElement.dataset.theme === 'light' ? 'light' : 'dark',
      call: (name, args) =>
        trusted.current.has(name)
          ? run(name, args)
          : new Promise((resolve, reject) =>
              setAsking({
                name,
                args,
                go: (allowed, always) => {
                  setAsking(null)
                  if (!allowed) return reject(new Error('the person did not allow it'))
                  if (always) trusted.current.add(name)
                  run(name, args).then(resolve, reject)
                },
              }),
            ),
      openLink: (url) => void ask(() => commands.urlOpen(url)).then(went),
      resize: (next) => setHeight(Math.min(1600, Math.max(60, Math.ceil(next)))),
      ready: () => {
        ready.current = true
        post(frame, toolInput(parsed))
        const { output: now, isError: failed } = result.current
        if (now !== null) post(frame, toolResult(now, failed))
      },
    }
    const heard = (event: MessageEvent): void => {
      if (event.source !== frame.current?.contentWindow || typeof event.data !== 'object' || event.data === null) return
      void answer(event.data as Rpc, bridge).then((said) => said && post(frame, said))
    }
    window.addEventListener('message', heard)
    return () => window.removeEventListener('message', heard)
  }, [scope, page, called, input])

  if (failed) return <p className="mcpapp__note">{failed}</p>
  if (!page) return <p className="mcpapp__note">{t('workspace.mcpApp.opening', { tool: called.split('__').pop() })}</p>

  return (
    <div className="mcpapp" data-bordered={page.bordered ? 'true' : undefined}>
      <iframe
        ref={frame}
        className="mcpapp__frame"
        title={t('workspace.mcpApp.frame', { tool: called.split('__').pop(), server: called.split('__')[1] ?? t('workspace.mcpApp.server') })}
        src={convertFileSrc(page.id, 'mcpapp')}
        sandbox="allow-scripts allow-forms"
        referrerPolicy="no-referrer"
        style={{ height }}
      />
      {asking && (
        <div className="mcpapp__ask" role="alertdialog" aria-label={t('workspace.mcpApp.askLabel')}>
          <span>
            {tRich('workspace.mcpApp.ask', { name: <code>{asking.name}</code> })}
          </span>
          <button className="btn" onClick={() => asking.go(false, false)}>{t('workspace.mcpApp.deny')}</button>
          <button className="btn" onClick={() => asking.go(true, false)}>{t('workspace.mcpApp.once')}</button>
          <button className="btn btn--go" onClick={() => asking.go(true, true)}>{t('workspace.mcpApp.always')}</button>
        </div>
      )}
    </div>
  )
}
