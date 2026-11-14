import { useCallback, useEffect, useState } from 'react'

import type { AgentChoice, Profile } from '../gen/bindings'
import { AgentRow } from './AgentRow'
import { EnvironmentCopy } from './EnvironmentCopy'
import { ProviderChecks } from './ProviderChecks'
import { ProviderScan } from './ProviderScan'
import { ScopedSetting } from './ScopedSetting'
import { FieldSelect } from './FieldSelect'
import { catalogue, choosable, defaultLost, elsewhere, here, type Entry } from './catalogue'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { ProfileEditor } from './ProfileEditor'
import { ProfilesWaiting } from './ProfilesWaiting'
import { blank, declaredFrom, draftOf, profilesChanged, type Draft } from './profiles'
import { useKnownAgents } from './useKnownAgents'

/*
 * The agents this computer can start, and what each one runs.
 *
 * Three questions, in the order somebody asks them: what opens when I start a
 * terminal, what is on this machine, and what exactly does each one run. The
 * pane used to answer only the second, as a list of paths.
 *
 * A profile — environment, program, arguments — is the same shape as an
 * override, so the row that edits a built-in agent and the row that holds
 * somebody's own account are one row with the same editor behind it.
 */

const NO_AGENT = ''
/* The editor's open id while it holds a profile that does not exist yet. No
   agent or minted profile id can be this. */
const NEW = '+new'

