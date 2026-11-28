import { ignoringWhitespace, parse } from './diff'
import { falsePositiveOf, newLines, SHAPE, type Note } from './diffNotes'
import { severityWord } from './found'
import { useViewPrefs } from './viewPrefs'
import { t } from '../i18n'

/*
 * A unified diff, one folded section per file: what a card's changes and a
 * session's window both read. A card's review findings sit on their lines.
 */

export function DiffFiles({
  diff,
  notes = [],
  onDismiss,
}: {
  diff: string
  notes?: readonly Note[]
  onDismiss?: (note: Note, dismissed: boolean) => void
}): React.JSX.Element {
  const prefs = useViewPrefs()
  const ignore = prefs.diffWhitespace === 'ignore'
  const files = parse(diff)
  const touched = new Set(files.map((file) => file.path))
  // A review can name a file the card's diff does not show; its finding still
  // has to be read and set aside somewhere.
  const unplaced = notes.filter((note) => !touched.has(note.finding.file))
  return (
    <>
      {files.map((parsed) => {
        const file = ignore ? { ...parsed, hunks: ignoringWhitespace(parsed.hunks) } : parsed
        const here = notes.filter((note) => note.finding.file === file.path)
        const shown = new Set(file.hunks.flatMap(newLines))
        const elsewhere = here.filter((note) => note.finding.line === null || !shown.has(note.finding.line))
        return (
          <details className="cdiff__f" key={file.path} open={!prefs.diffCollapsed || undefined}>
            <summary className="cdiff__s">
              <span className="cdiff__p">{file.path}</span>
              <span className="cdiff__n">
                {file.binary ? t('workspace.diffFiles.binary') : t('workspace.diffFiles.lines', { count: file.hunks.reduce((sum, hunk) => sum + hunk.rows.length, 0) })}
              </span>
            </summary>
            {elsewhere.map((note) => (
              <Marker key={`${note.runId}:${note.at}`} note={note} onDismiss={onDismiss} />
            ))}
            {/* Its own scroller: a diff is the one thing here wider than the
                pane, and letting it widen the pane would move everything else. */}
            <div className="cdiff__body">
              {file.hunks.map((hunk, at) => {
                const lines = newLines(hunk)
                return (
                  <div className="cdiff__h" key={at}>
                    <div className="cdiff__hh">{hunk.header}</div>
                    {hunk.rows.map((row, line) => (
                      <div key={line}>
                        <div className="cdiff__r" data-kind={row.kind} data-line={lines[line] ?? undefined}>
                          {row.text}
                        </div>
                        {lines[line] !== null &&
                          here
                            .filter((note) => note.finding.line === lines[line])
                            .map((note) => <Marker key={`${note.runId}:${note.at}`} note={note} onDismiss={onDismiss} />)}
                      </div>
                    ))}
                  </div>
                )
              })}
            </div>
          </details>
        )
      })}
      {unplaced.length > 0 && (
        <details className="cdiff__f" open>
          <summary className="cdiff__s">
            <span className="cdiff__p">{t('workspace.diffFiles.notInDiff')}</span>
          </summary>
          <Findings notes={unplaced} onDismiss={onDismiss} />
        </details>
      )}
    </>
  )
}

/* Findings with no diff line to sit on, each saying its file. */
export function Findings({
  notes,
  onDismiss,
}: {
  notes: readonly Note[]
  onDismiss?: (note: Note, dismissed: boolean) => void
}): React.JSX.Element {
  return (
    <>
      {notes.map((note) => (
        <Marker key={`${note.runId}:${note.at}`} note={note} onDismiss={onDismiss} named />
      ))}
    </>
  )
}

function Marker({
  note,
  onDismiss,
  named = false,
}: {
  note: Note
  onDismiss?: (note: Note, dismissed: boolean) => void
  /** Says its file, where no file section around it does. */
  named?: boolean
}): React.JSX.Element {
  const { finding } = note
  const old = note.standing === 'outdated'
  const falsePositive = falsePositiveOf(note)
  return (
    <details className="cdiff__fd" data-standing={note.standing} data-dismissed={note.dismissed || undefined}>
      <summary>
        <span className="fnd__s" data-severity={finding.severity}>
          {SHAPE[finding.severity]} {severityWord(finding.severity)}
        </span>
        {named && <span className="cdiff__fl">{finding.file}</span>}
        {finding.line !== null && <span className="cdiff__fl">{t('workspace.diffFiles.line', { line: finding.line })}</span>}
        {old && <span className="cdiff__fl">{t('workspace.diffFiles.outdated', { at: note.atRevision?.slice(0, 10) ?? t('workspace.diffFiles.unknownCommit') })}</span>}
        {note.dismissed && <span className="cdiff__fl">{falsePositive ? t('workspace.diffFiles.falsePositiveSaid') : t('workspace.diffFiles.dismissedSaid')}</span>}
      </summary>
      <p className="fnd__y">{finding.why}</p>
      {onDismiss && (
        <button className="btn" onClick={() => onDismiss(note, !note.dismissed)}>
          {falsePositive ? t(note.dismissed ? 'workspace.diffFiles.notFalsePositive' : 'workspace.diffFiles.falsePositive') : t(note.dismissed ? 'workspace.diffFiles.keep' : 'workspace.diffFiles.dismiss')}
        </button>
      )}
    </details>
  )
}
