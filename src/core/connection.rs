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

impl ConnectionProfile {
    pub fn profiles_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("eurosftp");
        p.push("connections.json");
        p
    }

    pub fn load_all() -> Vec<Self> {
        let path = Self::profiles_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
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
}
