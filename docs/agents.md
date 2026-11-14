# Any agent on the island and the card

devpit hears Claude Code through its hooks. Any other agent — aider, opencode,
a CI job, a script of your own — can tell devpit what it is doing the same
way, with one command, and shows up on the island and on its card like
Claude Code does.

```sh
devpit-agent hook --agent aider SessionStart
devpit-agent hook --agent aider UserPromptSubmit
devpit-agent hook --agent aider PreToolUse --tool Edit --target src/app.rs
devpit-agent hook --agent aider PostToolUse --tool Edit
devpit-agent hook --agent aider Stop --said "Tests pass"
devpit-agent hook --agent aider SessionEnd
```

`devpit-agent` is on the `PATH` of every terminal devpit opens. Run inside
one of them, the event belongs to that terminal and its card.

## The command

`devpit-agent hook --agent NAME EVENT [options]`

- `--agent NAME` — up to 24 of `a-z`, `0-9` and `-`. `claude` is taken:
  Claude Code speaks through its own hooks.
- `EVENT` — one of `SessionStart`, `UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `PostToolUseFailure`, `Notification` (the agent waits on
  the person), `Stop` (a turn ended), `StopFailure`, `SessionEnd`.
- `--session ID` — which session the event belongs to. Without it, every
  event from the same agent process is one session.
- `--tool NAME`, `--target WHAT` — the step: `Edit src/app.rs`,
  `Run npm test`. A target that looks like a path is read as one.
- `--said TEXT` — what the turn ended with (`Stop`), or the error
  (`StopFailure`).

JSON piped in is taken as the payload to build on, in Claude Code's hook
shape (`tool_name`, `tool_input`, `session_id`, `cwd`…); the options win over
it:

```sh
echo '{"tool_name":"Bash","tool_input":{"command":"npm test"}}' \
  | devpit-agent hook --agent aider PreToolUse
```

The command always exits 0 and waits at most a second and a half: a report
that fails must not fail the agent sending it.

## What it never does

There is no `PermissionRequest`. An approval asked this way could pass for
one of Claude Code's, and approving is the person's, in the agent itself.

## Gemini CLI, with nothing to set up

In a devpit terminal Gemini CLI reports on its own. The terminal sets
`GEMINI_CLI_SYSTEM_DEFAULTS_PATH` to `<home>/gemini-hooks.json`: the
system's own defaults file, if there is one, with devpit's hooks added.
Gemini merges that file under the person's settings, and it concatenates hook
lists, so their own hooks still run and `~/.gemini/settings.json` is never
written. If the system's file exists but cannot be read as JSON, devpit leaves
the variable alone rather than hide it.

The hooks post with `?from=gemini`. The listener puts each report into Claude
Code's words: `BeforeTool` becomes `PreToolUse`, `AfterTool` becomes
`PostToolUse`, or `PostToolUseFailure` when the tool returned an error.
`BeforeAgent` becomes `UserPromptSubmit`, and `AfterAgent` becomes `Stop`
with the reply. A `Notification` is a permission prompt, so the session shows
as waiting. Gemini's tool names are mapped to devpit's too: `run_shell_command`
to Bash, `replace` to Edit, and so on. Nothing is answered from devpit, and
the reply is thrown away, as Gemini would show any text a hook prints.

Antigravity is not covered. It is an editor whose agent settings are global,
with no per-terminal file to point at, and devpit does not write a global file.

## devpit's tools, instructions and sessions in each CLI

devpit hands each CLI its MCP server for that launch only, and writes nothing
in the person's own configuration:

| CLI | How it gets devpit's tools |
|---|---|
| Claude Code | `--mcp-config=<home>/mcp.json` |
| Codex | `-c mcp_servers.devpit.*` overrides on the terminal's line and on `codex app-server` |
| Gemini CLI | `mcpServers.devpit` in `<home>/gemini-hooks.json`, beside the hooks |
| OpenCode, Grok, Antigravity | `mcpServers` in the ACP request that opens the chat's session |

A chat on Codex or an ACP agent reads its server from `<home>/mcp-<agent>.json`,
signed with that agent. Every server waits on a tool past devpit's longest
answer, so a card's checks are never cut off and run again.

An orchestrator's folder holds its brief once, in `CLAUDE.md` and what it
imports. `GEMINI.md` imports `CLAUDE.md`; `AGENTS.md`, which Codex and the ACP
agents read and which imports nothing, is written from it on every opening and
before each turn. Edit `CLAUDE.md`: `AGENTS.md` is rewritten. The person's
preferences, which a Claude hook hands the model with each message, lead the
message on a CLI without that hook.

A chat's permission mode is held the CLI's own way. Claude Code takes it as
`--permission-mode`; Codex as its approval policy and sandbox; an ACP agent
through devpit's answers to its permission requests. Plan needs the agent's
own read-only mode: OpenCode's `plan` is set before the message goes, and an
agent that offers none is refused Plan, saying so, rather than run in a mode
that may still write.

A terminal is resumed with the CLI's own flag: `claude --resume`,
`codex resume`, `gemini --resume`, `opencode --session` and
`cursor-agent --resume`. A chat whose process went idle resumes its session
through the driver: `--resume`, Codex's `thread/resume`, or ACP's
`session/resume` or `session/load`.

## Orchestrators on any CLI

An orchestrator is a chat, so it runs on any CLI devpit holds a chat with.
A CLI that is terminal only here, such as Gemini CLI, cannot orchestrate, though
an orchestrator can start a session on it.

| CLI | Orchestrates | What it lacks as an orchestrator |
|---|---|---|
| Claude Code | yes | nothing: `SendMessage`, `ListAgents` and `notify_when_idle` reach its account's sessions too |
| Codex | yes | `SendMessage`; a cost only for a model devpit has a price for |
| OpenCode | yes | `SendMessage`; cost and context read n/a |
| Cursor | yes | `SendMessage`; cost and context read n/a; Plan, where it offers no read-only mode |
| Grok, Antigravity | yes | `SendMessage`; cost and context read n/a; Plan, where the agent offers no read-only mode; a session on their own CLI, which devpit does not start with a brief: pass `profile` |
| Gemini CLI and the rest | no | a chat: they are started as sessions only |

Every orchestrator reaches its sessions through devpit: `devpit_session_message`
types at a session's idle prompt, and `devpit_session_notify_idle` starts one
turn in the orchestrator's chat when the session goes idle or ends. A message
that waits two minutes for a prompt devpit cannot read tells its sender so.
`devpit_start_session` takes `profile` to start a session on another CLI:
Claude Code, Codex, Gemini CLI, OpenCode and Cursor are started with their
brief; a CLI whose start nobody checked is refused.

A Claude Code orchestrator lists its account's Claude Code sessions from
Claude Code's own registry. Every other session it starts, and every session
an orchestrator on another CLI starts, is listed by devpit from the pane it
typed it into: by the name devpit gave it, busy or idle as the pane shows,
until its agent leaves the pane. That list lives in devpit's memory, so after
devpit restarts such a session is reached by the pane id `devpit_card` lists.

Stops and drafts sent on the person's word read that word where the CLI keeps
it: Claude Code's own log, or, on any other CLI, the message devpit sent the
chat. Follow chat permission, Always ask and Never ask decide alike on every
CLI, since the chat's mode is kept in devpit's head of the conversation.

**Runs as…** in an orchestrator's menu switches it to another profile, on the
same CLI or another. Its folder, notes, links and settings stay. Conversations
so far stay with the profile they began on, and its next conversation starts
on the new one, its first turn led by a handoff from `context/now.md` and the
latest of `context/sessions.md`. Switching back works the same way.
