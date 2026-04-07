//! Platform-specific desktop integration: install/uninstall app to system menus.

use std::path::PathBuf;

/// Embedded app icon (PNG, 256x256).
const APP_ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");

// ── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub async fn install() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let bin_dir = home.join(".local/bin");
    let apps_dir = home.join(".local/share/applications");
    let icon_dir = home.join(".local/share/icons/hicolor/256x256/apps");

    // Create directories
    for dir in [&bin_dir, &apps_dir, &icon_dir] {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    }

    // Copy binary
    let current_exe = std::env::current_exe().map_err(|e| format!("Cannot find current binary: {e}"))?;
    let dest_bin = bin_dir.join("opensftp");

    // Don't copy over ourselves
    if current_exe != dest_bin {
        tokio::fs::copy(&current_exe, &dest_bin)
            .await
            .map_err(|e| format!("Failed to copy binary: {e}"))?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            tokio::fs::set_permissions(&dest_bin, perms)
                .await
                .map_err(|e| format!("Failed to set permissions: {e}"))?;
        }
    }

    // Write icon
    let icon_path = icon_dir.join("opensftp.png");
    tokio::fs::write(&icon_path, APP_ICON_PNG)
        .await
        .map_err(|e| format!("Failed to write icon: {e}"))?;

    // Write .desktop file
    let desktop_content = format!(
        "[Desktop Entry]\n\
         Name=Open SFTP\n\
         Comment=Modern SFTP Client\n\
         Exec={}\n\
         Icon=opensftp\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Network;FileTransfer;\n\
         Keywords=sftp;ssh;ftp;file;transfer;\n",
        dest_bin.display()
    );
    let desktop_path = apps_dir.join("opensftp.desktop");
    tokio::fs::write(&desktop_path, desktop_content)
        .await
        .map_err(|e| format!("Failed to write .desktop file: {e}"))?;

    // Best-effort: update desktop database
    let _ = tokio::process::Command::new("update-desktop-database")
        .arg(apps_dir.to_string_lossy().as_ref())
        .output()
        .await;

    Ok("Added to Applications. You can find Open SFTP in your app launcher.".into())
}

#[cfg(target_os = "linux")]
pub async fn uninstall() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;

    let files = [
        home.join(".local/bin/opensftp"),
        home.join(".local/share/applications/opensftp.desktop"),
        home.join(".local/share/icons/hicolor/256x256/apps/opensftp.png"),
    ];

    for file in &files {
        if file.exists() {
            let _ = tokio::fs::remove_file(file).await;
        }
    }

    let apps_dir = home.join(".local/share/applications");
    let _ = tokio::process::Command::new("update-desktop-database")
        .arg(apps_dir.to_string_lossy().as_ref())
        .output()
        .await;

    Ok("Removed from Applications.".into())
}

#[cfg(target_os = "linux")]
pub fn is_installed() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".local/share/applications/opensftp.desktop").exists())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn installed_binary_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".local/bin/opensftp"))
}

// ── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub async fn install() -> Result<String, String> {
    let app_dir = PathBuf::from("/Applications/Open SFTP.app");
    let contents = app_dir.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");

    for dir in [&macos_dir, &resources] {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    }

    // Copy binary
    let current_exe = std::env::current_exe().map_err(|e| format!("Cannot find current binary: {e}"))?;
    let dest_bin = macos_dir.join("opensftp");
    tokio::fs::copy(&current_exe, &dest_bin)
        .await
        .map_err(|e| format!("Failed to copy binary: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        tokio::fs::set_permissions(&dest_bin, perms)
            .await
            .map_err(|e| format!("Failed to set permissions: {e}"))?;
    }

    // Write icon
    tokio::fs::write(resources.join("AppIcon.png"), APP_ICON_PNG)
        .await
        .map_err(|e| format!("Failed to write icon: {e}"))?;

    // Write Info.plist
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>opensftp</string>
    <key>CFBundleIdentifier</key>
    <string>de.renemurrell.opensftp</string>
    <key>CFBundleName</key>
    <string>Open SFTP</string>
    <key>CFBundleDisplayName</key>
    <string>Open SFTP</string>
    <key>CFBundleVersion</key>
    <string>0.2.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.2.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
</dict>
</plist>"#;

    tokio::fs::write(contents.join("Info.plist"), plist)
        .await
        .map_err(|e| format!("Failed to write Info.plist: {e}"))?;

    Ok("Added to Applications. You can find Open SFTP in your Dock or Launchpad.".into())
}

