import type { Terminal } from '@xterm/xterm'
import { useCallback, useEffect, useRef, useState } from 'react'

import { AgentBar } from './AgentBar'
import { BlockCard } from './BlockCard'
import { CommandInput, type CommandInputHandle } from './CommandInput'
import { recalled, remember, withLine } from './commandHistory'
import { homeFolder } from './homeFolder'
import { Leaf } from './Leaf'
import { ask, commands } from './live'
import { finished, jumpTarget, modeOf } from './paneBlocks'
import { useFollow } from './useFollow'
import { usePaneBlocks } from './usePaneBlocks'
import { useShellPick } from './shellStore'
import { matches } from './shortcuts'
import { t } from '../i18n'

/*
 * A terminal as blocks, the way Warp draws one.
 *
 * Every command the shell ran is a block above — the line, where, how long,
 * how it ended, what it printed — and the next line is typed in an editor
 * below. While a command runs the live terminal is shown under the blocks,
 * and a command that takes the whole screen (an editor, an agent's TUI) gets
 * all of it. The live terminal is the pane itself, unchanged: tmux, its
 * scrollback, everything that works in a plain terminal still does.
 *
 * Only for a shell whose hooks are heard marking its prompts; any other pane
 * is drawn plain, and so is one the person switched back to plain.
 */

const CLASSIC = 'devpit.terminal.classic'

function classicPanes(): ReadonlySet<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(CLASSIC) ?? '[]')
    return new Set(Array.isArray(raw) ? raw.filter((one): one is string => typeof one === 'string') : [])
  } catch {
    return new Set()
  }
}

function saveClassic(paneId: string, on: boolean): void {
  const next = new Set(classicPanes())
  if (on) next.add(paneId)
  else next.delete(paneId)
  try {
    localStorage.setItem(CLASSIC, JSON.stringify([...next]))
  } catch {
    /* Kept for as long as the window is open. */
  }
}


