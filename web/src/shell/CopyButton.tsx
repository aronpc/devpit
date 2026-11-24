import { t } from '../i18n'
import { useCopied } from './useCopied'

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
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="m5 13 4 4L19 7" /></svg>
      ) : (
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>
      )}
    </button>
  )
}
