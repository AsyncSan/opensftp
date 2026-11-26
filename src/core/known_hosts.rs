use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use russh::keys::PublicKey;

/// Result of checking a host key against known_hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum HostKeyStatus {
    /// Key matches what we have on file.
    Known,
    /// Host not in known_hosts. Key was auto-added (TOFU).
    TofuAccepted,
    /// Key CHANGED since last connection. Possible MITM.
    Mismatch { expected: String, got: String },
    /// Host is marked as @revoked.
    Revoked,
}

/// Error during known_hosts operations.
#[derive(Debug)]
pub enum KnownHostsError {
    KeyMismatch { host: String, expected: String, got: String },
    Revoked(String),
    Io(std::io::Error),
}

impl fmt::Display for KnownHostsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyMismatch { host, expected, got } => {
                write!(
                    f,
                    "WARNING: HOST KEY CHANGED for {}! Expected {}, got {}. Possible MITM attack.",
                    host, expected, got
                )
            }
            Self::Revoked(host) => write!(f, "Host key for {} has been revoked", host),
            Self::Io(e) => write!(f, "Known hosts I/O error: {}", e),
        }
    }
}

impl std::error::Error for KnownHostsError {}

impl From<std::io::Error> for KnownHostsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Returns the path to ~/.ssh/known_hosts
pub fn known_hosts_path() -> PathBuf {
    // Tests never touch the real ~/.ssh/known_hosts.
    #[cfg(test)]
    return crate::core::config_dir().join(".ssh").join("known_hosts");
    #[cfg(not(test))]
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".ssh")
        .join("known_hosts")
}

/// Check a server's public key against known_hosts.
/// Uses TOFU (Trust On First Use): unknown hosts are auto-accepted and saved.
pub fn check_host_key(
    host: &str,
    port: u16,
    server_key: &PublicKey,
) -> Result<HostKeyStatus, KnownHostsError> {
    check_host_key_in(&known_hosts_path(), host, port, server_key)
}

fn check_host_key_in(
    path: &PathBuf,
    host: &str,
    port: u16,
    server_key: &PublicKey,
) -> Result<HostKeyStatus, KnownHostsError> {
    let host_pattern = if port == 22 {
        host.to_string()
    } else {
        format!("[{}]:{}", host, port)
    };

    let server_key_str = server_key
        .to_openssh()
        .unwrap_or_default();

    // Parse existing known_hosts. A host can have one line per key type
    // (ed25519, ecdsa, rsa), so only a line with the SAME key type and a
    // different key counts as a mismatch.
    let content = fs::read_to_string(path).unwrap_or_default();
    let server_key_parts = server_key_str.trim();
    let server_type = server_key_parts.split(' ').next().unwrap_or("");

    let mut known = false;
    let mut revoked = false;
    let mut mismatch: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // CA lines describe certificate signers, not host keys.
        if line.starts_with("@cert-authority ") {
            continue;
        }

        let is_revoked = line.starts_with("@revoked ");
        let line_data = if is_revoked {
            line.strip_prefix("@revoked ").unwrap_or(line).trim_start()
        } else {
            line
        };

        // Split: hostnames keytype base64key [comment]
        let mut parts = line_data.split_whitespace();
        let (Some(hosts_field), Some(key_type), Some(key_data)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };

        if !host_matches(hosts_field, &host_pattern) {
            continue;
        }

        let stored_key_str = format!("{} {}", key_type, key_data);
        let same_key = stored_key_str == server_key_parts;

        if is_revoked {
            // @revoked marks one specific key as never acceptable.
            if same_key {
                revoked = true;
            }
            continue;
        }

        if same_key {
            known = true;
        } else if key_type == server_type && mismatch.is_none() {
            mismatch = Some(key_type.to_string());
        }
    }

    if revoked {
        return Err(KnownHostsError::Revoked(host.to_string()));
    }
    if known {
        return Ok(HostKeyStatus::Known);
    }
    if let Some(expected) = mismatch {
        return Err(KnownHostsError::KeyMismatch {
            host: host.to_string(),
            expected,
            got: server_type.to_string(),
        });
    }

    // Host not found for this key type: TOFU, add it
    add_host_key(path, &host_pattern, &server_key_str)?;
    Ok(HostKeyStatus::TofuAccepted)
}

