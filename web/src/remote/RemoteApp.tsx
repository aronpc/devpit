import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'

import type { AgentHealth, Board, Conversations, LivePane, RemoteDraft, RemoteIn, RemoteOut, RemoteProject, RemoteQuestion, RemoteStop, RemoteTalks } from '../gen/bindings'
import { currentLocale, setLocale, subscribe, t } from '../i18n'
import { Kind, unframe } from '../shell/paneSocket'
import { launcherLink } from '../shell/remoteLauncher'
import { connectRelay, CLOSED, deviceKey, forgetMachine, keepMachine, machinesKept, offerOpened, type RelayMachine, type RelayPairing } from './relay'
import { RelayNothing, RelayPair } from './RelayPair'
import { RemoteBoard } from './RemoteBoard'
import { RemoteChats } from './RemoteChats'
import { RemotePair } from './RemotePair'
import { RemoteSecret } from './RemoteSecret'
import { RemoteTerminal } from './RemoteTerminal'
import { RemoteWaiting } from './RemoteWaiting'
import { bytesOf, connect, keepToken, tokenKept, type Link } from './socket'

/*
 * The viewer: which machine, which project, and its terminals, board,
 * questions and chats. What it may do — type, answer — is the machine's word,
 * said in its welcome; asking past it is refused there, not only here.
 *
 * It comes two ways and looks the same: served by the machine on the
 * tailnet, a token said in hello; or served by devpit.app at /remote/, a Noise
 * session through the relay with a machine whose key its QR code carried.
 */

/** devpit.app's build of the viewer, for the relay, lives under /remote/. */
const RELAY_PAGE = import.meta.env.BASE_URL !== '/'

type View = 'terminals' | 'board' | 'waiting' | 'chats' | 'secrets'

/* `binary` and `stops` are absent from an older host. */
export type Welcome = Omit<Extract<RemoteOut, { t: 'welcome' }>, 't'>

