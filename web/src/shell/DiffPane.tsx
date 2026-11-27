import { lazy, Suspense, useEffect, useState } from 'react'

import { Columns, Rows } from './GitIcons'
import { ask, commands } from './live'
import { PatchView } from './PatchView'
import type { Tab } from './strip'
import { useShell } from './useShell'
import { viewPrefs } from './viewPrefs'
import { t } from '../i18n'
import { Icon } from './Icon'

/* The merge editor is CodeMirror and its merge view: loaded with the first
   file diff opened, not with the window. */
const FileDiff = lazy(() => import('./FileDiff').then((module) => ({ default: module.FileDiff })))

/*
 * A diff, unified or side by side.
 *
 * Which diff is on screen is written on it: the panel's diff is against HEAD,
 * a card's is against the base it started from, and the two answer different
 * questions. A pane that shows one and implies the other is worse than a pane
 * that shows neither.
 *
 * A file's diff is the whole file lined up, editable on the side that is on
 * disk (`FileDiff`). A commit's diff spans many files and has no one file to
 * line up against, so it is the patch (`PatchView`).
 */

export function DiffPane({ tab }: { tab: Tab }): React.JSX.Element {
  const { project, close } = useShell()
  const path = tab.path ?? null
  /* A commit's diff and a file's diff are two questions, and the tab says
     which one it is asking. */
  const ofCommit = tab.id.startsWith('commit:')

  const bar = (controls: React.ReactNode): React.JSX.Element => (
    <div className="pane__bar dbar">
      <span className="pane__t">
        <b>{path?.split('/').pop() ?? t('workspace.diffPane.diff')}</b>
        {ofCommit ? t('workspace.diffPane.thisCommit') : t('workspace.diffPane.againstHead')}
      </span>
      <span className="drag" />
      {controls}
      <button className="sq26" onClick={() => close(tab.id)} aria-label={t('workspace.diffPane.close')} title={t('workspace.diffPane.close')}>
        <Icon size="s"><path d="M18 6 6 18M6 6l12 12" /></Icon>
      </button>
    </div>
  )

  if (!project || !path) return bar(null)
  if (ofCommit) return <CommitDiff projectId={project.id} sha={path} bar={bar} />
  return (
    <Suspense fallback={bar(null)}>
      <FileDiff key={path} projectId={project.id} path={path} bar={bar} />
    </Suspense>
  )
}

function CommitDiff({
  projectId,
  sha,
  bar,
}: {
  projectId: string
  sha: string
  bar: (controls: React.ReactNode) => React.JSX.Element
}): React.JSX.Element {
  const [raw, setRaw] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [split, setSplit] = useState(() => viewPrefs().diffLayout === 'split')
  /* Off by default: most diffs are read for what changed, and a page of dots
     is noise until the change *is* the whitespace. */
  const [spaces, setSpaces] = useState(false)

  useEffect(() => {
    void ask(() => commands.commitDiff(projectId, null, sha)).then((answer) => {
      setRaw(answer.data ?? '')
      setError(answer.error)
    })
  }, [projectId, sha])

  return (
    <>
      {bar(
        <>
          <button className="dbtn" aria-pressed={spaces} onClick={() => setSpaces((was) => !was)} title={spaces ? t('workspace.diffPane.hideSpaces') : t('workspace.diffPane.showSpaces')}>
            {t('workspace.diffPane.whitespace')}
          </button>
          <button className="dbtn" aria-pressed={split} onClick={() => setSplit((was) => !was)} title={split ? t('workspace.diffPane.unified') : t('workspace.diffPane.sideBySide')}>
            {split ? <Rows /> : <Columns />}
          </button>
        </>,
      )}
      <div className="code">
        {error ? (
          <div className="exempty__t">{error}</div>
        ) : (
          <PatchView key={sha} raw={raw} split={split} spaces={spaces} empty="This commit changed nothing." />
        )}
      </div>
    </>
  )
}
