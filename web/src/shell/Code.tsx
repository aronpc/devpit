import { useDeferredValue, useLayoutEffect, useMemo, useRef } from 'react'

import { onEnter, onTab, unit, type Edit } from './indenting'
import type { Language } from './languages'
import { Painted } from './Painted'
import { abandoned, committed, composing } from './typing'
import { offsetOf } from './revealLine'

/*
 * An editor with colour, which is a textarea with a painting behind it.
 *
 * Not an editor component. A real one would bring its own text model, its own
 * undo stack and its own idea of what a keystroke is — and this app has
 * already had to be careful about exactly that: `typing.ts` exists because a
 * dead key sends `Enter` to commit an accent, and the fix only holds where the
 * browser's own field is doing the typing. A textarea keeps selection, undo,
 * autocorrect, composition and every accessibility affordance the platform
 * has, for free and correctly.
 *
 * So the textarea stays, made transparent, and a `<pre>` behind it draws the
 * same text in colour. The two must agree on every pixel of layout or the
 * caret drifts from the letters, which is why both take their metrics from one
 * class and the box scrolls as a whole rather than each half scrolling itself.
 */

/*
 * Makes an edit the way the browser would have.
 *
 * `execCommand('insertText')` first, and that is the whole reason edits are
 * expressed as a range and a replacement: it is the only way to change a
 * textarea that keeps the browser's own undo stack. Assigning the value
 * instead makes every Tab the end of history, and ⌘Z jumps past an hour of
 * typing.
 *
 * It is deprecated, with no replacement that keeps undo, so the fallback has
 * to be real. Not "if it returns false" — it can be **absent**, and calling a
 * missing function throws where the first version of this crashed the pane
 * instead of indenting. Absent, refusing, or throwing all land in the same
 * place: the text goes back through React, and undo is what is lost rather
 * than the keystroke.
 */
function apply(
  field: HTMLTextAreaElement,
  edit: Edit | null,
  event: React.KeyboardEvent,
  onChange: ((text: string) => void) | undefined,
): boolean {
  if (!edit) return false
  event.preventDefault()
  field.setSelectionRange(edit.from, edit.to)

  let took = false
  try {
    took = document.execCommand?.('insertText', false, edit.insert) ?? false
  } catch {
    took = false
  }
  if (!took) {
    // Through React and not through the field: writing the value behind its
    // back leaves its own tracker believing nothing changed, and the edit
    // vanishes on the next render.
    onChange?.(field.value.slice(0, edit.from) + edit.insert + field.value.slice(edit.to))
  }
  return true
}

/* `1\n2\n3…`, as one string. Built here rather than in the markup so the
   gutter is a single text node — one element per line is a few thousand the
   browser lays out on every keystroke, for a column nobody clicks. */
function numbered(text: string): string {
  const lines = text.split('\n').length
  let said = ''
  for (let line = 1; line <= lines; line += 1) said += `${line}\n`
  return said
}

/* A newline, which is not words. */
const NEWLINE = '\n'

export function Code({
  text,
  language,
  onChange,
  readOnly,
  line,
}: {
  text: string
  language: Language | null
  onChange?: (text: string) => void
  /** A line to put the caret on and bring into view, from 1 — and a new
   *  object each time it is asked, so asking twice moves twice. */
  line?: { readonly at: number } | null
  readOnly?: boolean
}): React.JSX.Element {
  const behind = useRef<HTMLPreElement>(null)
  const gutter = useRef<HTMLPreElement>(null)
  const field = useRef<HTMLTextAreaElement>(null)
  /* Where the caret goes once the new text has been drawn. Both paths need
     this: the fallback re-renders from the parent, and a selection set before
     that render is a selection the render throws away. */
  const wanted = useRef<readonly [number, number] | null>(null)
  /* Read from the file, not configured: a Rust file indents by four and a
     TypeScript one by two, and a setting would be a question with a wrong
     answer half the time. */
  const step = useMemo(() => unit(text), [text])
  /* The painting and the line numbers follow the text a beat behind: they are
     work over the whole file, and done on every keystroke they made typing
     in a large one lag. React draws the field first and these when it can,
     dropping the ones a newer keystroke already replaced. */
  const deferred = useDeferredValue(text)
  /* Only where it pays: in a small file the painted letters trailing the
     caret by a render is a flicker for nothing. */
  const shown = text.length > 50_000 ? deferred : text
  const lines = useMemo(() => numbered(shown), [shown])
  const painted = useMemo(() => <Painted text={shown} language={language} />, [shown, language])

  /* The painting and the gutter scroll with the field, but only when it
     scrolls; a deferred painting that grows afterwards is put back level. */
  useLayoutEffect(() => {
    const box = field.current
    if (!box) return
    if (behind.current) {
      behind.current.scrollTop = box.scrollTop
      behind.current.scrollLeft = box.scrollLeft
    }
    if (gutter.current) gutter.current.scrollTop = box.scrollTop
  }, [shown])

  useLayoutEffect(() => {
    const box = field.current
    if (!line || !box || !text) return
    const from = offsetOf(text, line.at)
    const to = text.indexOf('\n', from)
    box.focus()
    box.setSelectionRange(from, to < 0 ? text.length : to)
    const height = Number.parseFloat(getComputedStyle(box).lineHeight) || 18
    box.scrollTop = Math.max(0, (line.at - 1) * height - box.clientHeight / 3)
    // Keyed on the ask alone: a line is gone to once, not chased while typing.
  }, [line])

  useLayoutEffect(() => {
    const put = wanted.current
    if (!put || !field.current) return
    wanted.current = null
    field.current.setSelectionRange(put[0], put[1])
  })

  return (
    <div className="code__wrap">
      <pre className="code__lines" ref={gutter} aria-hidden="true">
        {lines}
      </pre>
      <pre className="code__paint" ref={behind} aria-hidden="true">
        {painted}
        {/* A trailing newline leaves the painting one line shorter than the
            field, and the last line of a file is where a caret usually is. */}
        {NEWLINE}
      </pre>
      <textarea
        ref={field}
        className="code__edit"
        spellCheck={false}
        value={text}
        readOnly={readOnly}
        onChange={(event) => onChange?.(event.target.value)}
        onKeyDown={(event) => {
          // Tab has no helper of its own, so composition is asked about here.
          // An IME can take Tab to pick a candidate, and stealing it then
          // would indent the file instead of choosing a character.
          if (readOnly || composing(event)) return
          const here = event.currentTarget
          const { selectionStart: from, selectionEnd: to } = here

          if (abandoned(event)) {
            /* The way out. Tab has to indent to be an editor at all, which
               means it no longer moves focus — so Escape gives the keyboard
               back, the way every editor embedded in a page does. Trapping
               Tab with no way out is a pane somebody cannot leave. */
            here.blur()
            return
          }
          if (event.key === 'Tab') {
            const edit = onTab(text, from, to, event.shiftKey, step)
            if (apply(here, edit, event, onChange)) wanted.current = edit!.select
            return
          }
          if (committed(event)) {
            const edit = onEnter(text, from, to, step)
            if (apply(here, edit, event, onChange)) wanted.current = edit.select
          }
        }}
        onScroll={(event) => {
          const { scrollTop, scrollLeft } = event.currentTarget
          if (behind.current) {
            behind.current.scrollTop = scrollTop
            behind.current.scrollLeft = scrollLeft
          }
          // The gutter follows the rows and not the columns: a number that
          // slid sideways with the code would leave the screen.
          if (gutter.current) gutter.current.scrollTop = scrollTop
        }}
      />
    </div>
  )
}