#[cfg(target_os = "macos")]
pub async fn uninstall() -> Result<String, String> {
    let app_dir = PathBuf::from("/Applications/Open SFTP.app");
    if app_dir.exists() {
        tokio::fs::remove_dir_all(&app_dir)
            .await
            .map_err(|e| format!("Failed to remove app bundle: {e}"))?;
    }
    Ok("Removed from Applications.".into())
}

#[cfg(target_os = "macos")]
pub fn is_installed() -> bool {
    PathBuf::from("/Applications/Open SFTP.app").exists()
}

#[cfg(target_os = "macos")]
pub fn installed_binary_path() -> Option<PathBuf> {
    Some(PathBuf::from("/Applications/Open SFTP.app/Contents/MacOS/opensftp"))
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub async fn install() -> Result<String, String> {
    let local_app = dirs::data_local_dir().ok_or("Cannot determine local app data directory")?;
    let install_dir = local_app.join("OpenSFTP");

    tokio::fs::create_dir_all(&install_dir)
        .await
        .map_err(|e| format!("Failed to create install dir: {e}"))?;

    // Copy binary
    let current_exe = std::env::current_exe().map_err(|e| format!("Cannot find current binary: {e}"))?;
    let dest_bin = install_dir.join("opensftp.exe");
    if current_exe != dest_bin {
        tokio::fs::copy(&current_exe, &dest_bin)
            .await
            .map_err(|e| format!("Failed to copy binary: {e}"))?;
    }

    // Write icon
    tokio::fs::write(install_dir.join("icon.png"), APP_ICON_PNG)
        .await
        .map_err(|e| format!("Failed to write icon: {e}"))?;

    // Create Start Menu shortcut via PowerShell
    let start_menu = dirs::data_dir()
        .ok_or("Cannot determine roaming app data")?
        .parent()
        .ok_or("Cannot determine start menu path")?
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");

    let shortcut_path = start_menu.join("Open SFTP.lnk");
    let ps_script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath = '{}'; \
         $sc.WorkingDirectory = '{}'; \
         $sc.Description = 'Modern SFTP Client'; \
         $sc.Save()",
        shortcut_path.display(),
        dest_bin.display(),
        install_dir.display(),
    );

    let output = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .await
        .map_err(|e| format!("Failed to create shortcut: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell shortcut creation failed: {stderr}"));
    }

    Ok("Added to Start Menu. You can find Open SFTP in your Start Menu.".into())
}

#[cfg(target_os = "windows")]
pub async fn uninstall() -> Result<String, String> {
    let local_app = dirs::data_local_dir().ok_or("Cannot determine local app data directory")?;
    let install_dir = local_app.join("OpenSFTP");

    if install_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&install_dir).await;
    }

    // Remove Start Menu shortcut
    if let Some(data_dir) = dirs::data_dir() {
        if let Some(parent) = data_dir.parent() {
            let shortcut = parent
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Open SFTP.lnk");
            if shortcut.exists() {
                let _ = tokio::fs::remove_file(&shortcut).await;
            }
        }
    }

    Ok("Removed from Start Menu.".into())
}

#[cfg(target_os = "windows")]
pub fn is_installed() -> bool {
    dirs::data_local_dir()
        .map(|d| d.join("OpenSFTP").join("opensftp.exe").exists())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub fn installed_binary_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("OpenSFTP").join("opensftp.exe"))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_embedded() {
        assert!(!APP_ICON_PNG.is_empty());
        // PNG magic bytes
        assert_eq!(&APP_ICON_PNG[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn is_installed_returns_bool() {
        // Just verify it doesn't panic
        let _ = is_installed();
    }
}
