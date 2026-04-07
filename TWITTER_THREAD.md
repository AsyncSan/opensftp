# Twitter/X Thread - Build in Public

---

## Tweet 1 (hook)

I built a native SFTP client in Rust using agentic AI.

~8MB binary. 22 themes. Dual-pane. No Electron.

Here is what that dev process actually looked like. Thread:

*(attach: screenshot of dual-pane UI)*

---

## Tweet 2 (stack)

Stack:
- Rust
- Iced 0.13 for the GUI (native, reactive)
- russh for SSH/SFTP
- Single binary, instant startup
- macOS / Windows / Linux

No Python runtime. No 200MB install.

---

## Tweet 3 (workflow)

The workflow:

1. Write a task: "implement dual-pane browser, local left, remote right"
2. Agent explores codebase, implements, runs tests
3. I review the diff

I focused on product. The AI focused on Rust.

---

## Tweet 4 (surprise)

What surprised me: Rust + agentic AI actually works.

I expected borrow checker fights. The agent handled them reliably - as long as each task was scoped tightly.

263 tests written alongside features. Without me asking.

---

## Tweet 5 (shipped)

v0.1.0 shipped:

- Dual-pane file browser
- Live transfer progress
- SSH key/agent/password auth
- Embedded SSH terminal
- 22 themes (Dracula, Nord, Gruvbox, Catppuccin...)
- F5/F7/F8 keyboard shortcuts
- macOS, Windows, Linux

---

## Tweet 6 (insight)

Agentic AI is not autocomplete. It is a different model.

You become the architect + PM.
The agent becomes the implementer.

Your job: clear task specs + sharp review.

---

## Tweet 7 (CTA)

Open SFTP is MIT, free download.

https://renemurrell.de/software/opensftp
https://github.com/mylilcrowdi/Open-SFTP-Rust

Building with agentic AI? What has worked for you?

#Rust #BuildInPublic #AIEngineering
