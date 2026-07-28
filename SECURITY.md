# Security Policy

## Reporting a vulnerability

Please report security issues privately via [GitHub Security Advisories](https://github.com/AsyncSan/opensftp/security/advisories/new) rather than in a public issue.

You can expect an initial response within 7 days. This is a small project maintained in spare time — please set expectations accordingly. If you do not hear back within 14 days, feel free to open a public issue asking for a status update (without disclosing details).

## Supported versions

Only the latest release receives fixes. There are no long-term support branches.

## Known limitations you should be aware of

These are current, deliberate disclosures — not undisclosed vulnerabilities:

**Saved passwords are stored in plaintext.** Connection profiles are written as unencrypted JSON to your config directory (`~/.config/eurosftp/connections.json` on Linux, `~/Library/Application Support/eurosftp/connections.json` on macOS, `%APPDATA%\eurosftp\` on Windows). The file is created with your default umask, typically world-readable `0644`. There is no OS keychain integration. **If you save a password, treat that file as a secret.** Prefer SSH key files or the SSH agent, which never store a secret on disk through this app.

**macOS builds are not code-signed or notarized.** Gatekeeper will refuse to open the app on first launch; you have to right-click and choose *Open*. This also means the download is not tamper-evident beyond the checksum on the release page.

**Host key verification** reads `~/.ssh/known_hosts` and verifies against it. Unknown hosts prompt for confirmation.

## Scope

In scope: the SFTP client and its handling of credentials, host keys, and file transfers.

Out of scope: vulnerabilities in upstream dependencies (report those to [russh](https://github.com/warp-tech/russh) or [iced](https://github.com/iced-rs/iced) directly), and the three limitations listed above, which are already public.
