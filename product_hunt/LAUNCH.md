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

> NOTE: Replace [DEV.TO_URL] with the live Dev.to article URL before posting.

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

Built almost entirely with agentic AI (Claude Code). Scoped tasks, AI implements, I review diffs. Full write-up: [DEV.TO_URL]

MIT licensed. Free.
```

---

## Assets Checklist

### Icon
- [x] `icon_256.png` - 256x256 app icon (dual-pane + lock motif)

### Screenshots (1270x760 recommended, max 3840x2160)
- [x] `screenshots/01_connect_dialog.png` - Connect dialog
- [x] `screenshots/02_dual_pane.png` - Dual-pane file browser
- [x] `screenshots/03_transfer_progress.png` - Transfer progress bars (committed 9f38a97)
- [x] `screenshots/04_theme_switcher.png` - Theme switcher (committed 9f38a97)

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

### BLOCKING - must complete before posting
- [ ] Publish Dev.to article (DEVTO_ARTICLE.md) at dev.to and get live URL
- [ ] Replace [DEV.TO_URL] in maker comment below with real URL

### Ready
- [x] All 4 screenshots in `product_hunt/screenshots/`
- [x] Icon: `icon_256.png`
- [ ] Thumbnail set to `icon_256.png` in PH form
- [ ] Gallery uploaded: screenshots in order 01-04
- [ ] Tagline: "Native Rust SFTP client - no Electron, ~8MB, 22 themes" (54 chars, within limit)
- [ ] Website URL: https://renemurrell.de/software/opensftp
- [ ] Topics: Developer Tools, Open Source, Productivity
- [ ] Maker comment posted immediately after launch (update [DEV.TO_URL] first)
- [ ] GitHub repo linked: https://github.com/mylilcrowdi/Open-SFTP-Rust
- [ ] Post at exactly 12:01am PST on Tuesday or Wednesday

---

## Pre-Launch Outreach (day before launch)

All drafts are ready in the repo root:

| Community | Draft file | Notes |
|---|---|---|
| r/rust | `REDDIT_RUST_POST.md` | AI workflow angle |
| r/commandline | `REDDIT_COMMANDLINE_POST.md` | Feature/binary size angle |
| r/selfhosted | `REDDIT_SELFHOSTED_POST.md` | Self-hosted tool angle |
| Hacker News | `HN_POST.md` | Show HN, link GitHub |
| Twitter/X | `TWITTER_THREAD.md` | Thread with screenshots |

**Goal:** Ask supporters to upvote during the first 2 hours after launch (that's when PH ranking is decided).

Example ask to add to outreach posts:
> "Launching on Product Hunt [date] - if you find it useful, an upvote in the first hour helps a lot: [PH_URL]"