export function RemoteApp(): React.JSX.Element {
  /* The host's language, said in its welcome: this device has no settings of
     its own to read it from, and never reads `navigator.language` for it. */
  const locale = useSyncExternalStore(subscribe, currentLocale)
  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])
  const [token, setToken] = useState<string | null>(tokenKept)
  const [offer, setOffer] = useState(() => offerOpened(location.hash))
  const [relay, setRelay] = useState<RelayMachine | null>(() => machinesKept()[0] ?? null)
  const [pairing, setPairing] = useState<RelayPairing | null>(null)
  const [away, setAway] = useState<{ why: string; code: number } | null>(null)
  const [up, setUp] = useState(false)
  const [welcome, setWelcome] = useState<Welcome | null>(null)
  const [refused, setRefused] = useState<string | null>(null)
  const [approving, setApproving] = useState(false)
  const [failed, setFailed] = useState<string | null>(null)
  const [projects, setProjects] = useState<readonly RemoteProject[]>([])
  const [projectId, setProjectId] = useState<string | null>(null)
  const [view, setView] = useState<View>('terminals')
  const [pane, setPane] = useState<string | null>(null)
  const [board, setBoard] = useState<Board | null>(null)
  const [questions, setQuestions] = useState<readonly RemoteQuestion[]>([])
  const [drafts, setDrafts] = useState<readonly RemoteDraft[]>([])
  const [stops, setStops] = useState<readonly RemoteStop[]>([])
  const [health, setHealth] = useState<AgentHealth | null>(null)
  const [chats, setChats] = useState<Conversations | null>(null)
  const [talks, setTalks] = useState<RemoteTalks | null>(null)
  const [started, setStarted] = useState<string | null>(null)
  const [kept, setKept] = useState<string | null>(null)
  const link = useRef<Link | null>(null)
  /* A terminal's bytes, by its pane in JSON or by its handle when raw. */
  const bytes = useRef<(from: string | number, data: Uint8Array) => void>(() => {})
  /* The chat open now hears what is said about it here. */
  const talk = useRef<(heard: RemoteOut) => void>(() => {})
  const projectRef = useRef(projectId)
  projectRef.current = projectId

  const send = useCallback((message: RemoteIn) => link.current?.send(message), [])
  const sendBytes = useCallback((frame: Uint8Array<ArrayBuffer>) => link.current?.sendBytes(frame), [])

  useEffect(() => {
    if (!token && !relay) return
    const heard = (message: RemoteOut): void => {
      switch (message.t) {
        case 'welcome':
          setApproving(false)
          setWelcome(message)
          /* An older host says none: English stays. */
          if (message.locale) setLocale(message.locale)
          setRefused(null)
          link.current?.send({ t: 'projects' })
          link.current?.send({ t: 'waiting' })
          link.current?.send({ t: 'drafts' })
          if (message.stops) link.current?.send({ t: 'stops' })
          return
        case 'approving':
          setApproving(true)
          return
        case 'refused':
          setApproving(false)
          setRefused(message.why)
          // Through the relay, asking again would only be refused again.
          if (relay) link.current?.close()
          return
        case 'projects':
          setProjects(message.projects)
          setProjectId((was) => was ?? message.projects.find((one) => one.terminals.length > 0)?.id ?? message.projects[0]?.id ?? null)
          return
        case 'paneBytes':
          bytes.current(message.pane, bytesOf(message.b64))
          return
        case 'paneClosed':
          setPane((was) => (was === message.pane ? null : was))
          return
        case 'board':
          if (message.project === projectRef.current) setBoard(message.board)
          return
        case 'boardChanged':
          if (message.project === projectRef.current) link.current?.send({ t: 'board', project: message.project })
          return
        case 'waiting':
          setQuestions(message.questions)
          return
        case 'drafts':
          setDrafts(message.drafts)
          return
        case 'stops':
          setStops(message.stops)
          if (message.said) {
            setFailed(message.said)
            window.setTimeout(() => setFailed(null), 6000)
          }
          return
        case 'agentHealth':
          setHealth(message.health)
          return
        case 'chats':
          if (message.project === projectRef.current) setChats(message.conversations)
          return
        case 'talks':
          setTalks(message.talks)
          return
        case 'chat':
        case 'chatFrame':
        case 'session':
        case 'heard':
        case 'attached':
          talk.current(message)
          return
        case 'started':
          setStarted(message.name)
          window.setTimeout(() => setStarted(null), 6000)
          return
        case 'secretKept':
          setKept(message.name)
          window.setTimeout(() => setKept(null), 6000)
          return
        case 'failed':
          talk.current(message)
          setFailed(message.why)
          window.setTimeout(() => setFailed(null), 6000)
          return
        case 'pong':
          return
      }
    }
    const raw = (frame: Uint8Array): void => {
      const parts = unframe(frame)
      if (parts?.kind === Kind.Output) bytes.current(parts.handle, parts.bytes)
    }
    if (relay) {
      const state = (now: boolean, why?: string, code = 0): void => {
        setUp(now)
        setAway(now || !why ? null : { why, code })
      }
      link.current = connectRelay(relay, deviceKey(), heard, state, { origin: location.origin, pairing, raw })
    } else if (token) {
      link.current = connect(token, heard, setUp, raw)
    }
    return () => link.current?.close()
  }, [token, relay, pairing])

  /* Each view asks for what it shows when it is opened. */
  useEffect(() => {
    if (!projectId || !up) return
    if (view === 'board') send({ t: 'board', project: projectId })
    if (view === 'chats') send({ t: 'chats', project: projectId })
    if (view === 'terminals') send({ t: 'projects' })
    if (view === 'waiting') {
      send({ t: 'waiting' })
      send({ t: 'drafts' })
      if (welcome?.stops) send({ t: 'stops' })
      send({ t: 'agentHealth' })
    }
  }, [view, projectId, up, send, welcome?.stops])

  const waitingOn = questions.length + drafts.length + stops.length

  if (offer) {
    const pair = (name: string): void => {
      // The secret is good once: off the address before it is used.
      history.replaceState(null, '', location.pathname)
      keepMachine(offer.machine)
      setPairing({ secret: offer.secret, name })
      setRelay(offer.machine)
      setRefused(null)
      setOffer(null)
    }
    return <RelayPair machine={offer.machine} onPair={pair} />
  }

  if (!token && !relay) return RELAY_PAGE ? <RelayNothing /> : <RemotePair onPaired={(fresh) => (keepToken(fresh), setToken(fresh))} />

  if (approving)
    return (
      <main className="rm rm--center">
        <h1 className="rm__title">{t('remote.app.allowTitle')}</h1>
        <p className="rm__said">{t('remote.app.allowSaid', { machine: relay?.name ?? t('remote.app.theMachine') })}</p>
      </main>
    )

  if (refused)
    return (
      <main className="rm rm--center">
        <h1 className="rm__title">{t('remote.app.refusedTitle')}</h1>
        <p className="rm__said">{refused}</p>
        <button className="rm__btn" onClick={() => (relay ? (forgetMachine(relay.host), setRelay(null), setPairing(null)) : (keepToken(null), setToken(null)), setRefused(null))}>
          {t('remote.app.pairAgain')}
        </button>
      </main>
    )

  const project = projects.find((one) => one.id === projectId) ?? null
  const launcher = relay ? null : launcherLink(window.location.origin, welcome?.host)
  /* A session's own terminal, from its chat. */
  const terminal = (at: LivePane): void => {
    setProjectId(at.projectId)
    setBoard(null)
    setChats(null)
    setPane(at.paneId)
    setView('terminals')
  }
  return (
    <main className="rm">
      <header className="rm__head">
        <span className="rm__dot" data-up={up ? 'true' : undefined} />
        <span className="rm__host">{welcome?.host ?? relay?.name ?? 'devpit'}</span>
        {launcher && (
          <a className="rm__dim rm__add" href={launcher} target="_blank" rel="noreferrer" title={t('remote.app.addTitle')}>
            {t('remote.app.add')}
          </a>
        )}
        <select className="rm__pick" aria-label={t('remote.app.project')} value={projectId ?? ''} onChange={(event) => (setProjectId(event.target.value), setPane(null), setBoard(null), setChats(null))}>
          {projects.map((one) => (
            <option key={one.id} value={one.id}>
              {one.name}
            </option>
          ))}
        </select>
      </header>
      {away && (
        <p className="rm__failed" role="status">
          {away.why} {away.code === CLOSED.signIn && <a href="/login?next=/remote">{t('remote.app.signIn')}</a>}
        </p>
      )}
      {failed && <p className="rm__failed" role="alert">{failed}</p>}
      {started && <p className="rm__said" role="status">{t('remote.app.started', { name: started })}</p>}
      {kept && <p className="rm__said" role="status">{t('remote.app.kept', { name: kept })}</p>}
      <nav className="rm__tabs">
        {(['terminals', 'board', 'waiting', 'chats', 'secrets'] as const).map((one) => (
          <button key={one} className="rm__tab" data-on={view === one ? 'true' : undefined} onClick={() => setView(one)}>
            {one === 'waiting' && waitingOn > 0 ? t('remote.app.tab.waitingCount', { count: waitingOn }) : t(`remote.app.tab.${one}`)}
          </button>
        ))}
      </nav>
      <section className="rm__body">
        {view === 'terminals' && project && (
          <>
            <div className="rm__list">
              {project.terminals.length === 0 && <p className="rm__said">{t('remote.app.noTerminal', { project: project.name })}</p>}
              {project.terminals.map((one) => (
                <button key={one.pane} className="rm__row" data-on={pane === one.pane ? 'true' : undefined} onClick={() => setPane(one.pane)}>
                  <span>{one.command || t('remote.app.shell')}</span>
                  <span className="rm__dim">{one.pane.slice(-6)}</span>
                </button>
              ))}
            </div>
            {pane && <RemoteTerminal key={pane} project={project.id} pane={pane} typing={welcome?.typing ?? false} binary={welcome?.binary ?? false} send={send} sendBytes={sendBytes} bytes={bytes} />}
          </>
        )}
        {view === 'board' && <RemoteBoard board={board} project={projectId} typing={welcome?.typing ?? false} waiting={waitingOn} onWaiting={() => setView('waiting')} send={send} />}
        {view === 'waiting' && <RemoteWaiting questions={questions} drafts={drafts} stops={stops} health={health} answering={welcome?.answering ?? false} typing={welcome?.typing ?? false} send={send} />}
        {view === 'chats' && <RemoteChats project={projectId} chats={chats} talks={talks} drafts={drafts} typing={welcome?.typing ?? false} send={send} hears={talk} onTerminal={terminal} />}
        {view === 'secrets' && <RemoteSecret key={projectId} projects={projects} project={projectId} typing={welcome?.typing ?? false} send={send} />}
      </section>
    </main>
  )
}
