use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TwitterCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub access_token: String,
    pub access_token_secret: String,
}

#[derive(Debug)]
pub enum TwitterCredentialsError {
    NoCredentialsFile(Vec<PathBuf>),
    MissingVars(Vec<&'static str>),
    IoError(std::io::Error),
}

impl std::fmt::Display for TwitterCredentialsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TwitterCredentialsError::NoCredentialsFile(paths) => {
                let paths_str: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
                write!(f, "No Twitter credentials file found. Checked: {}", paths_str.join(", "))
            }
            TwitterCredentialsError::MissingVars(vars) => {
                write!(f, "Missing Twitter credential variables: {}", vars.join(", "))
            }
            TwitterCredentialsError::IoError(e) => {
                write!(f, "Failed to read credentials file: {e}")
            }
        }
    }
}

impl From<std::io::Error> for TwitterCredentialsError {
    fn from(e: std::io::Error) -> Self {
        TwitterCredentialsError::IoError(e)
    }
}

/// Load Twitter OAuth 1.0a credentials from ~/.openclaw/secrets/twitter.env
/// or ~/.openclaw/secrets/opensftp.env (fallback).
///
/// Expected variables:
///   TWITTER_API_KEY, TWITTER_API_SECRET,
///   TWITTER_ACCESS_TOKEN, TWITTER_ACCESS_TOKEN_SECRET
pub fn load() -> Result<TwitterCredentials, TwitterCredentialsError> {
    let secrets_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".openclaw")
        .join("secrets");

    let candidates = vec![
        secrets_dir.join("twitter.env"),
        secrets_dir.join("opensftp.env"),
    ];

    let content = candidates
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .ok_or_else(|| TwitterCredentialsError::NoCredentialsFile(candidates.clone()))?;

    let vars = parse_env_file(&content);

    let api_key = vars.get("TWITTER_API_KEY").cloned();
    let api_secret = vars.get("TWITTER_API_SECRET").cloned();
    let access_token = vars.get("TWITTER_ACCESS_TOKEN").cloned();
    let access_token_secret = vars.get("TWITTER_ACCESS_TOKEN_SECRET").cloned();

    let mut missing = Vec::new();
    if api_key.is_none() { missing.push("TWITTER_API_KEY"); }
    if api_secret.is_none() { missing.push("TWITTER_API_SECRET"); }
    if access_token.is_none() { missing.push("TWITTER_ACCESS_TOKEN"); }
    if access_token_secret.is_none() { missing.push("TWITTER_ACCESS_TOKEN_SECRET"); }

    if !missing.is_empty() {
        return Err(TwitterCredentialsError::MissingVars(missing));
    }

    Ok(TwitterCredentials {
        api_key: api_key.unwrap(),
        api_secret: api_secret.unwrap(),
        access_token: access_token.unwrap(),
        access_token_secret: access_token_secret.unwrap(),
    })
}

fn parse_env_file(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim_matches('"').trim_matches('\'');
            Some((key.trim().to_string(), value.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_file_basic() {
        let content = "TWITTER_API_KEY=key123\nTWITTER_API_SECRET=secret456\n";
        let vars = parse_env_file(content);
        assert_eq!(vars.get("TWITTER_API_KEY").map(String::as_str), Some("key123"));
        assert_eq!(vars.get("TWITTER_API_SECRET").map(String::as_str), Some("secret456"));
    }

    #[test]
    fn parse_env_file_quoted_values() {
        let content = r#"TWITTER_ACCESS_TOKEN="tok_abc"
TWITTER_ACCESS_TOKEN_SECRET='sec_xyz'
"#;
        let vars = parse_env_file(content);
        assert_eq!(vars.get("TWITTER_ACCESS_TOKEN").map(String::as_str), Some("tok_abc"));
        assert_eq!(vars.get("TWITTER_ACCESS_TOKEN_SECRET").map(String::as_str), Some("sec_xyz"));
    }

    #[test]
    fn parse_env_file_skips_comments() {
        let content = "# comment\nTWITTER_API_KEY=k\n";
        let vars = parse_env_file(content);
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn missing_vars_error() {
        // load() will fail in test env (no secrets dir), just check error formatting
        let err = TwitterCredentialsError::MissingVars(vec!["TWITTER_API_KEY", "TWITTER_API_SECRET"]);
        assert!(err.to_string().contains("TWITTER_API_KEY"));
    }
}
