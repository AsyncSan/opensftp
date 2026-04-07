# Reddit Post — r/rust

**Subreddit:** r/rust  
**Status:** Ready to post  
**Best time:** Tuesday-Thursday, 9-11am CET  
**Preview image:** Screenshot of the app

---

## Title

I built a native Rust SFTP client almost entirely with agentic AI (Claude Code) - Open SFTP v0.1.0

---

## Body

Hey r/rust,

Shipping a side project: **Open SFTP** - a native desktop SFTP client in Rust using Iced and russh.

**What it does:**
- Dual-pane file browser (local/remote)
- Recursive upload/download with live progress bars
- SSH key auth, SSH agent, known_hosts verification
- Embedded SSH terminal
- 22 built-in themes (Nord, Gruvbox, Catppuccin, Dracula, Tokyo Night...)
- ~8MB binary, no runtime dependencies

**The interesting part - how it was built:**

I used agentic AI (Claude Code) as the primary implementer throughout. My role was mostly architect + PM: define tasks tightly, review output, unblock the agent when it hit Rust-specific walls.

A few things that surprised me:
- The agent handled borrow checker errors and lifetime issues better than expected when tasks were scoped small enough
- 263 tests were written mostly by the agent and caught real bugs
- The workflow breaks down fast with vague tasks. Small, explicit specs = good output. "Add file transfer" = chaos.

Not a magic story. More like: AI is a very fast junior who needs clear tickets. For Rust specifically, the tighter the scope, the better.

**Links:**
- GitHub: https://github.com/mylilcrowdi/Open-SFTP-Rust
- Download (macOS/Win/Linux): https://renemurrell.de/software/opensftp
- MIT licensed

Would love feedback from Rust folks, especially on the Iced patterns. Still early (v0.1.0) but daily-driver stable for me.
