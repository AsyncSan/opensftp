# Reddit r/selfhosted Post — Open SFTP

**Subreddit:** r/selfhosted

**Title:**
Open SFTP - native Rust SFTP client, MIT license, ~8MB, no Electron

---

## Post Body

Hey r/selfhosted,

Built this for my own server management workflow and figured others here might find it useful.

**Open SFTP** - a desktop SFTP client written in Rust.

Why another SFTP client? FileZilla is a dinosaur and everything else either needs Java, runs Electron, or is macOS-only.

**Features:**
- Dual-pane browser (local left, remote right)
- Drag-and-drop + recursive folder upload/download
- Progress bars with transfer log
- SSH key auth, SSH agent, known_hosts verification
- Connection bookmarks/profiles
- Remote mkdir, rename, delete, chmod via context menu
- Embedded SSH terminal
- 22 themes (Nord, Gruvbox, Catppuccin, Dracula, Tokyo Night, Solarized...)

**Download:**
Pre-built binaries for macOS (Universal), Windows, Linux (aarch64 + x86_64)
https://renemurrell.de/software/opensftp

MIT licensed. ~8MB binary. No dependencies.
GitHub: https://github.com/AsyncSan/opensftp

v0.1.0 - early but stable for daily use. Feedback welcome.

---

## Comment Strategy

This audience cares about self-hosted server workflows. Lean into the SSH/server management angle in comments.

**Suggested comment responses:**

**If asked "why not just use scp/rsync?"**
> For quick one-offs, sure. But when you're managing multiple servers daily, bookmarks, drag-and-drop, and a visual diff between local and remote save real time. Plus the embedded terminal means you don't need to juggle two windows.

**If asked about security/trust:**
> MIT licensed, so you can audit every line. Binaries are built via GitHub Actions from the same source. Uses russh under the hood - same SSH library that powers many production tools.

**If asked about Windows/Mac support:**
> Yep, universal macOS binary (Apple Silicon + Intel), Windows x64, and Linux x86_64 + aarch64 (Pi-friendly). Same ~8MB on all platforms.

**If someone mentions WinSCP/Cyberduck/Transmit:**
> Those are solid but platform-specific or closed-source. This runs everywhere, MIT licensed, and doesn't require any runtime dependencies.

**If asked about roadmap:**
> v0.1.0 is focused on daily-driver stability. Next up: file editing in-place, port tunneling, and multi-hop SSH. Issues/PRs welcome on GitHub.
