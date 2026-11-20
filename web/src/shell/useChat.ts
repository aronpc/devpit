import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import type { Ask, Attachment, Context, Message, Profile, Question } from '../gen/bindings'
import type { Said } from './Asked'
import type { Chosen } from './choices'
import { appended, appliedAll, ASKS, batched, failedOn, prepended, retaken, fixedTo, modes, opening, rejoin, saidText, send, settled, steered, withFiles, type Opening } from './chat'
import { chatMessages, type ChatMessages } from './chatMessages'
import { conversationKey, remember, reopened, type Modes } from './chatModes'
import { ask, commands } from './live'
import { PAGE } from './useOlder'
import { KEPT_BYTES } from './pasting'
import type { Extras } from './queueing'
import { keepFiles, keptFiles } from './useDraft'
import { withSkills } from './pills'
import { offers, onProfilesChanged } from './profiles'
import { onChatWoke, onPermissionAsked, onPermissionSettled } from './window'
import { useShellPick } from './shellStore'

/** The card a conversation is filed under. */
export interface ChatCard {
  readonly id: string
  readonly title: string | null
  /** Off the board, the chat names it and cannot open it. */
  readonly onBoard: boolean
}

export interface Chat {
  /** Read by picking what is needed: a streamed batch then draws only its turn. */
  readonly messages: ChatMessages
  readonly card: ChatCard | null
  readonly profiles: readonly Profile[]
  /** The profile this conversation belongs to, once it has spoken. */
  readonly fixed: string | null
  readonly profileId: string | null
  readonly model: string | null
  /** What every turn so far has cost. */
  readonly cost: number
  /** How full the context was when the last turn ended. */
  readonly context: Context | null
  readonly permission: string
  readonly effort: string | null
  /** The folder the conversation runs in, once a turn has fixed it. */
  readonly folder?: string | null
  readonly files: readonly Attachment[]
  /** What the agent is waiting to be allowed to do. */
  readonly asked: readonly Question[]
  readonly sending: boolean
  /** Whether the last turn this chat sent ended in failure. */
  readonly failed: boolean
  readonly error: string | null
  pick: (profileId: string) => void
  setModel: (model: string | null) => void
  setPermission: (mode: string) => void
  setEffort: (effort: string) => void
  attach: (paths: readonly string[]) => void
  /** Skills picked for the next turn, as pills. */
  readonly skills: readonly string[]
  setSkills: (next: readonly string[]) => void
  /** Keeps a pasted picture and attaches it. */
  paste: (file: Blob) => void
  /** Attaches a file devpit already kept — a voice message, recorded. */
  keep: (file: Attachment) => void
  /** Data URLs for pasted pictures, by the path they were kept under. */
  readonly previews: Readonly<Record<string, string>>
  /** The CLI's id for this conversation, once it has one. */
  readonly session: string | null
  /** Whether older messages are left to load, and how a page of them goes in. */
  readonly older: boolean
  took: (page: readonly Message[], older: boolean) => void
  /** Turns a new conversation can go on from, forked at that turn. */
  readonly rewindable: readonly string[]
  rewind: (turnId: string) => void
  /** A new conversation from before a turn, its message in the composer; `putBack` puts the files changed since back first. */
  editFrom: (turnId: string, putBack: boolean) => void
  detach: (path: string) => void
  /** `chosen` answers a question the agent put, one list of labels per question; `reason` goes to the agent with a refusal. */
  answer: (id: string, said: Said, chosen?: Chosen, reason?: string) => void
  /** `back` gets the words of a send refused before it opened, to give back.
      `given` sends those files and skills instead of the composer's, and leaves the composer's be. */
  say: (prompt: string, back?: (prompt: string) => void, given?: Extras) => void
  /** Into the turn in flight; whether it took it. */
  steer: (prompt: string, given?: Extras) => Promise<boolean>
  /** Takes the composer's files and skills off it: they went somewhere else. */
  clearExtras: () => void
  stop: () => void
  /** A stop has not ended the turn after FORCE_AFTER_MS: it can be forced. */
  readonly forceable: boolean
  force: () => void
}

/* How long a stop is given before ending the turn by force is offered. */
export const FORCE_AFTER_MS = 10_000

/* What the composer can pick: something devpit can spawn, switched on, or
   the account this conversation already belongs to. */
const pickable = (all: readonly Profile[], belongs: string | null): Profile[] =>
  all.filter((profile) => profile.path !== null).filter(offers(belongs))

