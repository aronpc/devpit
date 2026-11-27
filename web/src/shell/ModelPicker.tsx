import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { AgentGlyph } from './AgentGlyph'
import { useAway } from './away'
import { accountHint, current, keyOf, offerOf, rowsFor, SHELF, shownName, stepTab, type Row } from './modelPick'
import { modelBlurb, modelHint } from './models'
import { installationOf } from './outside'
import { committed } from './typing'
import { counted, useAccountInfo } from './useAccountInfo'
import { useInstallations } from './useInstallations'
import { useShell } from './useShell'
import type { Profile } from '../gen/bindings'
import { intoView } from './intoView'
import { t } from '../i18n'
import { Icon, iconSize } from './Icon'

/*
 * Which account and model this conversation runs on.
 *
 * One picker for both, because they are one choice: a model belongs to an
 * account, and picking a model from another account would be changing the
 * account. Once the conversation has spoken the account is fixed, and only
 * that account's models are offered.
 *
 * The rail names each account and says what makes it that account — two
 * profiles of one CLI wear the same mark, so a mark and a letter was a control
 * nobody could read. The foot says where the account keeps its history,
 * skills and MCP servers, which is what changes when the rail does.
 */

const FAVOURITES = 'devpit.favourite-models'

export function ModelPicker({
  profiles,
  profileId,
  model,
  fixed,
  onPick,
}: {
  profiles: readonly Profile[]
  profileId: string | null
  model: string | null
  /** The account this conversation cannot leave, once it has one. */
  fixed: string | null
  onPick: (profileId: string, model: string) => void
}): React.JSX.Element {
  const { project, openPrefs } = useShell()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [at, setAt] = useState(0)
  const [tab, setTab] = useState<string>(profileId ?? SHELF)
  const [stars, setStars] = useState<readonly string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem(FAVOURITES) ?? '[]') as string[]
    } catch {
      return []
    }
  })
  const box = useRef<HTMLDivElement>(null)
  const field = useRef<HTMLInputElement>(null)
  useAway(box, useCallback(() => setOpen(false), []), open)
  const { list: installations } = useInstallations()

  const usable = useMemo(
    () => (fixed ? profiles.filter((one) => one.id === fixed) : profiles),
    [fixed, profiles],
  )

  useEffect(() => {
    if (!open) return
    setQuery('')
    setAt(0)
    setTab(profileId ?? usable[0]?.id ?? SHELF)
    field.current?.focus()
  }, [open, profileId, usable])

  const rows = rowsFor(usable, tab, query, stars)
  const browsing = !query && tab !== SHELF
  const viewed = usable.find((one) => one.id === (browsing ? tab : profileId))
  const info = useAccountInfo(
    open ? (installationOf(viewed, installations)?.directory ?? null) : null,
    project?.id ?? null,
  )

  const star = (row: Row): void =>
    setStars((was) => {
      const next = was.includes(keyOf(row))
        ? was.filter((one) => one !== keyOf(row))
        : [...was, keyOf(row)]
      try {
        localStorage.setItem(FAVOURITES, JSON.stringify(next))
      } catch {
        /* A favourite that cannot be saved is still a favourite this session. */
      }
      return next
    })

  const choose = (row: Row | undefined): void => {
    /* A model its agent refused stays listed, with why, but is not picked. */
    if (!row || offerOf(row.profile, row.model)?.unavailable) return
    setOpen(false)
    onPick(row.profile.id, row.model)
  }

  const goTo = (next: string): void => {
    setTab(next)
    setQuery('')
    setAt(0)
  }

  const here = profiles.find((one) => one.id === profileId)
  const on = current(here, model)
  const name = here ? shownName(here, on) : t('sessions.model.noCli')
  /* The account on the chip only when there is more than one to confuse it
     with: a lone account named on every chip is noise. */
  const label = here && profiles.length > 1 ? `${here.label} · ${name}` : name

  return (
    <div className="ctl" ref={box}>
      <button
        className="chip"
        aria-label={t('sessions.model.dialog')}
        aria-expanded={open}
        title={here ? `${here.label} · ${modelHint(on, here.env ?? [])}` : undefined}
        onClick={() => setOpen((was) => !was)}
      >
        <span className="chip__sun"><AgentGlyph agent={here?.id ?? ''} base={here?.base || here?.driver} /></span>
        <span className="ctl__l">{label}</span>
      </button>

      {open && (
        <div className="mpick" role="dialog" aria-label={t('sessions.model.dialog')}>
          <nav className="mpick__rail" aria-label={t('sessions.model.accounts')}>
            <button
              className="mpick__tab"
              aria-pressed={tab === SHELF && !query}
              data-on={tab === SHELF && !query}
              onClick={() => goTo(SHELF)}
            >
              <span className="mpick__ti"><Star on={false} size={14} /></span>
              <span className="mpick__tb"><span className="mpick__tn">{t('sessions.model.favourites')}</span></span>
            </button>
            <span className="mpick__sep" />
            {usable.map((profile) => (
              <button
                className="mpick__tab"
                key={profile.id}
                aria-pressed={tab === profile.id && !query}
                data-on={tab === profile.id && !query}
                title={`${profile.label} — ${profile.command}`}
                onClick={() => goTo(profile.id)}
              >
                <span className="mpick__ti"><AgentGlyph agent={profile.id} base={profile.base || profile.driver} /></span>
                <span className="mpick__tb">
                  <span className="mpick__tn">{profile.label}</span>
                  <span className="mpick__ts">{accountHint(profile)}</span>
                </span>
                {profile.id === profileId && <span className="mpick__live" aria-label={t('sessions.model.thisAccount')} />}
              </button>
            ))}
            {/* Where another account comes from, said where it is missed. */}
            <button className="mpick__add" onClick={() => (setOpen(false), openPrefs('providers'))}>
              {usable.length > 1 || fixed ? t('sessions.model.manage') : t('sessions.model.add')}
            </button>
          </nav>

          <div className="mpick__main">
            <div className="mpick__q">
              <Icon size="s"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></Icon>
              <input
                ref={field}
                className="mpick__in"
                placeholder={fixed ? t('sessions.model.searchOne') : t('sessions.model.searchAll')}
                value={query}
                spellCheck={false}
                aria-label={t('sessions.model.search')}
                onChange={(event) => {
                  setQuery(event.target.value)
                  setAt(0)
                }}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                    event.preventDefault()
                    const step = event.key === 'ArrowDown' ? 1 : -1
                    setAt((was) => (was + step + rows.length) % Math.max(rows.length, 1))
                  } else if (!query && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
                    /* Along the rail with the field still focused, so the
                       arrows and Enter never have to leave the keyboard. */
                    event.preventDefault()
                    goTo(stepTab(usable, tab, event.key === 'ArrowRight' ? 1 : -1))
                  } else if (committed(event)) {
                    event.preventDefault()
                    choose(rows[at])
                  }
                }}
              />
            </div>

            <div className="mpick__list" role="listbox" aria-label={t('sessions.model.models')}>
              {rows.length === 0 && (
                <div className="mpick__none">
                  {query
                    ? t('sessions.model.noMatch')
                    : tab === SHELF
                      ? t('sessions.model.starOne')
                      : t('sessions.model.noneReported')}
                </div>
              )}
              {rows.map((row, index) => {
                const env = row.profile.env ?? []
                const mine = row.profile.id === profileId && row.model === on
                const starred = stars.includes(keyOf(row))
                const refused = offerOf(row.profile, row.model)?.unavailable ?? null
                return (
                  <div
                    className="mpick__r"
                    key={keyOf(row)}
                    role="option"
                    aria-selected={mine}
                    aria-disabled={refused ? true : undefined}
                    data-unavailable={refused ? '' : undefined}
                    data-at={index === at}
                    ref={index === at ? intoView : undefined}
                    tabIndex={0}
                    onMouseEnter={() => setAt(index)}
                    onClick={() => choose(row)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault()
                        choose(row)
                      }
                    }}
                  >
                    <span className="mpick__check" aria-hidden="true">{mine ? '✓' : ''}</span>
                    {/* One line: the name, what it is for, and — across
                        accounts — whose. The alias the CLI is passed is on
                        hover, for anyone checking what is actually sent. */}
                    <span className="mpick__b" title={modelHint(row.model, env)}>
                      <span className="mpick__n">{shownName(row.profile, row.model)}</span>
                      <span className="mpick__p">
                        {!browsing && <span className="mpick__who">{row.profile.label}</span>}
                        {refused ?? modelBlurb(row.model, env)}
                      </span>
                    </span>
                    <span
                      className="mpick__star"
                      role="button"
                      tabIndex={0}
                      aria-label={starred ? t('sessions.model.unstar') : t('sessions.model.star')}
                      title={starred ? t('sessions.model.unstar') : t('sessions.model.star')}
                      data-on={starred}
                      onClick={(event) => {
                        event.stopPropagation()
                        star(row)
                      }}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault()
                          event.stopPropagation()
                          star(row)
                        }
                      }}
                    >
                      <Star on={starred} size={13} />
                    </span>
                  </div>
                )
              })}
            </div>

            {viewed?.driver === 'claude' && installations.length > 0 && (
              <div className="mpick__foot" title={info?.directory}>
                {info ? (
                  <>
                    <span className="mpick__dir">{info.directory.replace(/^\/(home|Users)\/[^/]+/, '~')}</span>
                    {counted(info) && <span>{counted(info)}</span>}
                  </>
                ) : (
                  <span>{t('sessions.model.noDirectory', { account: viewed.label })}</span>
                )}
              </div>
            )}

          </div>
        </div>
      )}
    </div>
  )
}

const Star = ({ on, size }: { on: boolean; size: number }): React.JSX.Element => (
  <Icon size={iconSize(size)} filled={on} aria-hidden="true">
    <path d="m12 3 2.6 5.6 5.9.8-4.3 4.2 1.1 6-5.3-3-5.3 3 1.1-6L3.5 9.4l5.9-.8Z" />
  </Icon>
)
