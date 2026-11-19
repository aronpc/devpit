import { useEffect, useState } from 'react'

import type { Settings as Stored } from '../gen/bindings'
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
            <h1 className="prefs__h">Account</h1>

            <div className="acc__grid">
              <span className="acc__big">{account.initials}</span>
              <div className="acc__fields">
                <div className="acc__t">{account.name}</div>
                <div className="acc__d">{account.email ?? 'no address yet'}</div>
              </div>
            </div>

            <div className="acc__sub">Sync</div>
            <SyncSettings />
            <p className="acc__note">
              Projects, boards, conversations, terminal history and files stay on this computer.
            </p>
            <PlusLine />

            <div className="acc__sub">Leaving</div>
            <div className="acc__row">
              <span className="acc__body">
                <span className="acc__t">Sign out</span>
                <span className="acc__d">Clears the signed-in state on this computer</span>
              </span>
              <button className="acc__act acc__act--danger" onClick={signOut}>Sign out</button>
            </div>
          </section>

          <section className="prefs__in" hidden={pane !== 'projects'}>
            <div className="prefs__hrow">
              <h1 className="prefs__h">Projects</h1>
              <button className="btn btn--go" onClick={onAddProject}><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14M5 12h14" /></svg>Add project</button>
            </div>
        <ProjectRows onRemove={onRemove} />
            <p className="acc__note">Removing a project takes it out of devpit. The folder, the git
              repository and your code are never touched &mdash; devpit only forgets where it was.</p>
          </section>

          <GeneralPanes pane={pane} on={on} set={set} />

          <section className="prefs__in" hidden={pane !== 'providers'}>
            <h1 className="prefs__h">Providers</h1>
            <ProviderRows />
          </section>

          <section className="prefs__in" hidden={pane !== 'storage'}>
            <h1 className="prefs__h">Storage</h1>
            <div className="pref">
              <span className="pref__body"><span className="pref__t">Workspace directory</span><span className="pref__d">Each project gets one, holding its board, skills, capabilities, transcripts and worktrees. None of it is in the repository.<br /><code>~/.devpit/projects/&lt;name&gt;-&lt;suffix&gt;</code></span></span>
            </div>
            <p className="acc__note">Transcripts are always written: a session that a restart
              loses is a session that was never yours to come back to. Nothing is uploaded, and
              removing a project leaves its folder where it is.</p>
          </section>

          <section className="prefs__in" hidden={pane !== 'worktrees'}><Worktrees /></section>
          <section className="prefs__in" hidden={pane !== 'hosts'}><HostsSettings /></section>
          <section className="prefs__in" hidden={pane !== 'appearance'}>
            <h1 className="prefs__h">Appearance</h1>
            <div className="thm" role="radiogroup" aria-label="Theme">
              <button className="thm__c" role="radio" aria-checked={theme === 'system'} onClick={() => setTheme('system')}>
                <span className="thm__p thm__p--sys">
                  <span className="thm__lay thm__p--light"><span className="thm__side"></span><span className="thm__main"><span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span></span></span>
                  <span className="thm__lay thm__p--dark"><span className="thm__side"></span><span className="thm__main"><span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span></span></span>
                </span>
                <span className="thm__l">System</span>
              </button>
              <button className="thm__c" role="radio" aria-checked={theme === 'light'} onClick={() => setTheme('light')}>
                <span className="thm__p thm__p--light"><span className="thm__side"></span><span className="thm__main">
                  <span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span>
                </span></span>
                <span className="thm__l">Light</span>
              </button>
              <button className="thm__c" role="radio" aria-checked={theme === 'dark'} onClick={() => setTheme('dark')}>
                <span className="thm__p thm__p--dark"><span className="thm__side"></span><span className="thm__main">
                  <span className="thm__bar"></span><span className="thm__bar thm__bar--s"></span><span className="thm__bar"></span>
                </span></span>
                <span className="thm__l">Dark</span>
              </button>
            </div>
            <p className="acc__note">The terminal keeps its dark ground in every theme. A shell paints
              its own ANSI colours for a dark background; on a white one they stop being readable.</p>
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
          <section className="prefs__in" hidden={pane !== 'assistant'}><h1 className="prefs__h">Assistant</h1><AssistantSettings /></section>
          <section className="prefs__in" hidden={pane !== 'orchestrator'}><h1 className="prefs__h">Orchestrator</h1><GuardrailSettings /></section>
          <section className="prefs__in" hidden={pane !== 'decisions'}><h1 className="prefs__h">Decisions</h1><DecisionsSettings /></section>
          <section className="prefs__in" hidden={pane !== 'devflow'}>
            <h1 className="prefs__h">Devflow</h1>
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
