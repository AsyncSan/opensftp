pub mod connection;
pub mod desktop_integration;
pub mod known_hosts;
pub mod sftp;
pub mod transfer;

/// Base directory for settings, profiles and the saved session.
///
/// Tests get a private temp directory per test thread, so `cargo test` never
/// overwrites the real settings or connection profiles of the person running
/// it, and parallel tests do not see each other's files.
pub fn config_dir() -> std::path::PathBuf {
    #[cfg(test)]
    {
        let thread: String = format!("{:?}", std::thread::current().id())
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        std::env::temp_dir().join(format!("opensftp-test-config-{}-{}", std::process::id(), thread))
    }
    #[cfg(not(test))]
    {
        dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
    }
}