/// Check if a known_hosts host field matches our target.
fn host_matches(hosts_field: &str, target: &str) -> bool {
    for pattern in hosts_field.split(',') {
        let pattern = pattern.trim();
        if pattern.starts_with("|1|") {
            // Hashed entry (HashKnownHosts yes): |1|base64(salt)|base64(hmac-sha1(salt, host))
            if hashed_matches(pattern, target) {
                return true;
            }
        } else if pattern.starts_with('!') {
            // Negation: if it matches, this entry does NOT apply
            if pattern_matches(pattern.strip_prefix('!').unwrap_or(pattern), target) {
                return false;
            }
        } else if pattern_matches(pattern, target) {
            return true;
        }
    }
    false
}

/// Match a hashed known_hosts entry against the target host pattern.
fn hashed_matches(entry: &str, target: &str) -> bool {
    use base64::Engine;
    use hmac::{Hmac, Mac};

    let mut parts = entry.split('|').skip(2);
    let (Some(salt_b64), Some(hash_b64)) = (parts.next(), parts.next()) else {
        return false;
    };
    let engine = base64::engine::general_purpose::STANDARD;
    let (Ok(salt), Ok(hash)) = (engine.decode(salt_b64), engine.decode(hash_b64)) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<sha1::Sha1>::new_from_slice(&salt) else {
        return false;
    };
    mac.update(target.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

/// Simple glob matching (* and ? only).
fn pattern_matches(pattern: &str, target: &str) -> bool {
    let mut p = pattern.chars().peekable();
    let mut t = target.chars().peekable();

    while p.peek().is_some() || t.peek().is_some() {
        match p.peek() {
            Some('*') => {
                p.next();
                if p.peek().is_none() {
                    return true; // trailing * matches everything
                }
                // Try matching rest of pattern at every position
                let remaining_pattern: String = p.collect();
                let remaining_target: String = t.collect();
                for i in 0..=remaining_target.len() {
                    if pattern_matches(&remaining_pattern, &remaining_target[i..]) {
                        return true;
                    }
                }
                return false;
            }
            Some('?') => {
                if t.peek().is_none() {
                    return false;
                }
                p.next();
                t.next();
            }
            Some(&pc) => {
                if t.peek() != Some(&pc) {
                    return false;
                }
                p.next();
                t.next();
            }
            None => return false,
        }
    }
    true
}

/// Append a new host key entry to the known_hosts file.
fn add_host_key(path: &PathBuf, host_pattern: &str, key_str: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    writeln!(file, "{} {}", host_pattern, key_str)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_matches_exact() {
        assert!(host_matches("example.com", "example.com"));
        assert!(!host_matches("example.com", "other.com"));
    }

    #[test]
    fn host_matches_multiple() {
        assert!(host_matches("foo.com,bar.com", "bar.com"));
        assert!(!host_matches("foo.com,bar.com", "baz.com"));
    }

    #[test]
    fn host_matches_glob_star() {
        assert!(host_matches("*.example.com", "server.example.com"));
        assert!(!host_matches("*.example.com", "example.com"));
    }

    #[test]
    fn host_matches_glob_question() {
        assert!(host_matches("server?.example.com", "server1.example.com"));
        assert!(!host_matches("server?.example.com", "server12.example.com"));
    }

    #[test]
    fn host_matches_negation() {
        assert!(!host_matches("!banned.com,*.com", "banned.com"));
        assert!(host_matches("!banned.com,*.com", "allowed.com"));
    }

    #[test]
    fn host_matches_with_port() {
        assert!(host_matches("[example.com]:2222", "[example.com]:2222"));
        assert!(!host_matches("[example.com]:2222", "[example.com]:22"));
    }

    #[test]
    fn pattern_matches_exact() {
        assert!(pattern_matches("abc", "abc"));
        assert!(!pattern_matches("abc", "abcd"));
        assert!(!pattern_matches("abcd", "abc"));
    }

    #[test]
    fn pattern_matches_star() {
        assert!(pattern_matches("*", "anything"));
        assert!(pattern_matches("*", ""));
        assert!(pattern_matches("a*c", "abc"));
        assert!(pattern_matches("a*c", "aXYZc"));
        assert!(!pattern_matches("a*c", "aXYZd"));
    }

    #[test]
    fn pattern_matches_question() {
        assert!(pattern_matches("a?c", "abc"));
        assert!(!pattern_matches("a?c", "ac"));
        assert!(!pattern_matches("a?c", "aXXc"));
    }

    #[test]
    fn host_matches_hashed_entry() {
        // Same format ssh-keygen -H writes: |1|base64(salt)|base64(HMAC-SHA1(salt, host))
        let entry = hashed_entry_for("example.com");
        assert!(host_matches(&entry, "example.com"));
        assert!(!host_matches(&entry, "other.com"));
        let entry_port = hashed_entry_for("[example.com]:2222");
        assert!(host_matches(&entry_port, "[example.com]:2222"));
        assert!(!host_matches(&entry_port, "example.com"));
    }

    fn hashed_entry_for(host: &str) -> String {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        let salt = [7u8; 20];
        let mut mac = Hmac::<sha1::Sha1>::new_from_slice(&salt).unwrap();
        mac.update(host.as_bytes());
        let hash = mac.finalize().into_bytes();
        let e = base64::engine::general_purpose::STANDARD;
        format!("|1|{}|{}", e.encode(salt), e.encode(hash))
    }

    const ED25519_A: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDsHmgN/VYf0Dut0ii+CKvsYSUm+TfAV6imtQ/9OZZgT";
    const ED25519_B: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJM3LtHslKL+OzeIk86Dw7bkJk7ZbjgRqeP/MndtTsiF";
    const ECDSA: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBGbDG7B5Wj7urHomrGau1sggNa48dcRYrPw8CR5a+yVzrQZxTd6ZiH56K4JymBcEsSo/81kq7LrEZ+BB+QoMhnQ=";

    fn temp_known_hosts(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("opensftp-test-{}-{}", std::process::id(), name));
        fs::write(&path, content).unwrap();
        path
    }

    fn key(s: &str) -> PublicKey {
        PublicKey::from_openssh(s).unwrap()
    }

    #[test]
    fn other_key_type_is_not_a_mismatch() {
        let path = temp_known_hosts("types", &format!("example.com {}\nexample.com {}\n", ECDSA, ED25519_A));
        let status = check_host_key_in(&path, "example.com", 22, &key(ED25519_A)).unwrap();
        assert_eq!(status, HostKeyStatus::Known);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn same_type_different_key_is_a_mismatch() {
        let path = temp_known_hosts("mismatch", &format!("example.com {}\n", ED25519_A));
        let err = check_host_key_in(&path, "example.com", 22, &key(ED25519_B)).unwrap_err();
        assert!(matches!(err, KnownHostsError::KeyMismatch { .. }));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn hashed_line_is_known() {
        let path = temp_known_hosts("hashed", &format!("{} {}\n", hashed_entry_for("example.com"), ED25519_A));
        let status = check_host_key_in(&path, "example.com", 22, &key(ED25519_A)).unwrap();
        assert_eq!(status, HostKeyStatus::Known);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn revoked_only_blocks_that_key() {
        let path = temp_known_hosts("revoked", &format!("@revoked example.com {}\nexample.com {}\n", ED25519_A, ED25519_B));
        let err = check_host_key_in(&path, "example.com", 22, &key(ED25519_A)).unwrap_err();
        assert!(matches!(err, KnownHostsError::Revoked(_)));
        let status = check_host_key_in(&path, "example.com", 22, &key(ED25519_B)).unwrap();
        assert_eq!(status, HostKeyStatus::Known);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn unknown_host_is_added() {
        let path = temp_known_hosts("tofu", "");
        let status = check_host_key_in(&path, "new.example.com", 2222, &key(ED25519_A)).unwrap();
        assert_eq!(status, HostKeyStatus::TofuAccepted);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("[new.example.com]:2222 ssh-ed25519 "));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn known_hosts_path_is_correct() {
        let path = known_hosts_path();
        assert!(path.to_string_lossy().contains(".ssh"));
        assert!(path.to_string_lossy().ends_with("known_hosts"));
    }
}
