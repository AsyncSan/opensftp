# Contributing

Thanks for considering a contribution. This is a small, opinionated project — the goal is a fast SFTP client that does a few things well, not feature parity with FileZilla.

## Before you start

For anything larger than a bug fix, **open an issue first** and describe what you want to change. That avoids you writing code that gets rejected on scope grounds. Small fixes, typos, and obvious bugs: just send the PR.

Things likely to be accepted: bug fixes, platform support (especially Windows and Linux x86_64 builds), performance work, accessibility, test isolation fixes, translations.

Things likely to be declined: new protocols (FTP, WebDAV, cloud storage backends), plugin systems, anything that adds a runtime dependency or meaningfully grows the binary.

## Development

Requires Rust 1.85+ and a working C linker.

```bash
git clone https://github.com/AsyncSan/opensftp.git
cd opensftp
cargo run
```

```bash
cargo test
cargo clippy
cargo fmt
```

> **Warning:** the test suite currently reads and writes your real config directory, so `cargo test` will overwrite your saved connection profiles and theme setting. Back them up first. Fixing this (injecting a config path instead of resolving it globally) is a welcome contribution.

## Pull requests

- One logical change per PR.
- Add tests for behaviour changes in `core/`. UI changes are harder to test; a description of what you clicked is fine.
- Run `cargo fmt` and make sure `cargo clippy` does not add new warnings.
- Describe what you changed and why. Screenshots for UI changes.

## Commit messages

Conventional-ish prefixes are used but not enforced: `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`.

## Licence

By contributing you agree that your contribution is licensed under the MIT License, the same terms as the rest of the project.
