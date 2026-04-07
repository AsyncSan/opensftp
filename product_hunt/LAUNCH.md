# Product Hunt Launch - Open SFTP

## Launch URL
https://www.producthunt.com/posts/new

## Product Details

**Name:** Open SFTP

**Tagline:**
```
Native Rust SFTP client - no Electron, ~8MB, 22 themes
```

**Website URL:**
```
https://renemurrell.de/software/opensftp
```

**Topics:** Developer Tools, Open Source, Productivity, Design Tools

---

## Maker Comment (post as first comment immediately after launch)

```
Open SFTP is a native desktop SFTP client built in Rust. No Electron, no Python runtime - just an ~8MB binary that starts instantly on macOS, Windows, and Linux.

Key features:
- Dual-pane file browser (local + remote side by side)
- Recursive upload/download with live progress bars
- SSH key, password, and agent auth + known_hosts verification
- Embedded SSH terminal
- Connection profiles and bookmarks bar
- 22 built-in themes: Dracula, Nord, Gruvbox, Catppuccin, Tokyo Night...
- Keyboard shortcuts (F5 refresh, F7 mkdir, F8 delete, Ctrl+T cycle themes)

Built almost entirely with agentic AI (Claude Code). Scoped tasks, AI implements, I review diffs. Cross-platform native desktop app shipped in days.

MIT licensed. Free.
```

---

## Assets Checklist

### Icon
- [x] `icon_256.png` - 256x256 app icon (dual-pane + lock motif)

### Screenshots (1270x760 recommended, max 3840x2160)
- [x] `screenshots/01_connect_dialog.png` - Connect dialog
- [x] `screenshots/02_dual_pane.png` - Dual-pane file browser
- [ ] `screenshots/03_transfer_progress.png` - **CAPTURE NEEDED**: Show active upload/download with progress bars
- [ ] `screenshots/04_theme_switcher.png` - **CAPTURE NEEDED**: Theme selector open (Ctrl+T), multiple themes visible

### Optional
- [ ] `opensftp_demo.gif` - Short screen recording (connect -> browse -> transfer), ~15-30 sec

---

## Capturing Missing Screenshots

Run the app and capture these scenes:

```bash
# Build latest
cd ~/eurosftp && cargo build --release

# Run
./target/release/opensftp
```

**Screenshot 3 - Transfer Progress:**
- Connect to a server
- Select a large folder and hit Download
- Screenshot while transfer is in progress (progress bar visible)
- Save as `product_hunt/screenshots/03_transfer_progress.png`

**Screenshot 4 - Theme Switcher:**
- With browser open, press Ctrl+T a few times to cycle themes
- Screenshot on a visually distinct theme (Dracula or Nord look great)
- Save as `product_hunt/screenshots/04_theme_switcher.png`

---

## Launch Day Checklist

- [ ] All 4 screenshots captured and in `product_hunt/screenshots/`
- [ ] Thumbnail set to `icon_256.png`
- [ ] Gallery uploaded: screenshots in order 01-04
- [ ] Tagline double-checked (max 60 chars): "Native Rust SFTP client - no Electron, ~8MB, 22 themes"
- [ ] Website URL set: https://renemurrell.de/software/opensftp
- [ ] Topics selected
- [ ] Maker comment text copied and ready to paste
- [ ] GitHub repo linked: https://github.com/mylilcrowdi/Open-SFTP-Rust
- [ ] Launch on Tuesday or Wednesday for best visibility
- [ ] Notify GitHub stargazers before launch for upvote momentum

---

## Pre-Launch Momentum

Post in these communities the day before:
- GitHub Discussions / Releases announcement
- Reddit: r/rust, r/commandline, r/selfhosted
- Hacker News: "Show HN: Open SFTP - native Rust SFTP client, ~8MB, 22 themes"
- Twitter/X post with GIF demo