export function BlockTerm({
  paneId,
  projectId,
  onSplit,
  onClosePane,
  onSeparate,
  target,
}: {
  paneId: string
  projectId: string
  /** The pane's target in the layout (`Leaf`). */
  target?: string
  onSplit?: (direction: 'horizontal' | 'vertical') => void
  onClosePane?: () => void
  onSeparate?: () => void
}): React.JSX.Element {
  const { state, clear } = usePaneBlocks(paneId)
  /* Only this pane's entry: the list changes whenever any terminal's command
     does, and every block list re-rendered with it. */
  const front = useShellPick((shell) => shell.running.find((one) => one.paneId === paneId && one.agent) ?? null)
  const agent = front?.label ?? null
  const [wanted, setWanted] = useState(() => !classicPanes().has(paneId))
  const [history, setHistory] = useState(() => recalled(projectId))
  const [homeDir, setHomeDir] = useState<string | null>(null)
  const [cols, setCols] = useState(100)
  const resized = useRef<{ dispose(): void } | null>(null)
  const settle = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(
    () => () => {
      resized.current?.dispose()
      if (settle.current) clearTimeout(settle.current)
    },
    [],
  )
  const terminal = useRef<Terminal | null>(null)
  const input = useRef<CommandInputHandle>(null)
  const mode = modeOf(state, wanted)
  const done = finished(state)
  const last = done[done.length - 1]
  const list = useFollow<HTMLDivElement>(`${done.length}:${mode}`).box
  const [jumped, setJumped] = useState<number | null>(null)

  useEffect(() => {
    void homeFolder().then(setHomeDir)
  }, [])

  /* A pane whose shell has not been heard at a prompt since the app started
     — it was already running — is nudged once to draw one, so it can be
     shown as blocks. Only where the shell is idle in front (`pane.nudge`). */
  const integrated = useRef(state.integrated)
  integrated.current = state.integrated
  useEffect(() => {
    if (!wanted) return
    /* Once per pane and choice: a shell with no hooks would be nudged forever. */
    const later = setTimeout(() => {
      if (!integrated.current) void ask(() => commands.paneNudge(projectId, paneId))
    }, 900)
    return () => clearTimeout(later)
  }, [paneId, projectId, wanted])

  /* The keyboard goes where the work is: the editor at the prompt, the live
     terminal while something runs and may be asking for input. */
  useEffect(() => {
    if (mode === 'idle') input.current?.focus()
    else if (mode === 'running' || mode === 'full') terminal.current?.focus()
  }, [mode])

  /* A line typed straight into the terminal is history too. */
  useEffect(() => {
    const line = last?.command
    if (line) setHistory((was) => (was[0] === line.trim() ? was : withLine(was, line)))
  }, [last?.id, last?.command])

  /* Stable, so a block that did not change skips its render (`memo`). */
  const run = useCallback(
    (line: string): void => {
      setHistory(remember(projectId, line))
      void ask(() => commands.paneSubmit(projectId, paneId, line))
    },
    [projectId, paneId],
  )
  const edit = useCallback((line: string) => input.current?.set(line), [])

  /* Alt+↑/↓ walks the blocks — the bookmarked ones when there are any — from
     wherever the keyboard is except the live terminal, whose programs may
     want those keys themselves. Down past the last goes back to the editor. */
  const jump = (event: React.KeyboardEvent): void => {
    const back = matches('previousCommand', event)
    if (!back && !matches('nextCommand', event)) return
    if (event.target instanceof Element && event.target.closest('.bterm__live')) return
    event.preventDefault()
    event.stopPropagation()
    const next = jumpTarget(done, jumped, back ? -1 : 1)
    setJumped(next)
    const box = list.current
    if (next === null) {
      box?.scrollTo({ top: box.scrollHeight })
      input.current?.focus()
      return
    }
    box?.querySelector(`[data-block-id="${next}"]`)?.scrollIntoView({ block: 'start' })
  }

  const bookmark = useCallback(
    (id: number | null, on: boolean): void => void ask(() => commands.blockBookmark(paneId, id, on)),
    [paneId],
  )

  const classic = (on: boolean): void => {
    saveClassic(paneId, on)
    setWanted(!on)
  }

  return (
    <div className="bterm" data-mode={mode} data-pane-id={paneId} onKeyDownCapture={jump}>
      {/* While a command runs, only blocks that exist take room from it: an
          empty list above a build was half the pane saying nothing. */}
      {(mode === 'idle' || (mode === 'running' && done.length > 0)) && (
        <div className="bterm__list" ref={list} tabIndex={-1}>
          {done.length === 0 && <div className="bterm__empty">{t('workspace.blockTerm.empty')}</div>}
          {done.map((block) => (
            <BlockCard
              key={block.id}
              paneId={paneId}
              block={block}
              cols={cols}
              home={homeDir}
              jumped={jumped === block.id}
              onRerun={run}
              onEdit={edit}
              onBookmark={bookmark}
            />
          ))}
        </div>
      )}
      <div className="bterm__live">
        <Leaf
          paneId={paneId}
          projectId={projectId}
          target={target}
          onSplit={onSplit}
          onClosePane={onClosePane}
          onSeparate={onSeparate}
          cwd={state.cwd}
          onTerminal={(one) => {
            terminal.current = one
            if (!one) return
            setCols(one.cols)
            /* Blocks are drawn at the pane's width, so they rewrap with it —
               once the width settles, not on every column of a drag, which
               drew every block again per step. */
            resized.current?.dispose()
            resized.current = one.onResize((size) => {
              if (settle.current) clearTimeout(settle.current)
              settle.current = setTimeout(() => setCols(size.cols), 150)
            })
          }}
          onBlocks={!wanted ? () => classic(false) : undefined}
        />
      </div>
      {agent && mode !== 'idle' && (
        <AgentBar projectId={projectId} paneId={paneId} agent={agent} agentId={front?.agent ?? null} cwd={state.cwd} home={homeDir} onSent={() => terminal.current?.focus()} />
      )}
      {mode === 'idle' && (
        <CommandInput
          ref={input}
          cwd={state.cwd}
          home={homeDir}
          history={history}
          settled={last?.id}
          onSubmit={run}
          onClear={clear}
          onClassic={() => classic(true)}
        />
      )}
    </div>
  )
}