/* A session can be two conversations' (a rewind copies it): a chat's question
   goes to the chat that asked; one from outside any chat, by its session alone. */
const asksHere = (question: Question, session: string, conversationId: string): boolean =>
  question.sessionId === session && (question.conversationId == null || question.conversationId === conversationId)

export function useChat(conversationId: string): Chat {
  const { project, show } = useShellPick((shell) => ({ project: shell.project, show: shell.show }))
  const [messages] = useState(() => chatMessages())
  const setMessages = messages.set
  /* Whether messages older than the first one drawn are left in the transcript. */
  const [older, setOlder] = useState(false)
  const took = useCallback(
    (page: readonly Message[], more: boolean) => {
      setMessages((was) => prepended(was, page))
      setOlder(more)
    },
    [setMessages],
  )
  /* The messages this chat drew itself — a send's, a steer's, a receipt — which a page read meanwhile cannot hold. */
  const sent = useRef(new Set<string>())
  const [rewindable, setRewindable] = useState<readonly string[]>([])
  const [skills, setSkills] = useState<readonly string[]>([])
  const [profiles, setProfiles] = useState<readonly Profile[]>([])
  const [fixed, setFixed] = useState<string | null>(null)
  const [profileId, setProfileId] = useState<string | null>(null)
  const [model, setModel] = useState<string | null>(null)
  const [cost, setCost] = useState(0)
  const [context, setContext] = useState<Context | null>(null)
  const [permission, setPermission] = useState(modes()[0].id)
  const [effort, setEffort] = useState<string | null>(null)
  const [folder, setFolder] = useState<string | null>(null)
  /* Attached and not yet sent, kept with the draft so a reload does not lose them. */
  const [files, setFiles] = useState<readonly Attachment[]>(() => keptFiles(conversationId))
  useEffect(() => keepFiles(conversationId, files), [conversationId, files])
  /* A pasted picture has no path the window can draw, so its preview is kept
     here, by the path the backend gave it. */
  const [previews, setPreviews] = useState<Readonly<Record<string, string>>>({})
  const [asked, setAsked] = useState<readonly Question[]>([])
  const [session, setSession] = useState<string | null>(null)
  /* How many times the conversation effect opened: the questions' effect
     follows it, because it holds that opening and a ref would not tell it. */
  const shown = useRef('')
  const [openings, setOpenings] = useState(0)
  const [card, setCard] = useState<ChatCard | null>(null)
  const [sending, setSending] = useState(false)
  /* Whether the turn in flight was asked to stop, whether that went unheeded
     long enough to offer more, and whether more was asked. */
  const [stopped, setStopped] = useState(false)
  const [late, setLate] = useState(false)
  const [forced, setForced] = useState(false)
  const over = useCallback(() => {
    setSending(false)
    setStopped(false)
    setLate(false)
    setForced(false)
  }, [])
  const [failed, setFailed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  /* This opening of the conversation; the one before it closed with it. */
  const opened = useRef<Opening>(opening())
  /* What the conversation's own head said it last ran with; the profile's
     last pick only fills what that leaves empty. */
  const stored = useRef<Modes>({})
  /* The mode a new chat of this project starts in, when it says one. */
  const startsIn = useRef<string | null>(null)

  /* A turn still running when this chat opens — it was left, or the window
     reloaded — is joined where it is, instead of being said to have died. */
  const rejoined = useCallback(
    (projectId: string, woken = false, before = messages.get().at(-1)?.id ?? null): void => {
      const now = opened.current
      const { push, flush } = batched((frames) => setMessages((was) => appliedAll(was, frames)))
      /* Said once: set again on every frame, it drew the pane again for nothing. */
      let told = false
      const joined = now.join(
        rejoin(conversationId, (frame) => {
          if (!now.open()) return
          if (!told) setSending((told = true))
          if (frame.type === 'session') setSession(frame.session_id)
          else push(frame)
        }),
      )
      void joined?.running
        .then((was) => {
          if (!now.open()) return
          flush()
          /* A woken turn can be over before this asks to join it — a short
             answer to a message. It was written down; read it from there. */
          if (!was && !woken) return
          over()
          /* Only what came after the join is read; the rest stays as drawn. */
          sent.current.clear()
          void Promise.all([
            ask(() => commands.chatAfter(projectId, conversationId, before, PAGE)),
            ask(() => commands.chatHead(projectId, conversationId)),
          ]).then(([after, head]) => {
            if (!now.open()) return
            const saved = after.data
            if (saved?.found) setMessages((was) => appended(was, saved.messages))
            else if (saved) {
              /* Its place was never saved: the last page takes the place of what was drawn,
                 but not of a turn sent while it was read. */
              setMessages((was) => retaken(saved.messages, was, sent.current))
              setOlder(true)
            }
            if (!head.data) return
            setCost(head.data.costUsd ?? 0)
            setContext(head.data.context)
            setRewindable(head.data.rewindable)
          })
        })
        /* `chat_rejoin` never refuses; only the window going rejects it, and
           then there is nobody left to tell. */
        .catch(() => undefined)
    },
    [conversationId, over, messages, setMessages],
  )

  useEffect(() => {
    const now = opening()
    opened.current = now
    setOpenings((was) => was + 1)
    /* Another conversation has no session until its history says so; the old
       one's would let its questions in. The same one, renewed, keeps it. */
    const key = `${project?.id}/${conversationId}`
    if (shown.current !== key) setSession(null)
    shown.current = key
    /* Questions belong to the conversation they were asked in. */
    setAsked([])
    if (!project) return now.close
    void (async () => {
      const [past, found] = await Promise.all([
        ask(() => commands.chatHistory(project.id, conversationId, PAGE, null)),
        ask(() => commands.agentProfiles()),
      ])
      if (!now.open()) return
      const belongs = past.data ? fixedTo(past.data) : null
      if (past.data) {
        setMessages(past.data.messages)
        setOlder(past.data.older)
        setFixed(belongs)
        setProfileId(belongs)
        setModel(past.data.model)
        setFolder(past.data.cwd ?? null)
        stored.current = { permission: past.data.permission ?? undefined, effort: past.data.effort ?? undefined }
        setCost(past.data.costUsd ?? 0)
        setContext(past.data.context ?? null)
        setSession(past.data.sessionId)
        const { cardId, cardTitle, cardOnBoard } = past.data
        setCard(cardId ? { id: cardId, title: cardTitle, onBoard: cardOnBoard } : null)
        setRewindable(past.data.rewindable ?? [])
        rejoined(project.id, false, past.data.messages.at(-1)?.id ?? null)
      }
      startsIn.current = null
      if (!past.data?.sessionId && (past.data?.messages.length ?? 0) === 0) {
        /* A new chat starts as its project says: its own, its repo's, then this machine's. */
        const said = await ask(() => commands.projectSettingsRead(project.id))
        if (!now.open()) return
        const applied = said.data?.applied
        if (applied?.model) setModel(applied.model)
        const mode = modes().find((one) => one.id === applied?.permissionMode)?.id ?? null
        // An orchestrator keeps the mode it listens in between turns.
        startsIn.current = project.orchestrator ? null : mode
        if (startsIn.current) setPermission(startsIn.current)
      }
      const installed = pickable(found.data ?? [], belongs)
      setProfiles(installed)
      /* The driver's own default, until someone picks otherwise. */
      setEffort((was) => was ?? installed[0]?.effortDefault ?? null)
      /* Nothing picked and only one account installed: pick it. Asking which
         of one is a question with no answer. */
      /* An orchestrator speaks as its own account, always. */
      setProfileId((was) => was ?? project.orchestrator ?? (installed.length === 1 ? installed[0].id : null))
      setError(past.error ?? found.error)
    })()
    return now.close
  }, [project, conversationId, rejoined, setMessages])

  /* Reopened, a conversation goes back to the mode it ran in; a new one to the
     last pick on its profile, instead of the most careful mode every time. */
  useEffect(() => {
    const open = reopened(conversationId, stored.current, profileId)
    // An orchestrator in the supervised mode would be deaf between turns:
    // its process stays only when nothing waits on a question.
    // This conversation's own pick, then its project's, then the profile's last.
    const own = reopened(conversationId, stored.current, null).permission
    const permission = own ?? startsIn.current ?? open.permission ?? (project?.orchestrator ? 'acceptEdits' : undefined)
    if (permission) setPermission(permission)
    if (open.effort) setEffort(open.effort)
  }, [profileId, conversationId, project?.orchestrator])

  /* Woken by another session, an orchestrator answers without being asked:
     that turn is joined here the way one left running is. */
  useEffect(() => {
    if (!project) return
    return onChatWoke((woken) => {
      if (woken !== conversationId) return
      /* Joined now, this chat's own turn would reach it twice — through its
         own channel and the rejoin — so a woken one is joined after it. */
      if (opened.current.sending) opened.current.woke = true
      else rejoined(project.id, true)
    })
  }, [project, conversationId, rejoined])

  /* A profile saved or switched in Settings is in the picker at once. */
  useEffect(() => {
    const again = (): void =>
      void ask(() => commands.agentProfiles()).then((found) => {
        if (opened.current.open() && found.data) setProfiles(pickable(found.data, fixed))
      })
    return onProfilesChanged(again)
  }, [fixed])

  /* The session is told to hold its tools only in the mode that asks. Holding
     them in a mode that never asks would wait for a question nobody sends. */
  useEffect(() => {
    if (!session) return
    void ask(() => commands.permissionAskFromNow(session, ASKS(permission)))
  }, [session, permission])

  /* Questions arrive for every conversation; this one keeps its own. */
  useEffect(() => {
    if (!session) return
    const now = opened.current
    let alive = true
    let reading = true
    let unheard = 2
    /* Settled while the read is in flight: the read may still carry them. */
    const gone = new Set<string>()
    /* Heard before both listeners were up: their settling may have been missed. */
    const early = new Set<string>()
    /* One asked while no window had this conversation open never arrived as an
       event. Read only once both events are heard, so a question asked in
       between is in the read or in an event, never in neither. */
    const read = (): void => {
      if (--unheard > 0) return
      void ask(() => commands.permissionQuestions(session, conversationId)).then((found) => {
        reading = false
        /* Answered for a conversation or a session no longer on screen. */
        if (!alive || opened.current !== now || !now.open() || !found.data) return
        const questions = found.data.questions.filter((one) => asksHere(one, session, conversationId) && !gone.has(one.id))
        const waiting = new Set(questions.map((one) => one.id))
        setAsked((was) => [
          ...was.filter((one) => !early.has(one.id) || waiting.has(one.id)),
          ...questions.filter((one) => !was.some((had) => had.id === one.id)),
        ])
      })
    }
    const dropAsked = onPermissionAsked((question) => {
      /* The read may have drawn it already: one question is one row. */
      if (!alive || opened.current !== now || !asksHere(question, session, conversationId)) return
      if (unheard > 0) early.add(question.id)
      setAsked((was) => (was.some((had) => had.id === question.id) ? was : [...was, question]))
    }, read)
    /* Answered from the island, or timed out: the card goes either way. */
    const dropSettled = onPermissionSettled((id) => {
      if (reading) gone.add(id)
      setAsked((was) => was.filter((one) => one.id !== id))
    }, read)
    return () => {
      alive = false
      dropAsked()
      dropSettled()
    }
  }, [session, openings])

  const choosePermission = useCallback(
    (mode: string) => {
      setPermission(mode)
      remember(conversationKey(conversationId), { permission: mode })
      if (profileId) remember(profileId, { permission: mode })
    },
    [conversationId, profileId],
  )

  const answer = useCallback(
    (id: string, said: Said, chosen?: Chosen, reason?: string) => {
      const question = asked.find((one) => one.id === id)
      const allow = said !== 'deny'
      const why = allow ? '' : (reason?.trim() ?? '')
      /* Taken off the list first: the question is answered either way, and a
         row that lingers invites a second click that has nothing to answer. */
      setAsked((was) => was.filter((one) => one.id !== id))
      /* An approved plan moves the session to Supervised; the next turns follow it. */
      if (allow && question?.tool === 'ExitPlanMode' && permission === 'plan') choosePermission(modes()[0].id)
      /* The answer is in the thread already, as the tool's result. */
      if (chosen && allow) return void ask(() => commands.permissionPick(id, chosen.map((one) => [...one])))
      const call =
        said === 'always' && question
          ? () => commands.permissionAlways(id, question.sessionId, question.tool, question.input)
          : why
            ? () => commands.permissionRefuse(id, why)
            : () => commands.permissionAnswer(id, allow ? 'allow' : 'deny')
      void ask(call).then((answered) => {
        /* Kept in the thread only when the answer reached the agent: a receipt
           for a question that had already timed out would record a decision
           nobody's turn ever heard. */
        if (answered.error || !question || !project) return
        void ask(() => commands.chatReceipt(project.id, conversationId, question.tool, question.input, allow)).then(
          (kept) => kept.data && (sent.current.add(kept.data.id), setMessages((was) => [...was, kept.data!])),
        )
      })
    },
    [asked, project, conversationId, setMessages, permission, choosePermission],
  )

  /* Picked for one message, like the attachments: a skill left on would
     silently shape every message after it. */
  const clearExtras = useCallback(() => {
    setFiles([])
    setPreviews((was) => {
      for (const shown of Object.values(was)) URL.revokeObjectURL(shown)
      return {}
    })
    setSkills([])
  }, [])

  const say = useCallback(
    (prompt: string, back?: (prompt: string) => void, given?: Extras) => {
      if (!project || !profileId) return
      const extras = given ?? { files, skills }
      const turn: Ask = {
        projectId: project.id,
        conversationId,
        profileId,
        model,
        prompt: withSkills(withFiles(prompt, extras.files), extras.skills),
        cwd: project.rootPath,
        budgetUsd: null,
        permission,
        effort,
      }
      const now = opened.current
      /* A turn woken while this one waits is saved before it: read from here. */
      const drawn = messages.get().at(-1)?.id ?? null
      const { push, flush } = batched((frames) => setMessages((was) => appliedAll(was, frames)))
      let heard = false
      let answer: string | null = null
      let failed: string | null = null
      const started = send(turn, (frame) => {
        if (!now.open()) return
        heard ||= frame.type === 'opened'
        if (frame.type === 'opened') sent.current.add(frame.message.id)
        if (frame.type === 'opened' && frame.message.role === 'assistant') answer = frame.message.id
        if (frame.type === 'session') setSession(frame.session_id)
        else push(frame)
      })
      if (!started) return setError('not running in the app')
      now.sending = true
      setSending(true)
      setFailed(false)
      setError(null)
      if (!given) clearExtras()
      void started.end
        .then((end) => {
          if (!now.open()) return
          flush()
          setCost((was) => was + (end.costUsd ?? 0))
          if (end.context) setContext(end.context)
          /* A turn's place in the CLI's transcript is known once it has run.
             Only the head is read: the turn's messages are already drawn. */
          void ask(() => commands.chatHead(project.id, conversationId)).then((head) => {
            if (!now.open() || !head.data) return
            setRewindable(head.data.rewindable)
            /* A new conversation learns its session from its first turn; until
               then nothing could tell it to stop and ask. */
            setSession(head.data.sessionId)
          })
        })
        .catch((thrown: { message?: string }) => {
          if (!now.open()) return
          failed = thrown.message ?? 'the turn failed'
          setError(failed)
          setFailed(true)
          /* Refused before it opened: nothing on screen holds the words. */
          if (!heard) back?.(prompt)
        })
        .finally(() => {
          now.sending = false
          if (!now.open()) return
          /* Whatever is still waiting for a frame is in before the turn is
             said to be over. */
          flush()
          setMessages(settled)
          if (failed && answer) setMessages((was) => failedOn(was, answer!, failed!))
          over()
          /* A question the turn left behind has nobody waiting on it now. */
          setAsked([])
          /* The first turn settles the account for good. */
          setFixed(profileId)
          if (now.woke) {
            now.woke = false
            rejoined(project.id, true, drawn)
          }
        })
    },
    [project, conversationId, profileId, model, permission, effort, files, skills, rejoined, clearExtras, over, messages, setMessages],
  )

  /* A dropped file is resolved against the project root before it is shown:
     a path outside the project is refused there, not here. */
  const attach = useCallback(
    (paths: readonly string[]) => {
      if (!project) return
      void (async () => {
        const resolved = await Promise.all(
          paths.map((path) => ask(() => commands.chatAttach(project.id, path))),
        )
        const kept = resolved.flatMap((one) => (one.data ? [one.data] : []))
        const refused = resolved.find((one) => one.error)
        if (refused) setError(refused.error)
        setFiles((was) => [
          ...was,
          ...kept.filter((file) => !was.some((had) => had.path === file.path)),
        ])
      })()
    },
    [project],
  )

  const paste = useCallback(
    (file: Blob) => {
      if (!project) return
      const picture = file.type.startsWith('image/')
      // Said here rather than after the whole of it was encoded and sent.
      if (file.size > KEPT_BYTES) return setError(`that ${picture ? 'picture' : 'paste'} is over 8 MB, too big to attach`)
      const reader = new FileReader()
      reader.onload = () => {
        const url = typeof reader.result === 'string' ? reader.result : ''
        const data = url.slice(url.indexOf(',') + 1)
        void ask(() => commands.chatPaste(project.id, file.type, data)).then((answer) => {
          if (answer.error) return setError(answer.error)
          const kept = answer.data
          if (!kept) return
          // Drawn from the blob, not kept as a second copy of it in text.
          if (picture) {
            const shown = URL.createObjectURL(file)
            setPreviews((was) => ({ ...was, [kept.path]: shown }))
          }
          setFiles((was) => [...was, kept])
        })
      }
      reader.readAsDataURL(file)
    },
    [project],
  )

  const keep = useCallback((file: Attachment) => {
    setFiles((was) => (was.some((had) => had.path === file.path) ? was : [...was, file]))
  }, [])

  const detach = useCallback((path: string) => {
    setFiles((was) => was.filter((file) => file.path !== path))
    setPreviews((was) => {
      if (!(path in was)) return was
      URL.revokeObjectURL(was[path]!)
      const { [path]: _gone, ...rest } = was
      return rest
    })
  }, [])

  const rewind = useCallback(
    (turnId: string) => {
      if (!project) return
      void ask(() => commands.chatRewind(project.id, conversationId, turnId)).then((forked) => {
        if (forked.data) show('chat', { id: forked.data })
        else setError(forked.error)
      })
    },
    [project, conversationId, show],
  )

  const editFrom = useCallback(
    (turnId: string, putBack: boolean) => {
      if (!project) return
      const said = messages.get().find((one) => one.role === 'user' && one.turnId === turnId)
      void ask(() => commands.chatEditFrom(project.id, conversationId, turnId, putBack)).then((forked) => {
        if (forked.data) show('chat', { id: forked.data, draft: said ? saidText(said) : undefined })
        else setError(forked.error)
      })
    },
    [project, conversationId, show, messages],
  )

  const stop = useCallback(() => {
    /* The turn's own end clears `sending`: the send button must not come back under a turn still going. */
    setStopped(true)
    void ask(() => commands.chatCancel(conversationId))
  }, [conversationId])
  /* A stop not heeded — a hung tool, a process deaf to its signal — is offered a harder
     one, counted from the first press: pressing again does not start the wait over. */
  useEffect(() => {
    if (!sending || !stopped) return
    const timer = window.setTimeout(() => setLate(true), FORCE_AFTER_MS)
    return () => window.clearTimeout(timer)
  }, [sending, stopped])
  const force = useCallback(() => {
    setForced(true)
    void ask(() => commands.chatForceStop(conversationId))
  }, [conversationId])

  const steer = useCallback(
    async (prompt: string, given?: Extras): Promise<boolean> => {
      if (!project) return false
      const extras = given ?? { files, skills }
      const text = withSkills(withFiles(prompt, extras.files), extras.skills)
      const said = await ask(() => commands.chatSteer(project.id, conversationId, text))
      if (said.data) {
        sent.current.add(said.data.id)
        setMessages((was) => steered(was, said.data!))
        if (!given) clearExtras()
      }
      return said.data !== null
    },
    [project, conversationId, files, skills, clearExtras, setMessages],
  )

  /* Another account is another installation, whose skills are not these:
     a pill picked on one would name nothing on the other. */
  const pick = useCallback(
    (next: string) => {
      if (next !== profileId) setSkills([])
      setProfileId(next)
    },
    [profileId],
  )
  const chooseEffort = useCallback(
    (next: string) => {
      setEffort(next)
      remember(conversationKey(conversationId), { effort: next })
      if (profileId) remember(profileId, { effort: next })
    },
    [conversationId, profileId],
  )
  /* In an orchestrator the one account it belongs to is the only choice. */
  const offered = useMemo(
    () => (project?.orchestrator ? profiles.filter((one) => one.id === project.orchestrator) : profiles),
    [project?.orchestrator, profiles],
  )
  const forceable = sending && stopped && late && !forced

  /* One object while nothing in it changed: the composer is memoised on it. */
  return useMemo(
    () => ({
      messages,
      card,
      profiles: offered,
      fixed,
      profileId,
      model,
      cost,
      context,
      permission,
      effort,
      folder,
      files,
      asked,
      sending,
      failed,
      error,
      pick,
      setModel,
      setPermission: choosePermission,
      setEffort: chooseEffort,
      attach,
      paste,
      keep,
      previews,
      skills,
      setSkills,
      session,
      older,
      took,
      rewindable,
      rewind,
      editFrom,
      detach,
      answer,
      say,
      steer,
      clearExtras,
      stop,
      forceable,
      force,
    }),
    [messages, card, offered, fixed, profileId, model, cost, context, permission, effort, folder, files, asked, sending, failed, error, pick, choosePermission, chooseEffort, attach, paste, keep, previews, skills, session, older, took, rewindable, rewind, editFrom, detach, answer, say, steer, clearExtras, stop, forceable, force],
  )
}
