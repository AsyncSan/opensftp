//! Tabs, paths and sort order, saved on every change and restored on start.
//!
//! Only what is needed to reopen a tab is stored: the server (host, port,
//! user), the local and remote path and the sort column. Credentials stay in
//! the connection profiles; this file never holds a password.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::file_panel::SortColumn;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedServer {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub remote_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTab {
    /// None for a tab that was not connected.
    pub server: Option<SavedServer>,
    pub local_path: String,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SavedSession {
    pub tabs: Vec<SavedTab>,
    pub active_tab: usize,
}

impl SavedSession {
    pub fn path() -> PathBuf {
        let mut p = crate::core::config_dir();
        p.push("eurosftp");
        p.push("session.json");
        p
    }

    pub fn load() -> Option<Self> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        serde_json::from_str(&s).ok()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            // Write to a temp file first so a crash never leaves half a file.
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json() {
        let s = SavedSession {
            tabs: vec![
                SavedTab {
                    server: Some(SavedServer {
                        host: "example.com".into(),
                        port: 2222,
                        username: "deploy".into(),
                        remote_path: "/var/www".into(),
                    }),
                    local_path: "/tmp".into(),
                    sort_column: SortColumn::Size,
                    sort_ascending: false,
                },
                SavedTab {
                    server: None,
                    local_path: "/".into(),
                    sort_column: SortColumn::Name,
                    sort_ascending: true,
                },
            ],
            active_tab: 1,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SavedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
