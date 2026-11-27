import { useEffect, useState } from 'react'

import type { Settings as Stored } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { inOrder } from './inOrder'
import { PlusLine } from './PlusLine'
import { PrefsSide } from './PrefsSide'
import { GeneralPanes } from './GeneralSettings'
import { KeyboardSettings } from './KeyboardSettings'
import { ProjectRows } from './ProjectRows'
import { useShell, type PrefsPane } from './useShell'
import { ProviderRows } from './ProviderRows'
import { flagOn, type Flag } from './settingsFlags'
import { AccountsSettings } from './AccountsSettings'
import { AssistantSettings } from './AssistantSettings'
import { GuardrailSettings } from './GuardrailSettings'
import { HostsSettings } from './HostsSettings'
import { ChannelsPane } from './ChannelsPane'
import { DecisionsSettings } from './DecisionsSettings'
import { DevflowSettings } from './DevflowSettings'
import { SkillsPane } from './SkillsPane'
import { SyncSettings } from './SyncSettings'
import { TerminalContrast } from './TerminalContrast'
import { Usage } from './Usage'
import { LookSettings } from './ViewSettings'
import { Worktrees } from './Worktrees'
import { onEvent } from './window'
import { Icon } from './Icon'

/* Settings takes the window. Back and Escape leave; the gear and every
   account-menu row land here on the pane they name. */

