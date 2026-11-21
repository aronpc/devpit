<p align="center">
  <img src="web/src/assets/brand/mark.png" alt="" width="128">
</p>

<h1 align="center">devpit</h1>

<p align="center">
  A native app for controlling your coding agents.
</p>

<p align="center">
  <a href="https://github.com/jholhewres/devpit/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/jholhewres/devpit/ci.yml?branch=main&style=flat-square&label=ci" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/licence-Apache%202.0-blue?style=flat-square" alt="Licence: Apache 2.0"></a>
  <img src="https://img.shields.io/badge/rust-1.93+-dea584?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.93+">
  <img src="https://img.shields.io/badge/status-early-e2795b?style=flat-square" alt="Status: early">
</p>

<p align="center">
  <b>English</b> &middot; <a href="README.pt-BR.md">Português</a>
</p>

<p align="center">
  <img src="docs/assets/hero.webp" alt="devpit's orchestrator chat beside a project board, with agent sessions running in terminals" width="100%">
</p>

---

## What it is

A desktop app that keeps your coding agents in view: one terminal you are
watching, a board that says where each piece of work stands, and the cost of
every agent call.

## What it does

- **One target terminal per project**; other sessions keep running in the background.
- **Sessions that outlive the window**: they are tmux sessions.
- **A git worktree per card**, made when a step needs one.
- **A board per project** whose columns run steps: an agent, a session or your own command.
- **A spending cap on every agent call**, and the real cost on the card.
- **An orchestrator** chat that sees every project and can hand work to sessions.
- **A browser pane** for the page your project serves.
- **File tree and diffs** beside the terminal.
- **Local or online boards**: keep work on this machine or carry a board through
  devpit.app; an account is optional.

## Where data lives

An online board keeps its board, columns, cards (title, description and plan),
comments, acceptance criteria, links, history with its author and date, and
attachments on devpit.app, where the server can read them. The app reaches it
through the board API and MCP. Attachments have limits per file, per account
and across the service.

Personal sync is separate: preferences and project settings remain sealed
with a key held only by your machines, so the server never opens those blobs. A local SQLite board, projects, conversations and terminal history
never leave this machine.

## Install

```sh
curl -fsSL https://devpit.app/install.sh | sh
```

Linux and macOS; it checks the download against the release's `SHA256SUMS`.
Windows, manual downloads, updates and uninstalling:
[docs/install.md](docs/install.md). Needs `tmux` and
[Claude Code](https://claude.com/claude-code).

## Links

- [Documentation](https://devpit.app/docs)
- [What it does, in detail](docs/features.md)
- [How the board works](docs/board.md)
- [Building and running](docs/building.md)
- [The model](docs/the-model.md) · [Architecture](docs/architecture.md)
- [Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md)
- [Community on Discord](https://discord.gg/yW8VUyY63b)
- [Licence: Apache 2.0](LICENSE)
