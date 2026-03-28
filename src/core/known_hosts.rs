use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use russh::keys::PublicKey;

/// Result of checking a host key against known_hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    let path = known_hosts_path();
    let host_pattern = if port == 22 {
        host.to_string()
    } else {
        format!("[{}]:{}", host, port)
    };

    let server_key_str = server_key
        .to_openssh()
        .unwrap_or_default();

    // Parse existing known_hosts
    let content = fs::read_to_string(&path).unwrap_or_default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let is_revoked = line.starts_with("@revoked ");
        let line_data = if is_revoked {
            line.strip_prefix("@revoked ").unwrap_or(line)
        } else {
            line
        };

        // Split: hostnames keytype base64key [comment]
        let mut parts = line_data.splitn(3, ' ');
        let hosts_field = match parts.next() {
            Some(h) => h,
            None => continue,
        };
        let key_rest = match parts.next() {
            Some(k) => match parts.next() {
                Some(rest) => format!("{} {}", k, rest.split_whitespace().next().unwrap_or(rest)),
                None => continue,
            },
            None => continue,
        };

        // Check if this entry matches our host
        if !host_matches(hosts_field, &host_pattern) {
            continue;
        }

        // Reconstruct the key string for comparison (keytype + base64)
        let stored_key_str = key_rest.trim();

        if is_revoked {
            // Any key for a revoked host is rejected
            return Err(KnownHostsError::Revoked(host.to_string()));
        }

        // Compare key type + key data
        // server_key_str is "keytype base64data"
        let server_key_parts = server_key_str.trim();
        if stored_key_str == server_key_parts {
            return Ok(HostKeyStatus::Known);
        }

        // Same host, different key = MISMATCH
        let stored_short = stored_key_str.split(' ').next().unwrap_or("unknown");
        let server_short = server_key_parts.split(' ').next().unwrap_or("unknown");
        return Err(KnownHostsError::KeyMismatch {
            host: host.to_string(),
            expected: stored_short.to_string(),
            got: server_short.to_string(),
        });
    }

    // Host not found: TOFU, add it
    add_host_key(&path, &host_pattern, &server_key_str)?;
    Ok(HostKeyStatus::TofuAccepted)
}

/// Check if a known_hosts host field matches our target.
fn host_matches(hosts_field: &str, target: &str) -> bool {
    for pattern in hosts_field.split(',') {
        let pattern = pattern.trim();
        if pattern.starts_with('!') {
            // Negation: if it matches, this entry does NOT apply
            if pattern_matches(&pattern[1..], target) {
                return false;
            }
        } else if pattern_matches(pattern, target) {
            return true;
        }
    }
    false
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
    fn known_hosts_path_is_correct() {
        let path = known_hosts_path();
        assert!(path.to_string_lossy().contains(".ssh"));
        assert!(path.to_string_lossy().ends_with("known_hosts"));
    }
}
