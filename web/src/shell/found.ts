import type { Found, Severity, Standing } from '../gen/bindings'
import { t } from '../i18n'

/*
 * What a review found, in words.
 *
 * Pure, and apart from the component, because every sentence here is a claim
 * about somebody's code. The one that matters: a review made against another
 * revision is **old**, not wrong, and its line numbers mean the commit it was
 * made against. Nothing here follows a line through a diff — a review pointing
 * confidently at the wrong line is worse than one that admits its age.
 */

/** How a severity reads, with a word so it is never colour alone. */
export function severityWord(severity: Severity): string {
  switch (severity) {
    case 'blocking':
      return t('workspace.found.blocking')
    case 'worth':
      return t('workspace.found.worth')
    case 'noted':
      return t('workspace.found.noted')
  }
}

/** Sorted the way somebody would act on them: worst first, then by file. */
const ORDER: Readonly<Record<Severity, number>> = { blocking: 0, worth: 1, noted: 2 }

export const worstFirst = (found: Found): Found['findings'] =>
  [...found.findings].sort(
    (a, b) =>
      ORDER[a.severity] - ORDER[b.severity] ||
      a.file.localeCompare(b.file) ||
      (a.line ?? 0) - (b.line ?? 0),
  )

/** Where a finding points, as an editor would write it. */
export const placeOf = (file: string, line: number | null): string =>
  line === null ? file : `${file}:${line}`

/**
 * What a review's standing means, said rather than coloured.
 *
 * `null` when there is nothing to say — a review made against the code that is
 * checked out needs no caveat, and adding one would teach people to ignore the
 * caveats that matter.
 */
export function standingWords(standing: Standing, found: Found): string | null {
  switch (standing) {
    case 'current':
      return null
    case 'outdated':
      return t('workspace.found.outdated', { at: short(found.atRevision), now: short(found.now) })
    case 'unanchored':
      return found.findings.length === 0 ? null : t('workspace.found.unanchored')
  }
}

/** A commit as it is read and quoted. */
const short = (revision: string | null): string =>
  revision === null ? t('workspace.diffFiles.unknownCommit') : revision.slice(0, 10)

/** Whether there is a review here at all, as against one that found nothing. */
export const leftNoReview = (found: Found): boolean =>
  found.findings.length === 0 && found.rubric === null && found.atRevision === null
