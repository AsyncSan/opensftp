# Open SFTP

**A fast, native SFTP client written in Rust.**
Built with [Iced](https://iced.rs) for the GUI and [russh](https://github.com/warp-tech/russh) for SSH/SFTP. No Electron. No runtime overhead. Single binary, ~8 MB.

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

![Open SFTP file browser](screenshots/10_final_browser.png)

---

## Download

Pre-built binaries for macOS, Windows and Linux. No installation required.

| Platform | Download | Notes |
|----------|----------|-------|
| macOS | [OpenSFTP-0.1.0-macOS.zip](https://github.com/mylilcrowdi/Open-SFTP-Rust/releases/latest) | Universal (Intel + Apple Silicon). Unzip and double-click. |
| Windows | [OpenSFTP-0.1.0-Windows.exe](https://github.com/mylilcrowdi/Open-SFTP-Rust/releases/latest) | Standalone .exe. Double-click to run. |
| Linux | [OpenSFTP-0.1.0-linux-x86_64.tar.gz](https://github.com/mylilcrowdi/Open-SFTP-Rust/releases/latest) | Extract and run: `tar xzf OpenSFTP-*.tar.gz && ./opensftp` |

On first launch, the app asks whether you want to run it **portable** (just the binary) or **install it** to your system (adds to Applications/Start Menu/Dock). You can change this anytime in Settings.

All releases: [github.com/mylilcrowdi/Open-SFTP-Rust/releases](https://github.com/mylilcrowdi/Open-SFTP-Rust/releases)

---

## Contents

1. [Download](#download)
2. [Screenshots](#screenshots)
3. [Features](#features)
4. [Build from Source](#build-from-source)
5. [Tests](#tests)
6. [Project Structure](#project-structure)
7. [License](#license)

---

## Screenshots

| File browser (connected) | Connection dialog |
|---|---|
| ![browser](screenshots/10_final_browser.png) | ![connect](screenshots/10_final_connect.png) |

---

## Features

**Dual-pane file browser:** local filesystem on the left, remote on the right. Navigate with double-click, breadcrumb path bar, or keyboard shortcuts.

**Transfers:** upload and download files and entire directories recursively. Live progress bar with byte counters. Transfer log with success/failure status.

**Authentication:** password, SSH key file, or SSH agent. Reads known hosts from `~/.ssh/known_hosts` with host key verification.

**Connection profiles:** save, load, and manage server connections. Favorites shown as bookmarks bar for quick connect.

**Remote operations:** create directories, rename files, delete files and folders, change permissions (chmod). Right-click context menu for quick access.

**Embedded SSH terminal:** run commands on the remote server without leaving the app.

**22 built-in themes:** Dark, Light, Nord, Dracula, Solarized, Gruvbox, Catppuccin (Latte/Frappe/Macchiato/Mocha), Tokyo Night, Kanagawa, Moonfly, Nightfly, Oxocarbon, Ferra. Switch with Ctrl+T, persisted across sessions.

**First-run setup:** choose portable mode or install to system (desktop shortcut, app menu). Changeable in Settings.

**Keyboard driven:** F5 refresh, F7 mkdir, F8 delete, Ctrl+Q quit, Ctrl+T cycle theme. Multi-select with Ctrl+Click and Shift+Click.

**Small binary:** release build with LTO and strip produces a single static executable under 10 MB. No runtime dependencies.

---

## Build from Source

Requires Rust 1.85+ and a working C linker.

```bash
git clone https://github.com/mylilcrowdi/Open-SFTP-Rust.git
cd Open-SFTP-Rust
cargo run
```

### Release Build

```bash
./build-release.sh
```

This builds optimized binaries for your current platform:
- **macOS:** `.app` bundle as `.zip` (Universal Binary: Intel + Apple Silicon)
- **Linux:** `.tar.gz` with standalone binary
- **Windows:** `.exe`

Output goes to `dist/`.

---

## Tests

```bash
cargo test
```

263 tests covering core modules: connection profiles, known hosts, SFTP operations, transfer engine, theme persistence, settings, desktop integration, and UI.

---

## Project Structure

```
src/
├── main.rs                      Entry point, Iced application setup
├── core/
│   ├── connection.rs            Connection profiles (save/load/favorites)
│   ├── desktop_integration.rs   System install/uninstall (Linux/macOS/Windows)
│   ├── known_hosts.rs           SSH known_hosts parsing and verification
│   ├── sftp.rs                  russh SFTP wrapper, async file operations
│   └── transfer.rs              Upload/download engine with progress
├── theme/
│   └── mod.rs                   22 themes, settings persistence, install mode
└── ui/
    ├── app.rs                   Main state, messages, update/view, dialogs
    ├── connect_dialog.rs        Connection form (host, port, auth method)
    ├── file_panel.rs            File list widget with sorting and selection
    └── helpers.rs               Formatting utilities (size, permissions, timestamps)
```

---

## License

MIT. See [Cargo.toml](Cargo.toml) for details.
