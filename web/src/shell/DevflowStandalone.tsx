import { useEffect, useState } from 'react'

import type { DevflowBinary, DevflowMcp, DevflowPlugin, DevflowPluginAction } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'

/*
 * The standalone devflow, beside devpit's own: its Claude Code plugin in the
 * profile the project's sessions run as, and its binary for other tools.
 * Both write outside devpit, so an install is done only on a yes to what it
 * runs. The plugin and devpit's Devflow on together give agents the same
 * tools twice; that is said here, with a way to switch one off.
 */

function where(plugin: DevflowPlugin): string {
  return t('devflow.standalone.where', { profile: plugin.profile ?? t('devflow.standalone.defaultProfile'), directory: plugin.directory })
}

export function DevflowPluginRow({ projectId, devpitOn, onDeactivate }: { projectId: string; devpitOn: boolean; onDeactivate: () => void }): React.JSX.Element | null {
  const [plugin, setPlugin] = useState<DevflowPlugin | null>(null)
  const [sure, setSure] = useState(false)
  const [busy, setBusy] = useState<DevflowPluginAction | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    void ask(() => commands.devflowPlugin(projectId)).then((answer) => {
      if (!live) return
      setPlugin(answer.data)
      setProblem(answer.error)
    })
    return () => {
      live = false
    }
  }, [projectId, devpitOn])

  if (!plugin) return problem ? <p className="pref__d voice__bad">{problem}</p> : null
  const act = (action: DevflowPluginAction): void => {
    setBusy(action)
    void ask(() => commands.devflowPluginDo(projectId, action)).then((answer) => {
      if (answer.data) setPlugin(answer.data)
      setProblem(answer.error)
      setSure(false)
      setBusy(null)
    })
  }
  const installed = plugin.version !== null
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('devflow.standalone.pluginTitle')}</span>
        <span className="pref__d">
          {installed
            ? t(plugin.enabled ? 'devflow.standalone.pluginOn' : 'devflow.standalone.pluginOff', { version: plugin.version ?? '', where: where(plugin) })
            : t('devflow.standalone.pluginNone', { where: where(plugin) })}
        </span>
        {plugin.twice && (
          <div className="pref" role="alert">
            <span className="pref__body">
              <span className="pref__t">{t('devflow.standalone.twiceTitle')}</span>
              <span className="pref__d">{t('devflow.standalone.twiceBody')}</span>
            </span>
            <button className="btn" onClick={() => act('disable')} disabled={busy !== null}>
              {t('devflow.standalone.disablePlugin')}
            </button>
            <button className="btn" onClick={onDeactivate} disabled={busy !== null}>
              {t('devflow.standalone.deactivateDevpit')}
            </button>
          </div>
        )}
        {!installed && sure && (
          <span className="pref__d">
            {tRich('devflow.standalone.pluginSure', {
              add: <code>claude plugin marketplace add jholhewres/devflow</code>, // ui-text: ok identifier
              update: <code>claude plugin marketplace update devflow</code>, // ui-text: ok identifier
              install: <code>claude plugin install devflow@devflow</code>, // ui-text: ok identifier
              env: <code>CLAUDE_CONFIG_DIR</code>, // ui-text: ok identifier
            }, { directory: plugin.directory })}
          </span>
        )}
        <div className="voice__row">
          {busy && <span className="pref__d">{t(`devflow.standalone.acting.${busy}`)}</span>}
          {!installed && !sure && (
            <button className="btn" onClick={() => setSure(true)}>
              {t('devflow.standalone.installPlugin')}
            </button>
          )}
          {!installed && sure && (
            <>
              <button className="btn" onClick={() => setSure(false)} disabled={busy !== null}>
                {t('devflow.cancel')}
              </button>
              <button className="btn btn--go" onClick={() => act('install')} disabled={busy !== null}>
                {t('devflow.install')}
              </button>
            </>
          )}
          {installed && (
            <>
              {!plugin.enabled && (
                <button className="btn" onClick={() => act('enable')} disabled={busy !== null}>
                  {t('devflow.enable')}
                </button>
              )}
              <button className="btn" onClick={() => act('update')} disabled={busy !== null}>
                {t('devflow.update')}
              </button>
              <button className="btn" onClick={() => act('remove')} disabled={busy !== null}>
                {t('devflow.remove')}
              </button>
            </>
          )}
        </div>
        {problem && <span className="pref__d voice__bad">{problem}</span>}
      </div>
    </div>
  )
}

/* Whether `version` is older than `known`, by its numbers: a newer devflow
   is never offered the download that would replace it with an older one. */
