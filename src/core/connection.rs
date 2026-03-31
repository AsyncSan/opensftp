use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Password(String),
    KeyFile(PathBuf),
    Agent,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            auth: AuthMethod::Agent,
        }
    }
}

/// Legacy Python sftp-ui connection format for import compatibility.
#[derive(Debug, Clone, Deserialize)]
struct PythonProfile {
    name: String,
    host: String,
    #[serde(default)]
    user: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    key_path: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    use_agent: bool,
}

fn default_port() -> u16 { 22 }

impl PythonProfile {
    fn into_profile(self) -> ConnectionProfile {
        let auth = if self.use_agent {
            AuthMethod::Agent
        } else if let Some(ref key) = self.key_path {
            if !key.is_empty() {
                AuthMethod::KeyFile(PathBuf::from(key))
            } else if let Some(ref pw) = self.password {
                if !pw.is_empty() && pw != "__keychain__" {
                    AuthMethod::Password(pw.clone())
                } else {
                    AuthMethod::Agent
                }
            } else {
                AuthMethod::Agent
            }
        } else if let Some(ref pw) = self.password {
            if !pw.is_empty() && pw != "__keychain__" {
                AuthMethod::Password(pw.clone())
            } else {
                AuthMethod::Agent
            }
        } else {
            AuthMethod::Agent
        };

        ConnectionProfile {
            name: self.name,
            host: self.host,
            port: self.port,
            username: self.user,
            auth,
        }
    }
}

