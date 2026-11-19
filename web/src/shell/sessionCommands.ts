/** Claude Code's command that makes a session reachable from claude.ai, or
 *  stops it when it already is (2.1.287). */
export const REMOTE_CONTROL = '/remote-control'

/** Claude Code commands worth sending to a session from outside its terminal. */
/** `what` is a catalog key: the sentence is said in the person's language. */
export const SESSION_COMMANDS: readonly { command: string; what: string }[] = [
  { command: REMOTE_CONTROL, what: 'sessions.cmd.remote' },
  { command: '/compact', what: 'sessions.cmd.compact' },
  { command: '/clear', what: 'sessions.cmd.clear' },
  { command: '/model', what: 'sessions.cmd.model' },
  { command: '/status', what: 'sessions.cmd.status' },
  { command: '/cost', what: 'sessions.cmd.cost' },
]

/** What is typed for a command picked or written: trimmed, one line. */
export function commandLine(text: string): string | null {
  const line = text.replace(/\s+/g, ' ').trim()
  return line ? line : null
}
