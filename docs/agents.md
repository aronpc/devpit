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
