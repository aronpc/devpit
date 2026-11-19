import { useState } from 'react'

import type { Change } from '../gen/bindings'
import { ChangeRows, type View } from './ChangeRows'
import { ChangeSection } from './ChangeSection'
import { grouped, rememberView, savedView, stageable } from './changes'
import { counted, primary } from './primary'
import { DiscardConfirm } from './DiscardConfirm'
import { List, Minus, Plus, Search, Trash, Tree, Undo } from './GitIcons'
import { ask, commands } from './live'
import { RemoteRow } from './RemoteRow'
import { keysLabel, matches } from './shortcuts'
import { Skeleton } from './Skeleton'
import { abandoned, composing } from './typing'
import { useShell } from './useShell'
import { changed, type UseTree } from './useTree'
import { t } from '../i18n'

/*
 * What has changed, and what a commit would take.
 *
 * Nothing here happens on its own: staging is a click and committing is a
 * click. The lists come from git each time rather than being predicted from
 * the click, so the panel cannot drift from the index.
 */

type Group = 'staged' | 'changed' | 'untracked'

export function Changes({ tree }: { tree: UseTree }): React.JSX.Element {
  const { project, show, active, reloadProjects } = useShell()
  /* The row of the file on screen is marked, so the list and the pane agree
     about where you are. */
  const onScreen = active?.kind === 'diff' ? (active.path ?? null) : null
  /* A row in Changes opens the diff, not the file: the question the panel is
     answering is what changed, and the file alone does not answer it. */
  const openDiff = (path: string): void =>
    show('diff', { id: `diff:${path}`, path, title: `${path.split('/').pop()} diff` })

  const [message, setMessage] = useState('')
  const [busy, setBusy] = useState(false)
  const [said, setSaid] = useState<string | null>(null)
  const [discarding, setDiscarding] = useState<readonly Change[] | null>(null)
  const [view, setView] = useState<View>(savedView)
  const [finding, setFinding] = useState<string | null>(null)
  const [shut, setShut] = useState<ReadonlySet<Group>>(() => new Set())

  const wanted = finding?.trim().toLowerCase() ?? ''
  const shown = wanted ? tree.changes.filter((change) => change.path.toLowerCase().includes(wanted)) : tree.changes
  const groups = grouped(shown)
  const act1 = primary(tree.changes, message)
  const here = project?.worktrees.find((worktree) => worktree.current) ?? project?.worktrees[0]

  const act = (call: () => Promise<unknown>): void => {
    if (!project) return
    setBusy(true)
    void call()
      .then(() => tree.reload())
      .finally(() => setBusy(false))
  }
  const stage = (paths: string[]): void => act(() => ask(() => commands.changesStage(project!.id, null, paths)))
  const unstage = (paths: string[]): void => act(() => ask(() => commands.changesUnstage(project!.id, null, paths)))
  const discard = (paths: string[]): void => act(() => ask(() => commands.changesDiscard(project!.id, null, paths)))

  const commit = (): void => {
    if (!project) return
    setBusy(true)
    void ask(() => commands.changesCommit(project.id, null, message))
      .then((answer) => {
        if (answer.data) {
          setSaid(`${answer.data.sha} · ${answer.data.subject}`)
          setMessage('')
        } else {
          setSaid(answer.error)
        }
        // History, the branch's ahead count and the file index moved too.
        changed()
        reloadProjects()
      })
      .finally(() => setBusy(false))
  }
  const go = (): void => (act1.doing === 'stage' ? stage(stageable(tree.changes)) : commit())
  /* A first subject in the repository's own voice, for what is staged, else for every change. */
  const draft = (): void => {
    const { staged } = grouped(tree.changes)
    const paths = (staged.length > 0 ? staged : tree.changes).map((change) => change.path)
    if (project) void ask(() => commands.changesDraft(project.id, null, paths)).then(({ data }) => data && setMessage((was) => was || data.message))
  }

  const turn = (next: View): void => {
    setView(next)
    rememberView(next)
  }

  const section = (group: Group, title: string, rows: readonly Change[]): React.JSX.Element => {
    const staged = group === 'staged'
    const paths = rows.map((change) => change.path)
    const name = title.toLowerCase()
    return (
      <ChangeSection
        title={title}
        count={rows.length}
        open={!shut.has(group)}
        onToggle={() =>
          setShut((was) => {
            const next = new Set(was)
            if (next.has(group)) next.delete(group)
            else next.add(group)
            return next
          })
        }
        actions={
          <>
            {!staged && (
              <button className="gitrow__act" disabled={busy} onClick={() => setDiscarding(rows)} title={group === 'untracked' ? t('workspace.changes.deleteAll') : t('workspace.changes.discardAll')} aria-label={t('workspace.changes.discardAria', { group: name })}>
                {group === 'untracked' ? <Trash /> : <Undo />}
              </button>
            )}
            <button className="gitrow__act" disabled={busy} onClick={() => (staged ? unstage(paths) : stage(paths))} title={staged ? t('workspace.changes.unstageAll') : t('workspace.changes.stageAll')} aria-label={t(staged ? 'workspace.changes.unstageAria' : 'workspace.changes.stageAria', { group: name })}>
              {staged ? <Minus /> : <Plus />}
            </button>
          </>
        }
      >
        <ChangeRows changes={rows} view={view} staged={staged} busy={busy} onOpen={openDiff} onScreen={onScreen} onStage={staged ? unstage : stage} onDiscard={(change) => setDiscarding([change])} />
      </ChangeSection>
    )
  }

  return (
    <>
      <div className="git__head">
        <div className="git__row">
          <span className="git__branch">{here?.branch ?? t('workspace.changes.branchFallback')}</span>
          <span className="git__up">
            <span className="add">+{counted(tree.totals.added)}</span>
            <span className="del">&minus;{counted(tree.totals.removed)}</span>
          </span>
          <button className="dbtn" aria-pressed={finding !== null} onClick={() => setFinding((was) => (was === null ? '' : null))} title={t('workspace.changes.filter')} aria-label={t('workspace.changes.filter')}>
            <Search />
          </button>
          <button className="dbtn" onClick={() => turn(view === 'tree' ? 'list' : 'tree')} title={t(view === 'tree' ? 'workspace.changes.viewList' : 'workspace.changes.viewTree')} aria-label={t(view === 'tree' ? 'workspace.changes.viewList' : 'workspace.changes.viewTree')}>
            {view === 'tree' ? <List /> : <Tree />}
          </button>
        </div>
        {project && <RemoteRow projectId={project.id} here={here} onDone={reloadProjects} />}
        {finding !== null && (
          <input className="git__find" autoFocus placeholder={t('workspace.changes.filter')} value={finding} onChange={(event) => setFinding(event.target.value)} onKeyDown={(event) => abandoned(event) && setFinding(null)} aria-label={t('workspace.changes.filterByPath')} />
        )}

        {/* At the top: the message used to sit under a list forty rows long,
            and was off the screen by the time you had read what changed. */}
        <textarea
          className="git__msg"
          placeholder={t('workspace.changes.messagePlaceholder', { keys: keysLabel('commit') })}
          value={message}
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (matches('commit', event) && !composing(event) && !busy && !act1.disabled) {
              event.preventDefault()
              go()
            }
          }}
          aria-label={t('workspace.changes.messageLabel')}
        />
        {!message && tree.changes.length > 0 && <button className="btn git__go" onClick={draft}>{t('workspace.changes.draft')}</button>}
        {/* One button, not three. What it says is decided in `primary.ts`. */}
        <button className="btn btn--go git__go" disabled={busy || act1.disabled} title={act1.why ?? undefined} onClick={go}>
          {act1.label}
        </button>
        {act1.why && <span className="git__why">{act1.why}</span>}
        {said && <span className="git__why">{said}</span>}
      </div>

      <div className="git__body">
        {tree.loading && tree.changes.length === 0 && <Skeleton />}
        {!tree.loading && tree.changes.length === 0 && (
          <div className="exempty">
            <span className="exempty__t">{t('workspace.changes.nothingChanged')}</span>
            {said && <span className="exempty__d">{said}</span>}
          </div>
        )}
        {wanted && shown.length === 0 && <div className="pempty"><span className="pempty__d">{t('workspace.changes.noMatch')}</span></div>}
        {section('staged', t('workspace.changes.staged'), groups.staged)}
        {section('changed', t('workspace.changes.changed'), groups.changed)}
        {section('untracked', t('workspace.changes.untracked'), groups.untracked)}
      </div>

      {discarding && (
        <DiscardConfirm
          changes={discarding}
          onClose={() => setDiscarding(null)}
          onConfirm={() => {
            discard(discarding.map((change) => change.path))
            setDiscarding(null)
          }}
        />
      )}
    </>
  )
}
