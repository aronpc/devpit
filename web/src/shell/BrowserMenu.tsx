import { useEffect, useState } from 'react'

import type { Kept, Store, Taken } from '../gen/bindings'
import { type Family, grouped, hostOf, type Viewport, viewportLabel, VIEWPORTS } from './browsing'
import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * Everything a browser pane can be told to do, in one panel.
 *
 * There were three controls before this: a key that opened an import form, a
 * chip that opened a session list, and sign-out inside the import panel next
 * to the routine action. Each was reasonable alone and together they were a
 * bar with three menus in it and no idea which held what.
 *
 * **This draws the panel and nothing around it.** It has no trigger and no
 * open state, because it does not live in the pane's document: a pane's page
 * is a second native webview and two native webviews have no z-order between
 * them, so a panel drawn beside the pane came out *behind the site*.
 * `BrowserMenuWindow` renders this in a window of its own, which the operating
 * system stacks for us. The trigger is a plain button back in the bar.
 *
 * The order is a browser's, and each rule in it separates things that do
 * different kinds of damage:
 *
 *   ✓ default                which session this pane is in
 *   ✓ work
 *   ───
 *   + New session…
 *   ───
 *   ⇩ Bring a signed-in session  ▸   browser ▸ profile
 *   ───
 *   ▭ Page width                 ▸
 *   ───
 *     Let an agent drive this page
 *   ───
 *     Sign out of “default”          the one with no undo, on its own
 *
 * **The import cascade asks browser first, then profile.** They arrive from
 * Rust as two fields for exactly this: somebody looking for a login thinks
 * *the Chrome one*, then *my work Chrome*, and a flat list of
 * `Google Chrome · Profile 1` makes them read the browser name five times to
 * find the profile.
 *
 * **What an import takes is said before it is offered, not after.** Picking a
 * profile brings that profile's cookies — all of them, which is what makes the
 * feature work at all and is also more than somebody may mean. The row says
 * so, and the field above it narrows to one site for anybody who wants that.
 */

function Flyout({
  label,
  what,
  children,
}: {
  label: string
  what?: string
  children: React.ReactNode
}): React.JSX.Element {
  const [open, setOpen] = useState(false)

  return (
    <div
      className="bmenu__has"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <button
        className="bmenu__i"
        type="button"
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        <span className="bmenu__b">
          <span className="bmenu__n">{label}</span>
          {what && <span className="bmenu__w">{what}</span>}
        </span>
        <svg className="bmenu__v" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6" /></svg>
      </button>
      {/* Always leftwards. The panel is anchored to the right of its window
          and every bit of empty room is on its left, which is what that window
          is oversized for — a flyout opening rightwards would be outside the
          window, and a window cannot draw outside itself. */}
      {open && (
        <div className="bmenu__sub" role="menu" aria-label={label}>
          {children}
        </div>
      )}
    </div>
  )
}

function Row({
  label,
  what,
  chosen,
  tone,
  disabled,
  onPick,
}: {
  label: string
  what?: string
  chosen?: boolean
  tone?: 'grave'
  disabled?: boolean
  onPick: () => void
}): React.JSX.Element {
  return (
    <button
      className={tone === 'grave' ? 'bmenu__i bmenu__i--grave' : 'bmenu__i'}
      type="button"
      role={chosen === undefined ? 'menuitem' : 'menuitemradio'}
      aria-checked={chosen}
      disabled={disabled}
      onClick={onPick}
    >
      <span className="bmenu__b">
        <span className="bmenu__n">{label}</span>
        {what && <span className="bmenu__w">{what}</span>}
      </span>
      {chosen && (
        <svg className="bmenu__c" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round"><path d="m5 13 4 4L19 7" /></svg>
      )}
    </button>
  )
}

