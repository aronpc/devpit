import { useEffect, useState } from 'react'

import { t } from '../i18n'
import { ask, commands } from './live'
import { Icon } from './Icon'

/*
 * Assistant mode for an orchestrator's chat: off by default, kept per
 * orchestrator. The running chat gains or loses the assistant's tools at
 * once; its brief is read when the chat next starts.
 */
export function AssistantToggle({ projectId }: { projectId: string }): React.JSX.Element {
  const [on, setOn] = useState(false)
  const [busy, setBusy] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.assistantMode(projectId)).then((found) => setOn(found.data?.on ?? false))
  }, [projectId])

  const flip = (): void => {
    setBusy(true)
    void ask(() => commands.assistantModeSet(projectId, !on))
      .then((said) => {
        if (said.data) setOn(said.data.on)
        setProblem(said.error)
      })
      .finally(() => setBusy(false))
  }

  const title = problem
    ? t('workspace.assistantToggle.failed', { problem })
    : on
      ? t('workspace.assistantToggle.on')
      : t('workspace.assistantToggle.off')
  return (
    <button className="sq26 asst" data-on={on ? 'true' : undefined} disabled={busy} title={title} aria-label={t('workspace.assistantToggle.label')} aria-pressed={on} onClick={flip}>
      <Icon size="s"><circle cx="12" cy="13" r="8" /><path d="M12 9v4l2 2M5 3 2 6M22 6l-3-3" /></Icon>
    </button>
  )
}
