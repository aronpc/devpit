import { useCallback, useEffect, useState } from 'react'

import type { ChannelRules, Channels } from '../gen/bindings'
import { clock, ESCALATE_CHOICES, escalated, EVENTS, flipped, minutes, offsetNow } from './channelRules'
import { ask, commands } from './live'
import { PlusLink } from './PlusLink'
import { HubSetup } from './HubSetup'
import { TelegramSetup } from './TelegramSetup'
import { t } from '../i18n'

const DAYS = [0, 1, 2, 3, 4, 5, 6]

/*
 * Where devpit tells you things while you are away from it, and which things.
 * Nothing is sent while devpit's window is in front or devpit is paused.
 */
export function ChannelsPane(): React.JSX.Element {
  const [channels, setChannels] = useState<Channels | null>(null)
  const load = useCallback(() => void ask(() => commands.channelsRead()).then((answer) => setChannels(answer.data)), [])
  useEffect(load, [load])

  const save = (rules: ChannelRules): void => {
    const kept = rules.quiet ? { ...rules, quiet: { ...rules.quiet, offsetMinutes: offsetNow() } } : rules
    void ask(() => commands.channelsRulesSet(kept)).then((answer) => answer.data && setChannels(answer.data))
  }

  const rules = channels?.rules
  const connected = channels?.connected ?? []
  return (
    <>
      <h1 className="prefs__h">{t('workspace.channels.title')}</h1>
      <TelegramSetup onLinked={load} />
      <HubSetup onChanged={load} />
      <p className="acc__note">
        <PlusLink from="app-channels" said={t('workspace.channels.plus')} />
      </p>

      {rules && connected.length > 0 && (
        <>
          <div className="acc__sub">{t('workspace.channels.whatWhere')}</div>
          <table className="chmatrix">
            <thead>
              <tr>
                <th />
                {connected.map((one) => (
                  <th key={one.id}>{one.label}</th>
                ))}
                <th>{t('workspace.channels.ifNobody')}</th>
              </tr>
            </thead>
            <tbody>
              {EVENTS.map(([event, label]) => (
                <tr key={event}>
                  <td>{label}</td>
                  {connected.map((one) => (
                    <td key={one.id}>
                      <input
                        type="checkbox"
                        aria-label={t('workspace.channels.on', { label, channel: one.label })}
                        checked={rules.routes.some((route) => route.event === event && route.channels.includes(one.id))}
                        onChange={(change) => save(flipped(rules, event, one.id, change.target.checked))}
                      />
                    </td>
                  ))}
                  <td>
                    {channels.escalates.includes(event) && (
                      <select
                        aria-label={t('workspace.channels.ifNobodyLabel', { label })}
                        value={String(rules.routes.find((route) => route.event === event)?.escalateAfter ?? '')}
                        onChange={(change) => save(escalated(rules, event, change.target.value ? Number(change.target.value) : null))}
                      >
                        {ESCALATE_CHOICES.map((choice) => (
                          <option key={String(choice)} value={choice ?? ''}>
                            {choice === null ? t('workspace.channels.everyChannel') : t('workspace.channels.nextChannel', { minutes: choice })}
                          </option>
                        ))}
                      </select>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="acc__note">{t('workspace.channels.delayNote')}</p>

          <div className="acc__sub">{t('workspace.channels.quiet')}</div>
          <div className="pref">
            <span className="pref__body">
              <label className="voice__row">
                <input type="checkbox" checked={Boolean(rules.quiet)} onChange={(change) => save({ ...rules, quiet: change.target.checked ? { from: 22 * 60, to: 7 * 60, days: [], offsetMinutes: offsetNow() } : null })} />
                {t('workspace.channels.sendNothing')}
              </label>
              {rules.quiet && (
                <span className="voice__row">
                  <input type="time" aria-label={t('workspace.channels.quietFrom')} value={clock(rules.quiet.from)} onChange={(change) => rules.quiet && save({ ...rules, quiet: { ...rules.quiet, from: minutes(change.target.value) } })} />
                  {t('workspace.channels.to')}
                  <input type="time" aria-label={t('workspace.channels.quietUntil')} value={clock(rules.quiet.to)} onChange={(change) => rules.quiet && save({ ...rules, quiet: { ...rules.quiet, to: minutes(change.target.value) } })} />
                  {DAYS.map((day, at) => (
                    <label key={day} className="chday">
                      <input
                        type="checkbox"
                        checked={rules.quiet?.days.includes(at) ?? false}
                        onChange={(change) =>
                          rules.quiet && save({ ...rules, quiet: { ...rules.quiet, days: change.target.checked ? [...rules.quiet.days, at] : rules.quiet.days.filter((one) => one !== at) } })
                        }
                      />
                      {t(`workspace.channels.day.${day}`)}
                    </label>
                  ))}
                </span>
              )}
              <span className="pref__d">{t('workspace.channels.noDay')}</span>
            </span>
          </div>

          <div className="pref">
            <span className="pref__body">
              <span className="pref__t">{t('workspace.channels.gather')}</span>
              <span className="pref__d">{t('workspace.channels.gatherNote')}</span>
            </span>
            <select value={rules.groupSeconds} onChange={(change) => save({ ...rules, groupSeconds: Number(change.target.value) })}>
              {[15, 60, 120, 300].map((seconds) => (
                <option key={seconds} value={seconds}>
                  {seconds < 60 ? t('workspace.channels.seconds', { count: seconds }) : t('workspace.channels.minutes', { count: seconds / 60 })}
                </option>
              ))}
            </select>
          </div>
        </>
      )}
    </>
  )
}