export function BrowserMenu({
  pane,
  at,
  session,
  viewport,
  granted,
  onSession,
  onViewport,
  onGrant,
  onSaid,
  onDone,
}: {
  pane: string
  at: string
  session: string
  viewport: Viewport | null
  /* Whether an agent may drive this page. It was a lit chip in the bar, beside
     a chip that only ever printed the session's name — two controls holding
     the bar open for one switch and one label. Both belong in here: the
     session is already a row with a tick on it, and letting something else
     type into a page you are signed into is a decision, not a toolbar. */
  granted: boolean
  onSession: (session: string) => void
  onViewport: (viewport: Viewport | null) => void
  onGrant: (granted: boolean) => void
  /* What happened, reported into the pane's own strip. This panel is a window
     of its own and it closes on the click that caused the report, so a notice
     left here would be a notice nobody sees. */
  onSaid: (said: string) => void
  /* Put the panel away. In the menu window that hides the window; there is
     nothing else it could mean, because the panel is all the window holds. */
  onDone: () => void
}): React.JSX.Element {
  const [sessions, setSessions] = useState<readonly string[]>([])
  const [naming, setNaming] = useState<string | null>(null)
  const [families, setFamilies] = useState<readonly Family[] | null>(null)
  const [holding, setHolding] = useState<Kept | null>(null)
  const [only, setOnly] = useState('')
  const [refused, setRefused] = useState<string | null>(null)

  /* Asked when the panel appears and not before. Listing cookie stores walks
     other programs' profile directories, which macOS treats as their private
     data and prompts about — asking on the pane's own mount would prompt for
     a menu nobody opened. */
  useEffect(() => {
    setRefused(null)
    void ask(() => commands.browserSessions()).then((answer) => {
      if (answer.data) setSessions(answer.data)
    })
    void ask(() => commands.browserSessionHeld(session)).then((answer) => {
      if (answer.data) setHolding(answer.data)
    })
    void ask(() => commands.browserStores()).then((answer) => {
      setFamilies(grouped((answer.data ?? []) as Store[]))
      if (answer.error !== null) setRefused(answer.error)
    })
  }, [session])

  /* The page you are on, which is the site you almost always mean. Offered,
     never assumed: leaving it empty is what brings the profile whole. */
  useEffect(() => setOnly((was) => was || hostOf(at)), [at])

  const bring = (path: string, from: string): void => {
    setRefused(null)
    const domains = only.trim() ? [only.trim()] : []
    void ask(() => commands.browserImport(pane, path, domains)).then((answer) => {
      if (answer.error !== null) return setRefused(answer.error)
      const taken = answer.data as Taken
      onSaid(
        domains.length
          ? t('workspace.browserMenu.broughtFor', { count: taken.count, from, domains: domains.join(', ') })
          : t('workspace.browserMenu.brought', { count: taken.count, from }),
      )
      onDone()
    })
  }

  const make = (): void => {
    const name = (naming ?? '').trim()
    if (!name) return
    setNaming(null)
    onDone()
    /* Nothing is created on disk here. A session *is* its directory, and the
       directory is made by the webview that opens in it — so a name that is
       never used costs nothing and leaves nothing behind. */
    onSession(name)
  }

  const forget = (): void => {
    void ask(() => commands.browserSessionForget(session)).then((answer) => {
      if (answer.error !== null) return setRefused(answer.error)
      setHolding(null)
      onDone()
      onSaid(t('workspace.browserMenu.signedOut', { session }))
      onSession(session)
    })
  }

  const kb = Math.round((holding?.bytes ?? 0) / 1024)
  const named = Array.from(new Set([...sessions, session]))

  return (
    <div className="bmenu__panel" role="menu" aria-label={t('workspace.browserMenu.panel')}>
      {named.map((one) => (
        <Row
          key={one}
          label={one}
          chosen={one === session}
          onPick={() => {
            onDone()
            if (one !== session) onSession(one)
          }}
        />
      ))}

      <div className="bmenu__rule" />

      {naming === null ? (
        <Row label={t('workspace.browserMenu.newSession')} onPick={() => setNaming('')} />
      ) : (
        <form
          className="bmenu__new"
          onSubmit={(event) => {
            event.preventDefault()
            make()
          }}
        >
          <input
            className="bmenu__in"
            aria-label={t('workspace.browserMenu.nameSession')}
            placeholder={t('workspace.browserMenu.nameExample')}
            spellCheck={false}
            autoFocus
            value={naming}
            onChange={(event) => setNaming(event.target.value)}
          />
          <button className="bmenu__ok" type="submit" disabled={!naming.trim()}>
            {t('workspace.browserMenu.make')}
          </button>
        </form>
      )}

      <div className="bmenu__rule" />

      <Flyout
        label={t('workspace.browserMenu.bring')}
        what={only.trim() ? t('workspace.browserMenu.cookiesFor', { site: only.trim() }) : t('workspace.browserMenu.everyCookie')}
      >
        <label className="bmenu__only">
          <span className="bmenu__k">{t('workspace.browserMenu.only')}</span>
          <input
            className="bmenu__in"
            aria-label={t('workspace.browserMenu.limitImport')}
            placeholder={t('workspace.browserMenu.everySite')}
            spellCheck={false}
            value={only}
            onChange={(event) => setOnly(event.target.value)}
          />
        </label>
        <div className="bmenu__rule" />

        {families === null && <p className="bmenu__note">{t('workspace.browserMenu.looking')}</p>}
        {families?.length === 0 && (
          <p className="bmenu__note">{t('workspace.browserMenu.noProfiles')}</p>
        )}
        {families?.map((one) =>
          one.profiles.length > 1 ? (
            <Flyout key={one.family} label={t('workspace.browserMenu.from', { family: one.family })}>
              {one.profiles.map((profile) => (
                <Row
                  key={profile.path}
                  label={profile.name}
                  what={profile.warning || undefined}
                  onPick={() => bring(profile.path, `${one.family} · ${profile.name}`)}
                />
              ))}
            </Flyout>
          ) : (
            <Row
              key={one.family}
              label={t('workspace.browserMenu.from', { family: one.family })}
              what={one.profiles[0]?.warning || undefined}
              onPick={() => bring(one.profiles[0]?.path ?? '', one.family)}
            />
          ),
        )}

        <div className="bmenu__rule" />
        {/* Said where the choice is made. A profile is read whole — the file
            gives no cheaper way — and a person bringing their GitHub login
            over did not thereby mean their bank, which is in the same file.
            The field above is how they say so. */}
        <p className="bmenu__note">
          {t('workspace.browserMenu.profileNote')}
        </p>
      </Flyout>

      <div className="bmenu__rule" />

      <Flyout label={t('workspace.browserMenu.pageWidth')} what={viewport ? viewportLabel(viewport) : t('workspace.browserMenu.wholePane')}>
        <Row
          label={t('workspace.browserMenu.wholePane')}
          chosen={viewport === null}
          onPick={() => {
            onViewport(null)
            onDone()
          }}
        />
        <div className="bmenu__rule" />
        {VIEWPORTS.map((one) => (
          <Row
            key={one.id}
            label={viewportLabel(one)}
            what={`${one.width} × ${one.height}`}
            chosen={viewport?.id === one.id}
            onPick={() => {
              onViewport(one)
              onDone()
            }}
          />
        ))}
        <div className="bmenu__rule" />
        {/* The honest half. A device toolbar also sends a phone's user agent
            and reports touch, and both need an engine hook this webview does
            not have. */}
        <p className="bmenu__note">
          {t('workspace.browserMenu.widthNote')}
        </p>
      </Flyout>

      <div className="bmenu__rule" />

      <Row
        label={t('workspace.browserMenu.drive')}
        what={
          granted
            ? t('workspace.browserMenu.driveOn')
            : t('workspace.browserMenu.driveOff')
        }
        chosen={granted}
        onPick={() => {
          onGrant(!granted)
          onDone()
        }}
      />

      <div className="bmenu__rule" />

      <Row
        label={t('workspace.browserMenu.signOut', { session })}
        what={
          holding?.used === true
            ? t('workspace.browserMenu.signOutUsed', { kb })
            : t('workspace.browserMenu.signOutEmpty')
        }
        tone="grave"
        disabled={holding?.used !== true}
        onPick={forget}
      />

      {/* A refusal stays in the panel, beside the thing that was refused.
          Nothing covers it: the panel is the whole window. */}
      {refused && (
        <p className="bmenu__said" role="alert">
          {refused}
        </p>
      )}
    </div>
  )
}
