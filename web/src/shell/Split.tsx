import { useRef, useState } from 'react'

import type { LayoutNode } from '../gen/bindings'
import { t } from '../i18n'
import { draggedTo, sides } from './splits'

/*
 * A tab's panes, drawn as the tree says.
 *
 * The tree was always there — persisted, with a direction and a ratio per
 * boundary — and the screen drew one pane at a time regardless. So splitting
 * had nowhere to show, and the shape the backend kept meant nothing.
 *
 * Every leaf stays mounted whether or not it is the focused one: a terminal
 * unmounted to draw its sibling loses its attachment and its scrollback, and
 * a split you can only half see is not a split.
 */

export function Split({
  node,
  focused,
  onFocus,
  onRatio,
  leaf,
}: {
  node: LayoutNode
  focused: string
  onFocus: (leafId: string) => void
  /** Called when a boundary settles, not while it moves. */
  onRatio: (splitId: string, ratio: number) => void
  leaf: (leafId: string) => React.ReactNode
}): React.JSX.Element {
  if (node.type === 'leaf') {
    return (
      <div
        className="tleaf"
        data-focused={String(node.id === focused)}
        onFocusCapture={() => onFocus(node.id)}
        onMouseDown={() => onFocus(node.id)}
      >
        {leaf(node.id)}
      </div>
    )
  }
  return (
    <Boundary node={node} focused={focused} onFocus={onFocus} onRatio={onRatio} leaf={leaf} />
  )
}

function Boundary({
  node,
  focused,
  onFocus,
  onRatio,
  leaf,
}: {
  node: Extract<LayoutNode, { type: 'split' }>
  focused: string
  onFocus: (leafId: string) => void
  onRatio: (splitId: string, ratio: number) => void
  leaf: (leafId: string) => React.ReactNode
}): React.JSX.Element {
  const box = useRef<HTMLDivElement>(null)
  /* The ratio while a drag is in flight. The tree is the truth between
     drags; persisting on every pointer move would be a write per frame. */
  const [dragging, setDragging] = useState<number | null>(null)
  /* A tree persisted before ratios existed reports none. Half is what a
     split with no remembered boundary looked like. */
  const ratio = dragging ?? node.ratio ?? 0.5
  const down = node.direction === 'vertical'
  const { first, second } = sides(ratio)

  const move = (event: PointerEvent): void => {
    const bounds = box.current?.getBoundingClientRect()
    if (!bounds) return
    const along = down ? event.clientY - bounds.top : event.clientX - bounds.left
    setDragging(draggedTo(along, down ? bounds.height : bounds.width, ratio))
  }

  const start = (event: React.PointerEvent): void => {
    event.preventDefault()
    const settle = (): void => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', settle)
      setDragging((landed) => {
        /* A boundary with no id cannot be addressed, so a drag on one is a
           drag that moves the screen and is forgotten. `name_the_splits` gives
           every boundary an id on the first read, so this is the shape of an
           older tree arriving, not a case that persists. */
        if (landed !== null && node.id) onRatio(node.id, landed)
        return null
      })
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', settle)
  }

  return (
    <div className="split" data-down={String(down)} ref={box}>
      <div className="split__side" style={{ flexBasis: first }}>
        <Split node={node.first} focused={focused} onFocus={onFocus} onRatio={onRatio} leaf={leaf} />
      </div>
      <div
        className="split__bar"
        role="separator"
        aria-orientation={down ? 'horizontal' : 'vertical'}
        aria-label={t('workspace.split.resize')}
        data-dragging={String(dragging !== null)}
        onPointerDown={start}
      />
      <div className="split__side" style={{ flexBasis: second }}>
        <Split
          node={node.second}
          focused={focused}
          onFocus={onFocus}
          onRatio={onRatio}
          leaf={leaf}
        />
      </div>
    </div>
  )
}
