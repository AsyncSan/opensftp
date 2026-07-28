use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JumpHostConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_host: Option<JumpHostConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethod {
    Password(String),
    KeyFile(PathBuf),
    Agent,
}

impl Default for JumpHostConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            username: String::new(),
            auth: AuthMethod::Agent,
        }
    }
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            auth: AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        }
    }
}

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

fn default_port() -> u16 {
    22
}

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
            favorite: false,
            jump_host: None,
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

        if profiles.is_empty() {
            let py_path = Self::python_profiles_path();
            if let Ok(content) = std::fs::read_to_string(&py_path)
                && let Ok(py_profiles) = serde_json::from_str::<Vec<PythonProfile>>(&content) {
                    profiles = py_profiles
                        .into_iter()
                        .filter(|p| !p.host.is_empty())
                        .map(|p| p.into_profile())
                        .collect();
                    let _ = Self::save_all(&profiles);
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
            .map_err(std::io::Error::other)?;
        std::fs::write(&path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_host_roundtrip() {
        let profile = ConnectionProfile {
            name: "prod".into(),
            host: "app.internal".into(),
            port: 22,
            username: "deploy".into(),
            auth: AuthMethod::Agent,
            favorite: true,
            jump_host: Some(JumpHostConfig {
                host: "bastion.example.com".into(),
                port: 2222,
                username: "proxy".into(),
                auth: AuthMethod::Password("jump-secret".into()),
            }),
        };
        let restored: ConnectionProfile = serde_json::from_str(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert!(restored.favorite);
        let jump = restored.jump_host.unwrap();
        assert_eq!(jump.host, "bastion.example.com");
        assert_eq!(jump.port, 2222);
    }
}
