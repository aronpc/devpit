import { t } from '../i18n'
import { useCopied } from './useCopied'
import { Icon } from './Icon'

/* Copies `text`, and says so for a moment where it was clicked. */
export function CopyButton({ text, className, label }: { text: string; className: string; label?: string }): React.JSX.Element {
  const [copied, copy] = useCopied()
  const said = label ?? t('common.copy')
  return (
    <button
      className={className}
      aria-label={copied ? t('common.copied') : said}
      title={copied ? t('common.copied') : said}
      onClick={() => copy(text)}
    >
      {copied ? (
        <Icon size="s"><path d="m5 13 4 4L19 7" /></Icon>
      ) : (
        <Icon size="s"><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></Icon>
      )}
    </button>
  )
}
