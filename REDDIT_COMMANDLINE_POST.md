# Reddit Post — r/commandline

**Subreddit:** r/commandline  
**Status:** Ready to post  
**Best time:** Tuesday or Wednesday, 9-11am CET — day before PH launch

---

## Title

Open SFTP - native Rust SFTP client with dual-pane UI, no Electron, ~8MB binary

---

## Body

Just shipped **Open SFTP** - a free, open-source SFTP client for macOS, Windows, and Linux.

Built in Rust with Iced. No Electron, no Python runtime. The stripped binary is ~8MB and starts instantly.

**Features:**
- Dual-pane browser (local left, remote right) with breadcrumb navigation
- Recursive upload/download with live progress bars per file and overall
- SSH key, password, and ssh-agent authentication with known_hosts verification
- Embedded SSH terminal
- Connection profiles and bookmarks bar
- 22 built-in themes: Dracula, Nord, Gruvbox, Catppuccin, Tokyo Night, Solarized...
- Keyboard shortcuts (F5 refresh, F7 mkdir, F8 delete, Ctrl+T cycle themes)

It replaces my previous Python/Electron client that was using 400-500MB RAM. This one sits around 20-40MB while actively transferring.

**Download:** https://renemurrell.de/software/opensftp  
**GitHub:** https://github.com/AsyncSan/opensftp  

MIT licensed, free, no telemetry. Feedback welcome.
