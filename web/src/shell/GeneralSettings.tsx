import { t } from '../i18n'
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
        <h1 className="prefs__h">{t('prefs.nav.general')}</h1>
        <div className="pref">
          <span className="pref__body"><span className="pref__t">{t('workspace.general.localTitle')}</span><span className="pref__d">{t('workspace.general.localHint')}</span></span>
        </div>

        <LanguageSettings />

        <div className="acc__sub">{t('workspace.general.desktop')}</div>
        <DeskSettings />
        <DisplaySettings />
        <OpenApps />

        <div className="acc__sub">{t('workspace.general.updatesPrivacy')}</div>
        <PrefSwitch on={on('automaticUpdates')} onFlip={flip('automaticUpdates')} title={t('workspace.general.autoUpdates')} said={t('workspace.general.autoUpdatesSaid')} />
        <UpdateSettings />
        <PrefSwitch on={on('errorReports')} onFlip={flip('errorReports')} title={t('workspace.general.errorReports')} said={t('workspace.general.errorReportsSaid')}>
          {on('errorReports') && <ErrorReportsPreview />}
        </PrefSwitch>
      </section>

      <section className="prefs__in" hidden={pane !== 'notifications'}>
        <h1 className="prefs__h">{t('prefs.nav.notifications')}</h1>
        <IslandSettings on={on('island')} onFlip={flip('island')} />
        <PauseSettings />
        <PrefSwitch on={on('reminders')} onFlip={flip('reminders')} title={t('workspace.general.reminders')} said={t('workspace.general.remindersSaid')} more={t('workspace.general.remindersMore')} />
        <PrefSwitch on={on('focusMode')} onFlip={flip('focusMode')} title={t('workspace.general.focus')} said={t('workspace.general.focusSaid')} />
      </section>

      <section className="prefs__in" hidden={pane !== 'sessions'}>
        <h1 className="prefs__h">{t('prefs.nav.sessions')}</h1>
        <SessionRelaySettings />
        <ActivityDefault />
        <ChatKeySettings />
        <ScopedSetting field="permissionMode" />
        <PrefSwitch on={on('confirmStop')} onFlip={flip('confirmStop')} title={t('workspace.general.confirmStop')} said={t('workspace.general.confirmStopSaid')} />
      </section>

      <section className="prefs__in" hidden={pane !== 'voice'}>
        <h1 className="prefs__h">{t('prefs.nav.voice')}</h1>
        <VoiceSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'remote'}>
        <h1 className="prefs__h">{t('prefs.nav.remote')}</h1>
        <RemoteSettings />
        <CalendarSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'maintenance'}>
        <h1 className="prefs__h">{t('prefs.nav.maintenance')}</h1>
        <MaintenanceSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'diagnostics'}>
        <h1 className="prefs__h">{t('prefs.nav.diagnostics')}</h1>
        {pane === 'diagnostics' && <DiagnosticsPane />}
      </section>

      <section className="prefs__in" hidden={pane !== 'about'}>
        <h1 className="prefs__h">{t('prefs.nav.about')}</h1>
        <AboutSettings />
      </section>

      <section className="prefs__in" hidden={pane !== 'archive'}>
        <h1 className="prefs__h">{t('prefs.nav.archive')}</h1>
        {pane === 'archive' && <ArchiveSettings />}
      </section>
    </>
  )
}