export function ProviderRows(): React.JSX.Element {
  const [profiles, setProfiles] = useState<readonly Profile[]>([])
  const [choice, setChoice] = useState<AgentChoice>({ defaultId: NO_AGENT, disabled: [], hooks: true, trust: true })
  const [checked, setChecked] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState<string | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [busy, setBusy] = useState(false)
  const [asked, setAsked] = useState(0)
  const agents = useKnownAgents(asked)

  const refresh = useCallback(() => {
    setAsked((was) => was + 1)
    void ask(() => commands.agentProfiles()).then((answer) => {
      setProfiles(answer.data ?? [])
      setError(answer.error)
      setChecked(new Date().toLocaleTimeString())
    })
    void ask(() => commands.agentChoice()).then((answer) => {
      if (answer.data) setChoice(answer.data)
    })
  }, [])

  useEffect(refresh, [refresh])

  const entries = catalogue(agents, profiles, choice)
  const lost = defaultLost(entries, choice.defaultId)

  const act = <T,>(call: () => Promise<{ data: T | null; error: string | null }>): void => {
    setBusy(true)
    void call()
      .then((answer) => {
        setError(answer.error)
        if (!answer.error) profilesChanged()
        return answer
      })
      .finally(() => setBusy(false))
  }

  const setDefault = (id: string): void =>
    act(() =>
      ask(() => commands.agentDefaultSet(id)).then((answer) => {
        if (answer.data) setChoice(answer.data)
        return answer
      }),
    )

  const setHooks = (on: boolean): void =>
    act(() =>
      ask(() => commands.agentHooksSet(on)).then((answer) => {
        if (answer.data) setChoice(answer.data)
        return answer
      }),
    )

  const setTrust = (on: boolean): void =>
    act(() =>
      ask(() => commands.agentTrustSet(on)).then((answer) => {
        if (answer.data) setChoice(answer.data)
        return answer
      }),
    )

  const setEnabled = (id: string, on: boolean): void =>
    act(() =>
      ask(() => commands.agentEnabledSet(id, on)).then((answer) => {
        if (answer.data) setChoice(answer.data)
        return answer
      }),
    )

  const save = (): void => {
    if (!draft) return
    act(() =>
      ask(() => commands.agentProfileSave(declaredFrom(draft))).then((answer) => {
        if (answer.data) {
          setProfiles(answer.data)
          setOpen(null)
        }
        return answer
      }),
    )
  }

  const remove = (id: string): void =>
    act(() =>
      ask(() => commands.agentProfileRemove(id)).then((answer) => {
        if (answer.data) {
          setProfiles(answer.data)
          setOpen(null)
        }
        return answer
      }),
    )

  /* Opening a built-in starts a profile based on it: an override and an
     account are the same three fields, so they are the same editor. It keeps
     the built-in's id, so the switch, the default and the steps that named it
     still mean this row. */
  const opened = (entry: Entry): void => {
    if (open === entry.id) return setOpen(null)
    setOpen(entry.id)
    setDraft(
      entry.profile
        ? draftOf(entry.profile)
        : { ...blank(entry.id), id: entry.id, label: entry.label, command: '' },
    )
  }

  /* Another account or endpoint of an agent already here — `claude2`, `glm`.
     Claude Code first, since it is the one those variables mean anything to. */
  const create = (): void => {
    if (open === NEW) return setOpen(null)
    setOpen(NEW)
    setDraft(blank(agents.find((one) => one.id === 'claude')?.id ?? agents[0]?.id ?? ''))
  }

  return (
    <>
      <ProfilesWaiting />
      <ProviderScan />
      <div className="card2">
        <div className="card2__top">
          <div style={{ flex: '1', minWidth: '0' }}>
            <div className="card2__t">Default agent</div>
            <div className="card2__d">
              What a new terminal opens. A plain shell is an answer too.
            </div>
          </div>
        </div>
        {/* One menu, not a row of buttons: the row grew with every profile
            anyone declared, and a default is picked once and read often. */}
        <div className="agpick">
          <FieldSelect
            label="Default agent"
            value={choice.defaultId}
            options={[
              { id: NO_AGENT, label: 'No agent', hint: 'A plain shell' },
              ...choosable(entries).map((entry) => ({ id: entry.id, label: entry.label, hint: entry.launch })),
            ]}
            onPick={(id) => {
              if (!busy) setDefault(id)
            }}
          />
        </div>
        {/* A default pointing at something absent or switched off opens
            nothing. Said here rather than discovered at the next launch. */}
        {lost && (
          <p className="acc__note">
            The default is not on this machine any more, so a new terminal opens a shell.
          </p>
        )}
      </div>

      <PrefSwitch
        on={choice.hooks}
        disabled={busy}
        onFlip={() => setHooks(!choice.hooks)}
        title="Agent status hooks"
        said="Shows working, waiting and done on the island, in the sidebar and on a card."
        more={
          <>
            For Claude Code and the Gemini CLI run in devpit&rsquo;s terminals. The hooks travel
            with the launch &mdash; nothing is written into your own configuration, so turning
            this off is the whole of turning it off. Any other agent can report with{' '}
            <code>devpit-agent hook --agent &lt;name&gt; &lt;event&gt;</code>, run in one of devpit&rsquo;s terminals.
          </>
        }
      />

      <PrefSwitch
        on={choice.trust}
        disabled={busy}
        onFlip={() => setTrust(!choice.trust)}
        title="Trust the folders of the projects I add"
        said="Claude Code stops asking whether to trust a project's folder or its worktrees."
        more={
          <>
            Adding a project is the answer: devpit writes it into each Claude Code account it knows,
            when the project is added and when a worktree is made. A trusted folder&rsquo;s own{' '}
            <code>.claude</code> settings &mdash; hooks, permissions &mdash; apply without asking, so
            add only what you would trust by hand. Off, the question comes back for new folders.
          </>
        }
      />

      <div className="card2">
        <div className="card2__top">
          <div style={{ flex: '1', minWidth: '0' }}>
            <div className="card2__t">
              Installed <span className="git__count">{here(entries).length} detected</span>
            </div>
            <div className="card2__d">
              devpit drives agent CLIs installed on this computer. Open one to change the
              program it runs, the arguments it carries or the variables it starts with.
            </div>
          </div>
          <div>
            <button className="card2__go" onClick={refresh}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12a9 9 0 1 1-2.6-6.4" /><path d="M21 3v6h-6" /></svg>
              Refresh
            </button>
            {checked && <div className="card2__when">Checked {checked}</div>}
          </div>
        </div>

        <div className="provnew">
          <span className="provnew__d">
            Another account or endpoint of the same CLI — a second sign-in in its own config
            directory, or a gateway such as z.ai — is a profile. Each one shows up in the
            chat&rsquo;s model picker with its own history, skills and MCP servers.
          </span>
          <button className="card2__go" aria-expanded={open === NEW} onClick={create}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round"><path d="M12 5v14M5 12h14" /></svg>
            New profile
          </button>
        </div>
        {open === NEW && draft && (
          <ProfileEditor
            draft={draft}
            agents={agents}
            onChange={setDraft}
            onSave={save}
            onCancel={() => setOpen(null)}
          />
        )}

        {error && <p className="acc__note">{error}</p>}
        {!error && entries.length === 0 && (
          <p className="acc__note">
            No agent CLI found. devpit looked on your login shell’s PATH and the usual install
            folders; set a profile’s Command to the full path.
          </p>
        )}
        <EnvironmentCopy />
        <ProviderChecks />
        <ScopedSetting field="model" />

        {here(entries).map((entry) => (
          <AgentRow
            key={entry.id}
            entry={entry}
            agents={agents}
            open={open === entry.id}
            draft={open === entry.id ? draft : null}
            busy={busy}
            onOpen={() => opened(entry)}
            onDraft={setDraft}
            onSave={save}
            onRemove={() => remove(entry.id)}
            onDefault={() => setDefault(entry.id)}
            onEnabled={(on) => setEnabled(entry.id, on)}
          />
        ))}
      </div>

      {/* Said rather than hidden: a list of what could be installed is more
          use than a short list with no explanation of what is missing. */}
      {elsewhere(entries).length > 0 && (
        <div className="card2">
          <div className="card2__top">
            <div style={{ flex: '1', minWidth: '0' }}>
              <div className="card2__t">
                Not on this machine{' '}
                <span className="git__count">{elsewhere(entries).length}</span>
              </div>
              <div className="card2__d">
                devpit can drive these too. Install one with its own CLI and press Refresh.
              </div>
            </div>
          </div>
          {elsewhere(entries).map((entry) => (
            <AgentRow
              key={entry.id}
              entry={entry}
              agents={agents}
              open={open === entry.id}
              draft={open === entry.id ? draft : null}
              busy={busy}
              onOpen={() => opened(entry)}
              onDraft={setDraft}
              onSave={save}
              onRemove={() => remove(entry.id)}
              onDefault={() => setDefault(entry.id)}
              onEnabled={(on) => setEnabled(entry.id, on)}
            />
          ))}
        </div>
      )}
    </>
  )
}
