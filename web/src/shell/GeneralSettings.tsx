import { AboutSettings } from './AboutSettings'
import { ActivityDefault } from './ActivityPick'
import { ArchiveSettings } from './ArchiveSettings'
import { CalendarSettings } from './CalendarSettings'
import { DeskSettings } from './DeskSettings'
import { DiagnosticsPane } from './DiagnosticsPane'
import { DisplaySettings, LanguageSettings } from './DisplaySettings'
import { ErrorReportsPreview } from './ErrorReportsPreview'
import { IslandSettings } from './IslandSettings'
import { MaintenanceSettings } from './MaintenanceSettings'
import { OpenApps } from './OpenApps'
import { PauseSettings } from './PauseSettings'
import { PrefSwitch } from './PrefSwitch'
import { RemoteSettings } from './RemoteSettings'
import { ScopedSetting } from './ScopedSetting'
import { SessionRelaySettings } from './SessionRelaySettings'
import type { Flag } from './settingsFlags'
import type { PrefsPane } from './shape'
import { UpdateSettings } from './UpdateSettings'
import { ChatKeySettings } from './ViewSettings'
import { VoiceSettings } from './VoiceSettings'

/*
 * General and the panes it was split into. One long General read as one
 * thing, and the rows that mattered were missed; each pane now holds one
 * subject, and `prefsRows` says which row went where.
 */

export function GeneralPanes({
  pane,
  on,
  set,
}: {
  pane: PrefsPane
  on: (field: Flag) => boolean
  set: (field: Flag, next: boolean) => void
}): React.JSX.Element {
  const flip = (field: Flag) => () => set(field, !on(field))

  return (
    <>
      <section className="prefs__in" hidden={pane !== 'general'}>
        <h1 className="prefs__h">General</h1>
        <div className="pref">
          <span className="pref__body"><span className="pref__t">Local by default</span><span className="pref__d">Projects, conversations and settings are kept on this computer.</span></span>
        </div>

        <LanguageSettings />

        <div className="acc__sub">On this desktop</div>
        <DeskSettings />
        <DisplaySettings />
        <OpenApps />

        <div className="acc__sub">Updates and privacy</div>
        <PrefSwitch on={on('automaticUpdates')} onFlip={flip('automaticUpdates')} title="Automatic updates" said="Check in the background and offer to install." />
        <UpdateSettings />
        <PrefSwitch on={on('errorReports')} onFlip={flip('errorReports')} title="Error reports" said={<>Send devpit&rsquo;s own errors, without paths or your work, anonymously and only while devpit sits idle. Turning it off deletes what was kept.</>}>
          {on('errorReports') && <ErrorReportsPreview />}
        </PrefSwitch>
      </section>

      <section className="prefs__in" hidden={pane !== 'notifications'}>
        <h1 className="prefs__h">Notifications</h1>
        <IslandSettings on={on('island')} onFlip={flip('island')} />
        <PauseSettings />
        <PrefSwitch on={on('reminders')} onFlip={flip('reminders')} title="Reminders" said="A card's date goes off at its time, as a banner here and a notification." more="A date with no time goes off at nine that morning. An agent can set one when you ask it to remind you." />
        <PrefSwitch on={on('focusMode')} onFlip={flip('focusMode')} title="Focus mode" said="Unfinished. A door for one project: what arrives from another waits until you come out." />
      </section>

      <section className="prefs__in" hidden={pane !== 'sessions'}>
        <h1 className="prefs__h">Sessions</h1>
        <SessionRelaySettings />
        <ActivityDefault />
        <ChatKeySettings />
        <ScopedSetting field="permissionMode" />
        <PrefSwitch on={on('confirmStop')} onFlip={flip('confirmStop')} title="Ask before stopping a terminal" said={<>A terminal still running something asks before it is closed. Its &ldquo;Don&rsquo;t ask again&rdquo; turns this off; this turns it back on.</>} />
      </section>

      <section className="prefs__in" hidden={pane !== 'voice'}>
        <h1 className="prefs__h">Voice</h1>
        <VoiceSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'remote'}>
        <h1 className="prefs__h">Remote</h1>
        <RemoteSettings />
        <CalendarSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'maintenance'}>
        <h1 className="prefs__h">Maintenance</h1>
        <MaintenanceSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'diagnostics'}>
        <h1 className="prefs__h">Diagnostics</h1>
        {pane === 'diagnostics' && <DiagnosticsPane />}
      </section>

      <section className="prefs__in" hidden={pane !== 'about'}>
        <h1 className="prefs__h">About</h1>
        <AboutSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'archive'}>
        <h1 className="prefs__h">Archive</h1>
        {pane === 'archive' && <ArchiveSettings />}
      </section>
    </>
  )
}