function behind(version: string, known: string): boolean {
  const parts = (one: string): number[] => one.split(/[.-]/).map((part) => Number.parseInt(part, 10) || 0)
  const [have, want] = [parts(version), parts(known)]
  for (let at = 0; at < Math.max(have.length, want.length); at += 1) {
    const diff = (have[at] ?? 0) - (want[at] ?? 0)
    if (diff !== 0) return diff < 0
  }
  return false
}

export function DevflowBinaryRow(): React.JSX.Element | null {
  const [binary, setBinary] = useState<DevflowBinary | null>(null)
  const [sure, setSure] = useState(false)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    void ask(() => commands.devflowBinary()).then((answer) => live && setBinary(answer.data))
    return () => {
      live = false
    }
  }, [])

  if (!binary) return null
  const install = (): void => {
    setBusy(true)
    void ask(() => commands.devflowBinaryInstall()).then((answer) => {
      if (answer.data) setBinary(answer.data)
      setProblem(answer.error)
      setSure(false)
      setBusy(false)
    })
  }
  const installed = binary.version !== null
  const offered = binary.version === null || behind(binary.version, binary.known)
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('devflow.standalone.binaryTitle')}</span>
        <span className="pref__d">
          {installed ? t('devflow.standalone.binaryInstalled', { version: binary.version ?? '', path: binary.path }) : t('devflow.standalone.binaryNone', { path: binary.path })}
          {t('devflow.standalone.binaryNote')}
        </span>
        {sure && (
          <span className="pref__d">
            {t('devflow.standalone.binarySure', { known: binary.known, path: binary.path })}
          </span>
        )}
        <div className="voice__row">
          {busy && <span className="pref__d">{t('devflow.standalone.downloading')}</span>}
          {sure ? (
            <>
              <button className="btn" onClick={() => setSure(false)} disabled={busy}>
                {t('devflow.cancel')}
              </button>
              <button className="btn btn--go" onClick={install} disabled={busy}>
                {installed ? t('devflow.update') : t('devflow.install')}
              </button>
            </>
          ) : (
            offered && (
              <button className="btn" onClick={() => setSure(true)}>
                {installed ? t('devflow.standalone.updateTo', { known: binary.known }) : t('devflow.standalone.installDevflow')}
              </button>
            )
          )}
        </div>
        {binary.doctor.length > 0 && (
          <pre className="errprev__body" aria-label={t('devflow.standalone.doctorLabel')}>
            {binary.doctor.join('\n')}
          </pre>
        )}
        {problem && <span className="pref__d voice__bad">{problem}</span>}
      </div>
    </div>
  )
}

/* devflow as an MCP server in each tool's own configuration, for its sessions
   outside devpit. A write to another tool's file waits for a yes to the file. */
export function DevflowMcpRow(): React.JSX.Element | null {
  const [mcp, setMcp] = useState<DevflowMcp | null>(null)
  const [sure, setSure] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    let live = true
    void ask(() => commands.devflowMcp()).then((answer) => {
      if (!live) return
      setMcp(answer.data)
      setProblem(answer.error)
    })
    return () => {
      live = false
    }
  }, [])

  if (!mcp) return problem ? <p className="pref__d voice__bad">{problem}</p> : null
  const set = (tool: string, added: boolean): void => {
    setBusy(true)
    void ask(() => commands.devflowMcpSet(tool, added)).then((answer) => {
      if (answer.data) setMcp(answer.data)
      setProblem(answer.error)
      setSure(null)
      setBusy(false)
    })
  }
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('devflow.standalone.mcpTitle')}</span>
        <span className="pref__d">{tRich('devflow.standalone.mcpBody', {
            command: <code>{mcp.command} mcp</code>, // ui-text: ok identifier
          })}</span>
        {mcp.tools.map((one) => (
          <div className="pref" key={one.tool}>
            <span className="pref__body">
              <span className="pref__t">{one.label}</span>
              <span className="pref__d">
                {one.added ? t('devflow.standalone.mcpAdded', { file: one.file }) : sure === one.tool ? t('devflow.standalone.mcpSure', { file: one.file }) : t('devflow.standalone.mcpNotAdded')}
              </span>
            </span>
            {one.added ? (
              <button className="btn" onClick={() => set(one.tool, false)} disabled={busy}>
                {t('devflow.remove')}
              </button>
            ) : sure === one.tool ? (
              <>
                <button className="btn" onClick={() => setSure(null)} disabled={busy}>
                  {t('devflow.cancel')}
                </button>
                <button className="btn btn--go" onClick={() => set(one.tool, true)} disabled={busy}>
                  {t('devflow.standalone.add')}
                </button>
              </>
            ) : (
              <button className="btn" onClick={() => setSure(one.tool)} disabled={busy}>
                {t('devflow.standalone.addAsk')}
              </button>
            )}
          </div>
        ))}
        {problem && <span className="pref__d voice__bad">{problem}</span>}
      </div>
    </div>
  )
}
