import { convertFileSrc } from '@tauri-apps/api/core'
import { useEffect, useState } from 'react'

import { bytes } from './disk'
import { ask, commands } from './live'
import { Picture } from './Picture'
import { t } from '../i18n'

/*
 * Video, audio and pictures too big to travel inline, streamed by devpit's
 * own scheme in the ranges the player asks for (`media.rs`).
 */

export type StreamedKind = 'video' | 'audio' | 'image'

/** The address the window streams a file from, or why it cannot. */
export function useStream(projectId: string | null, path: string): { url: string | null; error: string | null } {
  const [said, setSaid] = useState<{ url: string | null; error: string | null }>({ url: null, error: null })
  useEffect(() => {
    let live = true
    void ask(() => commands.mediaOpen(path.startsWith('/') ? null : projectId, path)).then((answer) => {
      if (live) setSaid({ url: answer.data ? convertFileSrc(answer.data, 'devpitmedia') : null, error: answer.error })
    })
    return () => {
      live = false
    }
  }, [projectId, path])
  return said
}

export function Streamed({ kind, projectId, path, name, size }: { kind: StreamedKind; projectId: string | null; path: string; name: string; size: number | null }): React.JSX.Element {
  const { url, error } = useStream(projectId, path)
  const [broken, setBroken] = useState(false)

  if (error) return <div className="exempty__t">{error}</div>
  if (!url) return <div className="exempty__d">{t('workspace.streamed.opening', { name })}</div>
  if (kind === 'image') return <Picture src={url} name={name} size={size} />
  if (broken)
    return (
      <div className="exempty">
        <span className="exempty__t">{t('workspace.streamed.broken', { name })}</span>
        <span className="exempty__d">{t('workspace.streamed.codec')} {bytes(size)}</span>
      </div>
    )
  return (
    <div className="media">
      {kind === 'video' ? (
        <video className="media__video" src={url} controls preload="metadata" aria-label={name} onError={() => setBroken(true)} />
      ) : (
        <audio className="media__audio" src={url} controls preload="metadata" aria-label={name} onError={() => setBroken(true)} />
      )}
      <div className="media__what">{bytes(size)}</div>
    </div>
  )
}
