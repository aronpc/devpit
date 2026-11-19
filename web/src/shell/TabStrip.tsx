import { useEffect, useLayoutEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { AgentMark } from './AgentMark'
import { HostBadge } from './HostBadge'
import { paneMeta } from './paneList'
import { Rename } from './Rename'
import { busyIn, doingIn } from './running'
import { tabTitle, twice, type Clicked, type Tab } from './strip'
import { unreadIn } from './unread'
import { useShell } from './useShell'
import { useKeys, useShortcut } from './useShortcut'

/*
 * The strip is the order. A tab is appended when it opens and only moves when
 * it is dragged — clicking one must never reshuffle the row under the pointer.
 *
 * Pointer events rather than HTML5 drag-and-drop, which cannot be styled and
 * fires a ghost image nobody asked for.
 */
export function TabStrip(): React.JSX.Element {
  const { open, active, focus, close, move, rename, renaming, setRenaming, reopen } = useShell()
  const { running, doing, unread, openPalette, mark, project } = useShell()
  useShortcut('reopenTab', () => reopen())
  useShortcut('goToTab', (event) => {
    const tab = open[Number(/\d$/.exec(event.code || event.key)?.[0] ?? 0) - 1]
    if (!tab) return false
    focus(tab.id)
  })
  const keys = useKeys()
    const strip = useRef<HTMLDivElement>(null)
  const before = useRef(new Map<string, number>())
  const moving = useRef<{ id: string; at: number; moved: boolean } | null>(null)
  // State as well as the ref: the strip only redraws as `grabbing` when this changes.
  const [dragging, setDragging] = useState<string | null>(null)
  const clicked = useRef<Clicked | null>(null)

  /* FLIP: the tabs that did not move animate from where they were. */
  useLayoutEffect(() => {
    const row = strip.current
    if (!row) return
    const calm = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const here = new Set(open.map((tab) => tab.id))
    for (const id of before.current.keys()) {
      if (!here.has(id)) before.current.delete(id)
    }
    for (const el of Array.from(row.children) as HTMLElement[]) {
      const id = el.dataset.tab ?? ''
      const was = before.current.get(id)
      const now = el.getBoundingClientRect().left
      before.current.set(id, now)
      if (was === undefined || was === now || calm) continue
      el.animate([{ transform: `translateX(${was - now}px)` }, { transform: 'none' }], {
        duration: 170,
        easing: 'cubic-bezier(0.2, 0.7, 0.3, 1)',
      })
    }
  }, [open])

  function onMove(event: React.PointerEvent): void {
    const drag = moving.current
    const row = strip.current
    if (!drag || !row) return
    if (Math.abs(event.clientX - drag.at) <= 4) return
    if (!drag.moved) setDragging(drag.id)
    drag.moved = true
    const tabs = Array.from(row.children) as HTMLElement[]
    const to = tabs.findIndex((el) => {
      const box = el.getBoundingClientRect()
      return event.clientX < box.left + box.width / 2
    })
    move(drag.id, to === -1 ? tabs.length - 1 : to)
  }

  /* Long enough to tell two chats apart, short enough that six tabs still
     fit across the strip. */
  /* The whole name. It used to be cut at 22 characters here, which cut it
     whether or not there was room — with one tab open and half the window
     free, `App.tsx · very-long-branch` still lost its tail. The strip now
     trims with CSS, which trims only when it has to. */
  const label = named

  /* What the tab is called, which is not always what it was named.

     A terminal is born `Terminal 3`, which says where it is in the strip and
     nothing about what is in it. Once an agent is open, the agent is the
     truer answer — so the generated name gives way and a name the person
     typed never does. */
  function named(tab: Tab): string {
    if (tab.title && !generated(tab)) return tab.title
    const here = busyIn(running, tab)
    return here?.agent ? here.label : (tabTitle(tab) ?? paneMeta(tab.kind).label)
  }

  /* A name this app wrote, rather than one a person chose. */
  const generated = (tab: Tab): boolean =>
    tab.kind === 'term' && /^Terminal \d+$/.test(tab.title ?? '')

  /* The glyph: the agent's while one is open, and the pane's otherwise.

     `data-doing` carries what the agent says about itself, which is the one
     thing the process table cannot answer — an agent blocked on the network
     and an agent blocked on you look identical from outside. */
  function icon(tab: Tab): React.ReactNode {
    const here = busyIn(running, tab)
    if (!here?.agent) return paneMeta(tab.kind).icon
    return (
      <span className="tab__mk" data-doing={doingIn(running, doing, tab) ?? undefined}>
        <AgentMark agent={here.agent} />
      </span>
    )
  }

  /* Picked from the palette or with ⌘K, the tab that becomes active can be
     one the strip has scrolled past. Selecting something you cannot see is
     the failure the scrolling introduced, so it is undone here. */
  useEffect(() => {
    if (!active) return
    strip.current
      ?.querySelector(`[data-tab="${CSS.escape(active.id)}"]`)
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [active])

  return (
    <div className="tabs">
      {/* The rail scrolls and the plus does not. `.tabs` was `flex: none`, so
          with eight files open it never shrank and pushed the branch chip and
          the bell off the right edge of the window. A strip that scrolls is
          the limit; the plus stays reachable because it is outside it. */}
      <div
        className="tabs__rail"
        role="group"
        aria-label={t('workspace.tabs.panes')}
        ref={strip}
        data-moving={dragging ? 'true' : undefined}
        onPointerMove={onMove}
        onWheel={(event) => {
          /* A wheel with no sideways travel is somebody scrolling a strip
             that only goes sideways. Every editor does this; without it a
             mouse can only reach the far tabs by dragging. */
          if (event.deltaX !== 0) return
          event.currentTarget.scrollLeft += event.deltaY
        }}
      >
      {open.map((tab) =>
        /* Renaming swaps the whole tab for a field: an input inside a button
           is neither valid nor operable — the button swallows the click that
           would place the cursor. */
        renaming?.id === tab.id && renaming.where === 'strip' ? (
          <div className="tab" data-tab={tab.id} data-ctx="tab" data-id={tab.id} data-toggle={tab.kind} data-active={String(active?.id === tab.id)} key={tab.id}>
            {icon(tab)}
            <Rename
              value={tabTitle(tab) ?? paneMeta(tab.kind).label}
              editing
              onDone={(title) => {
                if (title) rename(tab.id, title)
                /* Kept on the conversation too, so its name outlives the tab. */
                if (title && tab.kind === 'chat') mark(tab.id, { name: title })
                setRenaming(null)
              }}
            />
          </div>
        ) : (
        <button
          key={tab.id}
          className="tab"
          data-tab={tab.id}
          /* Right-click reaches the same menu from either shape of tab. */
          data-ctx="tab"
          data-id={tab.id}
          data-toggle={tab.kind}
          data-active={String(active?.id === tab.id)}
          data-dragging={dragging === tab.id ? 'true' : undefined}
          aria-pressed={true}
          title={label(tab)}
          onPointerDown={(event) => {
            /* The cross is a target of its own: capturing the pointer would
               redirect the click that follows to the tab, and closing would
               quietly become selecting. */
            if (event.button !== 0 || (event.target as HTMLElement).closest('.tab__x')) return
            moving.current = { id: tab.id, at: event.clientX, moved: false }
            event.currentTarget.setPointerCapture(event.pointerId)
          }}
          onPointerUp={() => {
            moving.current = null
            setDragging(null)
          }}
          onClick={(event) => {
            if (moving.current?.moved) return
            if (twice(clicked.current, tab.id, event.timeStamp)) {
              clicked.current = null
              setRenaming({ id: tab.id, where: 'strip' })
              return
            }
            clicked.current = { id: tab.id, at: event.timeStamp }
            focus(tab.id)
          }}
        >
          {icon(tab)}
          <span className="tab__n">{label(tab)}</span>
          {tab.kind === 'term' && project && <HostBadge projectId={project.id} tabId={tab.id} shape={`${tab.regrouped ?? 0}:${tab.panes?.join(',') ?? ''}`} />}
          {/* Something is running here and it is not an agent. The name stays
              the tab's own; the dot is the whole message. */}
          {busyIn(running, tab)?.agent === null && <i className="tab__dot" aria-hidden="true" />}
          {unreadIn(unread, tab) && <i className="tab__new" role="img" aria-label={t('workspace.tabs.unread')} />}
          <span
            className="tab__x"
            role="button"
            aria-label={t('workspace.tabs.close')}
            title={t('workspace.tabs.close')}
            onClick={(event) => {
              event.stopPropagation()
              close(tab.id)
            }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </span>
        </button>
        ),
      )}

      </div>

      {/* Anything new, where every terminal app puts the plus.

          The field rather than a bare terminal: the plus is where a person
          goes to start something, and most of the time the something is an
          agent. A plain terminal is the first row in it, and ⌘T still opens
          one without stopping to ask. */}
      <button className="tab tab--new" title={t('workspace.tabs.newKeys', { keys: keys('palette') })} aria-label={t('workspace.tabs.new')} onClick={openPalette}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round">
          <path d="M12 5v14M5 12h14" />
        </svg>
      </button>
    </div>
  )
}
