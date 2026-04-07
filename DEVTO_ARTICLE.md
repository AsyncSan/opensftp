---
title: I built a native Rust desktop app with AI - here is what actually happened
published: true
tags: rust, ai, programming, opensource
---

A few weeks ago I set out to replace my Python-based SFTP client. I wanted something native, fast, and pleasant to use. The result: Open SFTP v0.1.0 - a cross-platform SFTP client in Rust with a GUI built in Iced.

The more interesting story is HOW I built it: almost entirely using agentic AI (Claude Code).

## The Stack

- Rust
- Iced 0.13 (native, reactive, no Electron)
- russh 0.58 + russh-sftp
- ~8MB stripped binary

No Node.js. No Python runtime. No 200MB install bundle.

## The Workflow

I did not write most of the Rust by hand. Each feature:

1. Write a task: "Implement dual-pane file browser, local left, remote right, with breadcrumb navigation"
2. Agent explores codebase, implements, runs tests
3. I review the diff - accept or add follow-up task

That is it. The agent handled borrow checker battles, Iced widget patterns, async tokio channels for progress updates. I focused on product decisions and architecture direction.

## What Worked Well

Rust + agentic AI is actually a good combination. I expected it to struggle with borrow checking - it handled it reliably when each task was scoped tightly.

The agent also wrote 263 tests alongside features without me asking. Real confidence in the code.

## What Did Not Work

Complex multi-file refactors needed several rounds. Iced's reactive model is different enough that it needed more guidance. UI layout edge cases required manual iteration.

## What Shipped in v0.1.0

- Dual-pane file browser (local + remote)
- Recursive upload/download with live progress bars
- SSH key / password / agent auth + known_hosts verification
- Embedded SSH terminal
- Connection profiles with bookmarks bar
- Context menu: mkdir, rename, delete, chmod
- 22 built-in themes: Dracula, Nord, Catppuccin, Gruvbox, Tokyo Night...
- Keyboard shortcuts (F5 refresh, F7 mkdir, F8 delete, Ctrl+T cycle themes)
- Cross-platform: macOS .app, Windows .exe, Linux .tar.gz

## The Bigger Point

Agentic AI is not autocomplete on steroids. You become architect + PM. The agent becomes implementer. Your job: clear task specs and sharp review.

For solo developers building real tools, this is genuinely powerful. I shipped something I would have taken months on in a fraction of the time.

Download: https://renemurrell.de/software/opensftp
Repo: https://github.com/mylilcrowdi/Open-SFTP-Rust

What has your experience been with agentic AI on systems-level projects?