impl ConnectionProfile {
    pub fn profiles_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("eurosftp");
        p.push("connections.json");
        p
    }

    /// Path to the Python sftp-ui connections file.
    fn python_profiles_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("sftp-ui");
        p.push("connections.json");
        p
    }

    pub fn load_all() -> Vec<Self> {
        let path = Self::profiles_path();
        let mut profiles: Vec<Self> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        // Import from Python sftp-ui if we have no profiles yet
        if profiles.is_empty() {
            let py_path = Self::python_profiles_path();
            if let Ok(content) = std::fs::read_to_string(&py_path) {
                if let Ok(py_profiles) = serde_json::from_str::<Vec<PythonProfile>>(&content) {
                    profiles = py_profiles
                        .into_iter()
                        .filter(|p| !p.host.is_empty())
                        .map(|p| p.into_profile())
                        .collect();
                    // Save imported profiles to our own path
                    let _ = Self::save_all(&profiles);
                }
            }
        }

        profiles
    }

    pub fn save_all(profiles: &[Self]) -> std::io::Result<()> {
        let path = Self::profiles_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(profiles)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_port_22() {
        let p = ConnectionProfile::default();
        assert_eq!(p.port, 22);
        assert!(p.name.is_empty());
        assert!(p.host.is_empty());
        assert!(p.username.is_empty());
        assert!(matches!(p.auth, AuthMethod::Agent));
    }

    #[test]
    fn serialize_password_roundtrip() {
        let profile = ConnectionProfile {
            name: "test-server".into(),
            host: "192.168.1.1".into(),
            port: 2222,
            username: "admin".into(),
            auth: AuthMethod::Password("secret".into()),
        };

        let json = serde_json::to_string(&profile).unwrap();
        let restored: ConnectionProfile = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.name, "test-server");
        assert_eq!(restored.host, "192.168.1.1");
        assert_eq!(restored.port, 2222);
        assert_eq!(restored.username, "admin");
        assert!(matches!(restored.auth, AuthMethod::Password(ref pw) if pw == "secret"));
    }

    #[test]
    fn serialize_keyfile_roundtrip() {
        let profile = ConnectionProfile {
            name: "key-server".into(),
            host: "10.0.0.1".into(),
            port: 22,
            username: "deploy".into(),
            auth: AuthMethod::KeyFile(PathBuf::from("/home/user/.ssh/id_ed25519")),
        };

        let json = serde_json::to_string(&profile).unwrap();
        let restored: ConnectionProfile = serde_json::from_str(&json).unwrap();

        assert!(matches!(
            restored.auth,
            AuthMethod::KeyFile(ref p) if p == &PathBuf::from("/home/user/.ssh/id_ed25519")
        ));
    }

    #[test]
    fn serialize_multiple_profiles() {
        let profiles = vec![
            ConnectionProfile {
                name: "server-a".into(),
                host: "a.example.com".into(),
                port: 22,
                username: "user".into(),
                auth: AuthMethod::Agent,
            },
            ConnectionProfile {
                name: "server-b".into(),
                host: "b.example.com".into(),
                port: 8022,
                username: "root".into(),
                auth: AuthMethod::Password("pw".into()),
            },
        ];

        let json = serde_json::to_string(&profiles).unwrap();
        let restored: Vec<ConnectionProfile> = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].name, "server-a");
        assert_eq!(restored[1].port, 8022);
    }

    #[test]
    fn profiles_path_ends_with_connections_json() {
        let path = ConnectionProfile::profiles_path();
        assert_eq!(path.file_name().unwrap(), "connections.json");
        assert!(path.to_string_lossy().contains("eurosftp"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        use std::env;

        // Use a temp dir to avoid touching real config
        let tmp = env::temp_dir().join("opensftp_test_profiles");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let test_path = tmp.join("connections.json");
        let profiles = vec![ConnectionProfile {
            name: "roundtrip".into(),
            host: "localhost".into(),
            port: 22,
            username: "test".into(),
            auth: AuthMethod::Password("pass".into()),
        }];

        // Write directly to test path
        let json = serde_json::to_string_pretty(&profiles).unwrap();
        std::fs::write(&test_path, &json).unwrap();

        // Read back
        let content = std::fs::read_to_string(&test_path).unwrap();
        let loaded: Vec<ConnectionProfile> = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "roundtrip");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn python_profile_password_import() {
        let json = r#"[{
            "name": "prod-server",
            "host": "10.0.0.1",
            "user": "deploy",
            "port": 2222,
            "password": "s3cret",
            "use_agent": false
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter().map(|p| p.into_profile()).collect();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "prod-server");
        assert_eq!(profiles[0].host, "10.0.0.1");
        assert_eq!(profiles[0].port, 2222);
        assert_eq!(profiles[0].username, "deploy");
        assert!(matches!(profiles[0].auth, AuthMethod::Password(ref pw) if pw == "s3cret"));
    }

    #[test]
    fn python_profile_keyfile_import() {
        let json = r#"[{
            "name": "key-server",
            "host": "example.com",
            "user": "admin",
            "port": 22,
            "key_path": "/home/admin/.ssh/id_ed25519",
            "use_agent": false
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter().map(|p| p.into_profile()).collect();
        assert!(matches!(
            profiles[0].auth,
            AuthMethod::KeyFile(ref p) if p.to_string_lossy() == "/home/admin/.ssh/id_ed25519"
        ));
    }

    #[test]
    fn python_profile_agent_import() {
        let json = r#"[{
            "name": "agent-server",
            "host": "example.com",
            "user": "root",
            "port": 22,
            "use_agent": true
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter().map(|p| p.into_profile()).collect();
        assert!(matches!(profiles[0].auth, AuthMethod::Agent));
    }

    #[test]
    fn python_profile_keychain_sentinel_falls_back_to_agent() {
        let json = r#"[{
            "name": "keychain-server",
            "host": "example.com",
            "user": "user",
            "port": 22,
            "password": "__keychain__",
            "use_agent": false
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter().map(|p| p.into_profile()).collect();
        assert!(matches!(profiles[0].auth, AuthMethod::Agent));
    }

    #[test]
    fn python_profile_empty_host_filtered() {
        let json = r#"[
            {"name": "cloud-only", "host": "", "user": "", "port": 22, "use_agent": false},
            {"name": "real", "host": "server.com", "user": "u", "port": 22, "use_agent": true}
        ]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter()
            .filter(|p| !p.host.is_empty())
            .map(|p| p.into_profile())
            .collect();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "real");
    }

    #[test]
    fn python_profile_extra_fields_ignored() {
        let json = r#"[{
            "name": "full",
            "host": "x.com",
            "user": "u",
            "port": 22,
            "use_agent": false,
            "id": "abc-123",
            "favorite": true,
            "group": "Production",
            "last_connected": 1700000000.0,
            "protocol": "sftp",
            "cloud": null,
            "keepalive_interval": 30,
            "tunnel": null,
            "key_passphrase": null
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        let profiles: Vec<ConnectionProfile> = py.into_iter().map(|p| p.into_profile()).collect();
        assert_eq!(profiles[0].name, "full");
        assert_eq!(profiles[0].host, "x.com");
    }

    #[test]
    fn python_profile_default_port() {
        let json = r#"[{
            "name": "no-port",
            "host": "server.com",
            "user": "user"
        }]"#;
        let py: Vec<PythonProfile> = serde_json::from_str(json).unwrap();
        assert_eq!(py[0].port, 22);
    }
}
