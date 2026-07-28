# Hacker News — Show HN

**Type:** Show HN  
**Status:** Ready to post  
**Best time:** Tuesday or Wednesday, 8-10am PST (15-17 CET) — day before PH launch

---

## Title

Show HN: Open SFTP - native Rust SFTP client, no Electron, ~8MB, 22 themes

---

## URL

https://github.com/AsyncSan/opensftp

---

## Comment (optional — paste as first comment after submitting)

Open SFTP is a cross-platform desktop SFTP client built in Rust with Iced.

Why I built it: I was using a Python/Electron-based SFTP client that was slow and used 500MB RAM just browsing files. Wanted something that starts instantly and feels native.

**Stack:**
- Rust + Iced 0.13 (reactive, native, no web tech)
- russh 0.58 + russh-sftp for SSH/SFTP
- Stripped binary: ~8MB on Linux aarch64

**Features:**
- Dual-pane browser (local left, remote right) with breadcrumbs
- Recursive upload/download with live per-file + overall progress bars
- SSH key, password, and agent auth with known_hosts verification
- Embedded SSH terminal
- Connection profiles + bookmarks bar
- 22 built-in themes (Dracula, Nord, Gruvbox, Catppuccin, Tokyo Night...)
- Keyboard shortcuts throughout

**Build workflow:** Almost entirely developed with agentic AI (Claude Code). Scoped tasks as tickets, agent implements, I review diffs. Full write-up on Dev.to: [DEV.TO_URL]

MIT licensed, free, no telemetry.

Downloads: https://renemurrell.de/software/opensftp