export function Settings({
  pane,
  row,
  onAddProject,
  onRemove,
}: {
  pane: PrefsPane
  row: string | null
  onAddProject: () => void
  onRemove: (project: string) => void
}): React.JSX.Element {
  const { closePrefs, openPrefs, theme, setTheme, signOut, account } = useShell()
  /* Hears the locale, so the panes re-say themselves when it changes. */
  useLocale()
  const [flags, setFlags] = useState<Stored | null>(null)

  /* Read again when another computer's settings were written here. */
  useEffect(() => {
    const read = (): void => void ask(() => commands.settingsRead()).then((answer) => setFlags(answer.data))
    read()
    return onEvent('sync:written', read)
  }, [])

  /* Each toggle writes only its own field: the command takes null for
     "leave this one alone", so one switch cannot overwrite another. */
  const set = (field: Flag, next: boolean): void => {
    /* Its own command: switching it also wakes or quiets the app's clock. */
    if (field === 'reminders') {
      void inOrder('settings', () => ask(() => commands.remindersSet(next)))
        .then(() => ask(() => commands.settingsRead()))
        .then((answer) => setFlags(answer.data ?? flags))
      return
    }
    void inOrder('settings', () =>
      ask(() =>
        commands.settingsWrite(
          null,
          null,
          field === 'automaticUpdates' ? next : null,
          field === 'confirmStop' ? next : null,
          null,
          field === 'focusMode' ? next : null,
          field === 'errorReports' ? next : null,
          field === 'island' ? next : null,
        ),
      ),
    ).then((answer) => {
      setFlags(answer.data ?? flags)
      if (field === 'island' && answer.data) void ask(() => commands.islandApply())
    })
  }

  const on = (field: Flag): boolean => flagOn(flags, field)

  return (
    <div className="prefs" data-open="true">
      <PrefsSide pane={pane} row={row} onBack={closePrefs} onSelect={openPrefs} />
      <div className="prefs__main">
          <section className="prefs__in" hidden={pane !== 'account'}>
            <h1 className="prefs__h">{t('prefs.nav.account')}</h1>

            <div className="acc__grid">
              <span className="acc__big">{account.initials}</span>
              <div className="acc__fields">
                <div className="acc__t">{account.name}</div>
                <div className="acc__d">{account.email ?? t('settings.account.noAddress')}</div>
              </div>
            </div>

            <div className="acc__sub">{t('settings.account.sync')}</div>
            <SyncSettings />
            <p className="acc__note">{t('settings.account.stays')}</p>
            <PlusLine />

            <div className="acc__sub">{t('settings.account.leaving')}</div>
            <div className="acc__row">
              <span className="acc__body">
                <span className="acc__t">{t('settings.account.signOut')}</span>
                <span className="acc__d">{t('settings.account.signOutSaid')}</span>
              </span>
              <button className="acc__act acc__act--danger" onClick={signOut}>{t('settings.account.signOut')}</button>
            </div>
          </section>

          <section className="prefs__in" hidden={pane !== 'projects'}>
            <div className="prefs__hrow">
              <h1 className="prefs__h">{t('prefs.nav.projects')}</h1>
              <button className="btn btn--go" onClick={onAddProject}><Icon size="s"><path d="M12 5v14M5 12h14" /></Icon>{t('settings.projects.add')}</button>
            </div>
        <ProjectRows onRemove={onRemove} />
            <p className="acc__note">{t('settings.projects.note')}</p>
          </section>

          <GeneralPanes pane={pane} on={on} set={set} />

          <section className="prefs__in" hidden={pane !== 'providers'}>
            <h1 className="prefs__h">{t('prefs.nav.providers')}</h1>
            <ProviderRows />
          </section>

          <section className="prefs__in" hidden={pane !== 'storage'}>
            <h1 className="prefs__h">{t('prefs.nav.storage')}</h1>
            <div className="pref">
              <span className="pref__body"><span className="pref__t">{t('settings.storage.directory')}</span><span className="pref__d">{t('settings.storage.directorySaid')}<br /><code>{t('settings.storage.path')}</code></span></span>
            </div>
            <p className="acc__note">{t('settings.storage.note')}</p>
          </section>

          <section className="prefs__in" hidden={pane !== 'worktrees'}><Worktrees /></section>
          <section className="prefs__in" hidden={pane !== 'hosts'}><HostsSettings /></section>
          <section className="prefs__in" hidden={pane !== 'appearance'}>
            <h1 className="prefs__h">{t('prefs.nav.appearance')}</h1>
            <div className="thm" role="radiogroup" aria-label={t('theme.title')}>
              <button className="thm__c" role="radio" aria-checked={theme === 'system'} onClick={() => setTheme('system')}>
                <span className="thm__p thm__p--sys">
                  <span className="thm__lay thm__p--light"><span className="thm__side"></span><span className="thm__main"><span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span></span></span>
                  <span className="thm__lay thm__p--dark"><span className="thm__side"></span><span className="thm__main"><span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span></span></span>
                </span>
                <span className="thm__l">{t('theme.system')}</span>
              </button>
              <button className="thm__c" role="radio" aria-checked={theme === 'light'} onClick={() => setTheme('light')}>
                <span className="thm__p thm__p--light"><span className="thm__side"></span><span className="thm__main">
                  <span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span>
                </span></span>
                <span className="thm__l">{t('theme.light')}</span>
              </button>
              <button className="thm__c" role="radio" aria-checked={theme === 'dark'} onClick={() => setTheme('dark')}>
                <span className="thm__p thm__p--dark"><span className="thm__side"></span><span className="thm__main">
                  <span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span>
                </span></span>
                <span className="thm__l">{t('theme.dark')}</span>
              </button>
            </div>
            <p className="acc__note">{t('settings.appearance.terminalNote')}</p>
            <TerminalContrast
              value={flags?.terminalContrast ?? null}
              onPick={(value) =>
                void inOrder('settings', () => ask(() => commands.settingsWrite(null, null, null, null, value, null, null, null))).then((answer) =>
                  setFlags(answer.data ?? flags),
                )
              }
            />
            <LookSettings />
          </section>

          <section className="prefs__in" hidden={pane !== 'keyboard'}>
            <KeyboardSettings onGeneral={() => openPrefs('general', 'Shortcut')} />
          </section>
          <section className="prefs__in" hidden={pane !== 'channels'}><ChannelsPane /></section>
          <section className="prefs__in" hidden={pane !== 'assistant'}><h1 className="prefs__h">{t('prefs.nav.assistant')}</h1><AssistantSettings /></section>
          <section className="prefs__in" hidden={pane !== 'orchestrator'}><h1 className="prefs__h">{t('prefs.nav.orchestrator')}</h1><GuardrailSettings /></section>
          <section className="prefs__in" hidden={pane !== 'decisions'}><h1 className="prefs__h">{t('prefs.nav.decisions')}</h1><DecisionsSettings /></section>
          <section className="prefs__in" hidden={pane !== 'devflow'}>
            <h1 className="prefs__h">{t('prefs.nav.devflow')}</h1>
            <DevflowSettings />
          </section>
          <section className="prefs__in prefs__in--wide" hidden={pane !== 'skills'}>
            <SkillsPane />
          </section>
          <section className="prefs__in" hidden={pane !== 'usage'}><Usage /></section>
          <section className="prefs__in" hidden={pane !== 'accounts'}><AccountsSettings /></section>
      </div>
    </div>
  )
}
