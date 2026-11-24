import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { droppedPaths } from './pasting'

/*
 * Where a drop will go, drawn over the whole pane while files hover.
 *
 * Heard as the page's own drag events. The native drop is off — it would take
 * every drag in the window, and the file tree's own drag needs the page's —
 * so the files come as the browser hands them: pictures as files, and the
 * rest by the paths `text/uri-list` names. Only the chat in front listens.
 */

export function DropTarget({
  mine,
  onDrop,
}: {
  mine: boolean
  onDrop: (dropped: { paths: readonly string[]; files: readonly File[] }) => void
}): React.JSX.Element | null {
  const [hovering, setHovering] = useState(false)
  useEffect(() => {
    if (!mine) return setHovering(false)
    const carriesFiles = (event: DragEvent): boolean => event.dataTransfer?.types.includes('Files') ?? false
    const over = (event: DragEvent): void => {
      if (!carriesFiles(event)) return
      event.preventDefault()
      setHovering(true)
    }
    const left = (event: DragEvent): void => {
      if (event.relatedTarget === null) setHovering(false)
    }
    const dropped = (event: DragEvent): void => {
      setHovering(false)
      if (!carriesFiles(event)) return
      event.preventDefault()
      onDrop({ paths: droppedPaths(event.dataTransfer), files: Array.from(event.dataTransfer?.files ?? []) })
    }
    document.addEventListener('dragover', over)
    document.addEventListener('dragleave', left)
    document.addEventListener('drop', dropped)
    return () => {
      document.removeEventListener('dragover', over)
      document.removeEventListener('dragleave', left)
      document.removeEventListener('drop', dropped)
    }
  }, [mine, onDrop])

  if (!hovering) return null
  return (
    <div className="chat__drop" aria-hidden="true">
      <span className="chat__dropt">{t('workspace.drop')}</span>
    </div>
  )
}
