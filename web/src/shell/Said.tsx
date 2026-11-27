import type { Message } from '../gen/bindings'
import { clock } from './chat'
import { CopyButton } from './CopyButton'
import { copiedOf, isLong, sentWith, type SentFile } from './sentFiles'
import { useShellPick } from './shellStore'
import { useStream } from './Streamed'
import { millis } from './turnFold'
import { foldOf, useDisclosed } from './useDisclosed'
import { Zoomable } from './Zoom'
import { t } from '../i18n'
import { Icon } from './Icon'

/* What a person said: the files as chips and pictures, then the words.
   `onResend` when it was steered into a turn that ended without taking it in.
   Hovered, it offers to copy it, edit from it or send it again. */
export function Said({
  message,
  onResend,
  onEdit,
  onAgain,
}: {
  message: Message
  onResend?: () => void
  onEdit?: () => void
  onAgain?: () => void
}): React.JSX.Element {
  const text = message.parts.map((part) => ('text' in part ? part.text : '')).join('')
  const { files, prompt } = sentWith(text)
  const long = isLong(prompt)
  const projectId = useShellPick((shell) => shell.project?.id ?? null)
  const [open, setOpen] = useDisclosed(foldOf(message.id, 'said'), false)
  const at = millis(message.createdAt)

  return (
    <article className="said" data-lost={onResend ? '' : undefined}>
      <div className="said__b">
        {files.length > 0 && (
          <div className="said__files">
            {files.map((file) => (file.image ? <Picture key={file.path} file={file} projectId={projectId} /> : <Chip key={file.path} file={file} />))}
          </div>
        )}
        {prompt && (
          <div className="said__t" data-folded={(long && !open) || undefined}>
            {prompt}
          </div>
        )}
        {long && (
          <button className="said__more" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
            {open ? t('workspace.said.less') : t('workspace.said.more')}
          </button>
        )}
      </div>
      {onResend && (
        <p className="said__lost">
          {t('workspace.said.lost')}
          <button onClick={onResend}>{t('workspace.said.resend')}</button>
        </p>
      )}
      <div className="hbar said__bar">
        {at !== null && <span className="tmeta">{clock(at)}</span>}
        <CopyButton className="tfbtn" text={copiedOf(text)} label={t('workspace.said.copy')} />
        {onEdit && (
          <button className="tfbtn" aria-label={t('workspace.said.editLabel')} title={t('workspace.said.editTitle')} onClick={onEdit}>
            <Icon size="s"><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" /></Icon>
          </button>
        )}
        {onAgain && (
          <button className="tfbtn" aria-label={t('workspace.said.againLabel')} title={t('workspace.said.againTitle')} onClick={onAgain}>
            <Icon size="s"><path d="M21 12a9 9 0 1 1-3-6.7" /><path d="M21 4v5h-5" /></Icon>
          </button>
        )}
      </div>
    </article>
  )
}

const Chip = ({ file }: { file: SentFile }): React.JSX.Element => (
  <span className="said__chip" title={file.path}>
    <span className="said__k">{file.kind || t('workspace.said.file')}</span>
    {file.name}
  </span>
)

/* Streamed by devpit's media scheme: a paste lives outside the project, where the page cannot reach. */
function Picture({ file, projectId }: { file: SentFile; projectId: string | null }): React.JSX.Element {
  const { url, error } = useStream(projectId, file.path)
  if (error) return <Chip file={file} />
  return <span className="said__pic">{url && <Zoomable src={url} name={file.name} path={file.path} className="said__img" />}</span>
}
