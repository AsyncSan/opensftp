use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, horizontal_rule, progress_bar, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};
use iced::futures::stream;
use tokio::sync::mpsc;

use crate::core::sftp::{RemoteEntry, SftpConnection, TransferProgress};
use crate::theme::AppTheme;
use crate::ui::connect_dialog::{self, ConnectMessage, ConnectState};
use crate::ui::file_panel::{self, FileEntry, PanelMessage, SortColumn};

/// Events yielded by the transfer stream for live progress.
#[derive(Debug, Clone)]
enum TransferEvent {
    Progress { transferred: u64, total_size: u64 },
    Complete(String),
    Failed(String),
}

#[derive(Clone)]
pub enum Message {
    Connect(ConnectMessage),
    LocalPanel(PanelMessage),
    RemotePanel(PanelMessage),
    Disconnect,
    CycleTheme,
    // Async results
    Connected(Arc<SftpConnection>, Vec<RemoteEntry>, String),
    ConnectionFailed(String),
    RemoteListing(Vec<RemoteEntry>, String),
    RemoteListingFailed(String),
    Disconnected,
    // Transfer
    DownloadSelected,
    UploadSelected,
    TransferProgress(TransferProgress),
    TransferComplete(String),
    TransferFailed(String),
    // Remote operations
    RemoteMkdir,
    MkdirNameChanged(String),
    MkdirConfirm,
    MkdirCancel,
    RemoteDelete,
    DeleteConfirm,
    DeleteCancel,
    RemoteRename,
    RenameNameChanged(String),
    RenameConfirm,
    RenameCancel,
    RemoteOpDone(String),
    RemoteOpFailed(String),
    // Local operations
    LocalMkdir,
    LocalMkdirNameChanged(String),
    LocalMkdirConfirm,
    LocalMkdirCancel,
    LocalDelete,
    LocalDeleteConfirm,
    LocalDeleteCancel,
    LocalRename,
    LocalRenameNameChanged(String),
    LocalRenameConfirm,
    LocalRenameCancel,
    // Save connection
    SaveConnection,
    SaveConnectionName(String),
    SaveConnectionConfirm,
    SaveConnectionCancel,
    // Toggle
    ToggleHidden,
    ToggleLog,
    // Keyboard
    KeyPressed(Key, Modifiers),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(m) => write!(f, "Connect({:?})", m),
            Self::LocalPanel(m) => write!(f, "LocalPanel({:?})", m),
            Self::RemotePanel(m) => write!(f, "RemotePanel({:?})", m),
            Self::Disconnect => write!(f, "Disconnect"),
            Self::CycleTheme => write!(f, "CycleTheme"),
            Self::Connected(_, _, h) => write!(f, "Connected(home={})", h),
            Self::ConnectionFailed(e) => write!(f, "ConnectionFailed({})", e),
            Self::RemoteListing(_, p) => write!(f, "RemoteListing({})", p),
            Self::RemoteListingFailed(e) => write!(f, "RemoteListingFailed({})", e),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::DownloadSelected => write!(f, "DownloadSelected"),
            Self::UploadSelected => write!(f, "UploadSelected"),
            Self::TransferProgress(p) => write!(f, "Progress({}/{})", p.transferred, p.total_size),
            Self::TransferComplete(s) => write!(f, "TransferComplete({})", s),
            Self::TransferFailed(e) => write!(f, "TransferFailed({})", e),
            Self::RemoteMkdir => write!(f, "RemoteMkdir"),
            Self::MkdirNameChanged(n) => write!(f, "MkdirNameChanged({})", n),
            Self::MkdirConfirm => write!(f, "MkdirConfirm"),
            Self::MkdirCancel => write!(f, "MkdirCancel"),
            Self::RemoteDelete => write!(f, "RemoteDelete"),
            Self::DeleteConfirm => write!(f, "DeleteConfirm"),
            Self::DeleteCancel => write!(f, "DeleteCancel"),
            Self::RemoteRename => write!(f, "RemoteRename"),
            Self::RenameNameChanged(n) => write!(f, "RenameNameChanged({})", n),
            Self::RenameConfirm => write!(f, "RenameConfirm"),
            Self::RenameCancel => write!(f, "RenameCancel"),
            Self::RemoteOpDone(s) => write!(f, "RemoteOpDone({})", s),
            Self::RemoteOpFailed(e) => write!(f, "RemoteOpFailed({})", e),
            Self::LocalMkdir => write!(f, "LocalMkdir"),
            Self::LocalMkdirNameChanged(n) => write!(f, "LocalMkdirNameChanged({})", n),
            Self::LocalMkdirConfirm => write!(f, "LocalMkdirConfirm"),
            Self::LocalMkdirCancel => write!(f, "LocalMkdirCancel"),
            Self::LocalDelete => write!(f, "LocalDelete"),
            Self::LocalDeleteConfirm => write!(f, "LocalDeleteConfirm"),
            Self::LocalDeleteCancel => write!(f, "LocalDeleteCancel"),
            Self::LocalRename => write!(f, "LocalRename"),
            Self::LocalRenameNameChanged(n) => write!(f, "LocalRenameNameChanged({})", n),
            Self::LocalRenameConfirm => write!(f, "LocalRenameConfirm"),
            Self::LocalRenameCancel => write!(f, "LocalRenameCancel"),
            Self::SaveConnection => write!(f, "SaveConnection"),
            Self::SaveConnectionName(n) => write!(f, "SaveConnectionName({})", n),
            Self::SaveConnectionConfirm => write!(f, "SaveConnectionConfirm"),
            Self::SaveConnectionCancel => write!(f, "SaveConnectionCancel"),
            Self::ToggleHidden => write!(f, "ToggleHidden"),
            Self::ToggleLog => write!(f, "ToggleLog"),
            Self::KeyPressed(k, m) => write!(f, "KeyPressed({:?}, {:?})", k, m),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Connect,
    Connecting,
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActivePanel {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Dialog {
    None,
    Mkdir(String),
    ConfirmDelete(String),
    Rename(String, String),      // (old_name, new_name)
    LocalMkdir(String),
    LocalConfirmDelete(String),
    LocalRename(String, String), // (old_name, new_name)
    SaveProfile(String),         // profile name
}

#[derive(Debug, Clone)]
struct TransferLogEntry {
    message: String,
    is_error: bool,
    timestamp: String,
}

pub struct App {
    screen: Screen,
    theme: AppTheme,
    connect_state: ConnectState,
    local_path: String,
    remote_path: String,
    local_entries: Vec<FileEntry>,
    remote_entries: Vec<FileEntry>,
    local_selected: BTreeSet<usize>,
    remote_selected: BTreeSet<usize>,
    last_anchor_local: Option<usize>,
    last_anchor_remote: Option<usize>,
    modifiers: Modifiers,
    status: String,
    sftp: Option<Arc<SftpConnection>>,
    // Transfer state
    transfer_active: bool,
    transfer_progress: f32,
    transfer_status: String,
    transfer_start: Option<Instant>,
    transfer_bytes: u64,
    transfer_total: u64,
    // UI state
    active_panel: ActivePanel,
    dialog: Dialog,
    show_hidden: bool,
    local_filter: String,
    remote_filter: String,
    sort_column: SortColumn,
    sort_ascending: bool,
    // Double-click tracking
    last_local_click: Option<(usize, Instant)>,
    last_remote_click: Option<(usize, Instant)>,
    // Transfer log
    transfer_log: Vec<TransferLogEntry>,
    show_log: bool,
}

impl Default for App {
    fn default() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());

        let local_entries = read_local_dir(&home, false);

        #[cfg(feature = "demo")]
        let (demo_screen, demo_remote_path, demo_remote_entries, demo_status) = {
            let mock_remote = vec![
                FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
                FileEntry { name: "backups".into(), is_dir: true, size: 4096, modified: "2026-03-28 14:30".into(), permissions: "rwxr-xr-x".into() },
                FileEntry { name: "config".into(), is_dir: true, size: 4096, modified: "2026-03-15 09:00".into(), permissions: "rwxr-x---".into() },
                FileEntry { name: "logs".into(), is_dir: true, size: 4096, modified: "2026-03-31 08:45".into(), permissions: "rwxrwxr-x".into() },
                FileEntry { name: "www".into(), is_dir: true, size: 4096, modified: "2026-03-30 22:10".into(), permissions: "rwxr-xr-x".into() },
                FileEntry { name: "deploy.sh".into(), is_dir: false, size: 2048, modified: "2026-03-29 16:20".into(), permissions: "rwxr-xr-x".into() },
                FileEntry { name: "docker-compose.yml".into(), is_dir: false, size: 1536, modified: "2026-03-25 11:00".into(), permissions: "rw-r--r--".into() },
                FileEntry { name: "nginx.conf".into(), is_dir: false, size: 4200, modified: "2026-03-20 13:45".into(), permissions: "rw-r--r--".into() },
                FileEntry { name: "README.md".into(), is_dir: false, size: 8700, modified: "2026-03-18 10:30".into(), permissions: "rw-r--r--".into() },
                FileEntry { name: "server.log".into(), is_dir: false, size: 524288, modified: "2026-03-31 18:22".into(), permissions: "rw-------".into() },
            ];
            (Screen::Browser, "/home/deploy".to_string(), mock_remote, "Connected to server.example.com".to_string())
        };
        #[cfg(not(feature = "demo"))]
        let (demo_screen, demo_remote_path, demo_remote_entries, demo_status) = {
            (Screen::Connect, String::new(), Vec::new(), "Not connected".to_string())
        };
        Self {
            screen: demo_screen,
            theme: AppTheme::default(),
            connect_state: ConnectState::new(),
            local_path: home,
            remote_path: demo_remote_path,
            local_entries,
            remote_entries: demo_remote_entries,
            local_selected: BTreeSet::new(),
            remote_selected: BTreeSet::new(),
            last_anchor_local: None,
            last_anchor_remote: None,
            modifiers: Modifiers::default(),
            status: demo_status,
            sftp: None,
            transfer_active: false,
            transfer_progress: 0.0,
            transfer_status: String::new(),
            transfer_start: None,
            transfer_bytes: 0,
            transfer_total: 0,
            active_panel: ActivePanel::Local,
            dialog: Dialog::None,
            show_hidden: false,
            local_filter: String::new(),
            remote_filter: String::new(),
            sort_column: SortColumn::Name,
            sort_ascending: true,
            last_local_click: None,
            last_remote_click: None,
            transfer_log: Vec::new(),
            show_log: false,
        }
    }
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Connect(msg) => match msg {
                ConnectMessage::HostChanged(v) => self.connect_state.host = v,
                ConnectMessage::PortChanged(v) => self.connect_state.port = v,
                ConnectMessage::UsernameChanged(v) => self.connect_state.username = v,
                ConnectMessage::PasswordChanged(v) => self.connect_state.password = v,
                ConnectMessage::AuthTypeSelected(t) => self.connect_state.auth_type = t,
                ConnectMessage::KeyPathChanged(v) => self.connect_state.key_path = v,
                ConnectMessage::BrowseKeyFile => {
                    return Task::perform(
                        async {
                            let handle = rfd::AsyncFileDialog::new()
                                .set_title("Select SSH Key File")
                                .set_directory(
                                    dirs::home_dir()
                                        .map(|h| h.join(".ssh"))
                                        .unwrap_or_default(),
                                )
                                .pick_file()
                                .await;
                            handle.map(|h| h.path().to_string_lossy().to_string())
                        },
                        |path| Message::Connect(ConnectMessage::KeyFileChosen(path)),
                    );
                }
                ConnectMessage::KeyFileChosen(Some(path)) => {
                    self.connect_state.key_path = path;
                }
                ConnectMessage::KeyFileChosen(None) => {}
                ConnectMessage::ProfileSelected(name) => {
                    self.connect_state.apply_profile(&name);
                }
                ConnectMessage::Connect => {
                    self.screen = Screen::Connecting;
                    self.status = format!(
                        "Connecting to {}:{}...",
                        self.connect_state.host, self.connect_state.port
                    );

                    let host = self.connect_state.host.clone();
                    let port: u16 = self.connect_state.port.parse().unwrap_or(22);
                    let username = self.connect_state.username.clone();
                    let auth = self.connect_state.auth_method();

                    return Task::perform(
                        async move {
                            let conn = SftpConnection::connect(&host, port, &username, &auth).await?;
                            let conn = Arc::new(conn);

                            let home = conn.sftp.canonicalize(".").await
                                .unwrap_or_else(|_| "/".to_string());

                            let entries = conn.list_dir(&home).await?;
                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((conn, entries, home))
                        },
                        |result| match result {
                            Ok((conn, entries, home)) => Message::Connected(conn, entries, home),
                            Err(e) => Message::ConnectionFailed(e.to_string()),
                        },
                    );
                }
                ConnectMessage::Cancel => {
                    self.connect_state = ConnectState::new();
                }
            },

            Message::Connected(conn, entries, home) => {
                self.status = format!("Connected to {}", self.connect_state.host);
                self.screen = Screen::Browser;
                self.sftp = Some(conn);
                self.remote_path = home;
                self.remote_entries = remote_to_file_entries(&entries, self.show_hidden);
            }

            Message::ConnectionFailed(err) => {
                self.screen = Screen::Connect;
                self.status = format!("Failed: {}", err);
            }

            Message::LocalPanel(msg) => {
                self.active_panel = ActivePanel::Local;
                match msg {
                    PanelMessage::EntryClicked(i, _, _) => {
                        let ctrl = self.modifiers.control();
                        let shift = self.modifiers.shift();

                        let is_double = self.last_local_click
                            .map(|(idx, t)| idx == i && t.elapsed().as_millis() < 400)
                            .unwrap_or(false);
                        self.last_local_click = Some((i, Instant::now()));

                        if let Some(entry) = self.local_entries.get(i) {
                            if entry.is_dir && (is_double || entry.name == "..") {
                                let new_path = if entry.name == ".." {
                                    std::path::Path::new(&self.local_path)
                                        .parent()
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "/".to_string())
                                } else {
                                    format!("{}/{}", self.local_path, entry.name)
                                };
                                self.local_entries = read_local_dir(&new_path, self.show_hidden);
                                self.local_path = new_path;
                                self.local_selected.clear();
                                self.last_anchor_local = None;
                                self.last_local_click = None;
                            } else if shift {
                                if let Some(anchor) = self.last_anchor_local {
                                    let (lo, hi) = if anchor <= i { (anchor, i) } else { (i, anchor) };
                                    self.local_selected.clear();
                                    for idx in lo..=hi {
                                        self.local_selected.insert(idx);
                                    }
                                } else {
                                    self.local_selected.clear();
                                    self.local_selected.insert(i);
                                    self.last_anchor_local = Some(i);
                                }
                            } else if ctrl {
                                if self.local_selected.contains(&i) {
                                    self.local_selected.remove(&i);
                                } else {
                                    self.local_selected.insert(i);
                                }
                                self.last_anchor_local = Some(i);
                            } else {
                                self.local_selected.clear();
                                self.local_selected.insert(i);
                                self.last_anchor_local = Some(i);
                            }
                        }
                    }
                    PanelMessage::GoUp => {
                        if let Some(parent) = std::path::Path::new(&self.local_path).parent() {
                            let new_path = parent.to_string_lossy().to_string();
                            self.local_entries = read_local_dir(&new_path, self.show_hidden);
                            self.local_path = new_path;
                            self.local_selected.clear();
                            self.local_filter.clear();
                        }
                    }
                    PanelMessage::Refresh => {
                        self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                    }
                    PanelMessage::PathSegmentClicked(path) => {
                        self.local_entries = read_local_dir(&path, self.show_hidden);
                        self.local_path = path;
                        self.local_selected.clear();
                        self.local_filter.clear();
                    }
                    PanelMessage::FilterChanged(f) => {
                        self.local_filter = f;
                        self.local_selected.clear();
                    }
                    PanelMessage::SortBy(col) => {
                        if self.sort_column == col {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_column = col;
                            self.sort_ascending = true;
                        }
                        sort_entries(&mut self.local_entries, self.sort_column, self.sort_ascending);
                        sort_entries(&mut self.remote_entries, self.sort_column, self.sort_ascending);
                        self.local_selected.clear();
                        self.remote_selected.clear();
                    }
                }
            }

            Message::RemotePanel(msg) => {
                self.active_panel = ActivePanel::Remote;
                let sftp = self.sftp.clone();
                match msg {
                    PanelMessage::EntryClicked(i, _, _) => {
                        let ctrl = self.modifiers.control();
                        let shift = self.modifiers.shift();

                        let is_double = self.last_remote_click
                            .map(|(idx, t)| idx == i && t.elapsed().as_millis() < 400)
                            .unwrap_or(false);
                        self.last_remote_click = Some((i, Instant::now()));

                        if let Some(entry) = self.remote_entries.get(i) {
                            if entry.is_dir && (is_double || entry.name == "..") {
                                let new_path = if entry.name == ".." {
                                    parent_path(&self.remote_path)
                                } else {
                                    format!("{}/{}", self.remote_path, entry.name)
                                };
                                self.remote_path = new_path.clone();
                                self.remote_selected.clear();
                                self.last_anchor_remote = None;
                                self.last_remote_click = None;

                                if let Some(conn) = sftp {
                                    return Task::perform(
                                        async move {
                                            let entries = conn.list_dir(&new_path).await?;
                                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, new_path))
                                        },
                                        |result| match result {
                                            Ok((entries, path)) => Message::RemoteListing(entries, path),
                                            Err(e) => Message::RemoteListingFailed(e.to_string()),
                                        },
                                    );
                                }
                            } else if shift {
                                if let Some(anchor) = self.last_anchor_remote {
                                    let (lo, hi) = if anchor <= i { (anchor, i) } else { (i, anchor) };
                                    self.remote_selected.clear();
                                    for idx in lo..=hi {
                                        self.remote_selected.insert(idx);
                                    }
                                } else {
                                    self.remote_selected.clear();
                                    self.remote_selected.insert(i);
                                    self.last_anchor_remote = Some(i);
                                }
                            } else if ctrl {
                                if self.remote_selected.contains(&i) {
                                    self.remote_selected.remove(&i);
                                } else {
                                    self.remote_selected.insert(i);
                                }
                                self.last_anchor_remote = Some(i);
                            } else {
                                self.remote_selected.clear();
                                self.remote_selected.insert(i);
                                self.last_anchor_remote = Some(i);
                            }
                        }
                    }
                    PanelMessage::GoUp => {
                        let new_path = parent_path(&self.remote_path);
                        self.remote_path = new_path.clone();

                        if let Some(conn) = sftp {
                            return Task::perform(
                                async move {
                                    let entries = conn.list_dir(&new_path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, new_path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteListingFailed(e.to_string()),
                                },
                            );
                        }
                    }
                    PanelMessage::Refresh => {
                        let path = self.remote_path.clone();
                        if let Some(conn) = sftp {
                            return Task::perform(
                                async move {
                                    let entries = conn.list_dir(&path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteListingFailed(e.to_string()),
                                },
                            );
                        }
                    }
                    PanelMessage::PathSegmentClicked(path) => {
                        self.remote_path = path.clone();
                        self.remote_selected.clear();
                        self.remote_filter.clear();
                        if let Some(conn) = sftp {
                            return Task::perform(
                                async move {
                                    let entries = conn.list_dir(&path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteListingFailed(e.to_string()),
                                },
                            );
                        }
                    }
                    PanelMessage::FilterChanged(f) => {
                        self.remote_filter = f;
                        self.remote_selected.clear();
                    }
                    PanelMessage::SortBy(col) => {
                        if self.sort_column == col {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_column = col;
                            self.sort_ascending = true;
                        }
                        sort_entries(&mut self.local_entries, self.sort_column, self.sort_ascending);
                        sort_entries(&mut self.remote_entries, self.sort_column, self.sort_ascending);
                        self.local_selected.clear();
                        self.remote_selected.clear();
                    }
                }
            }

            Message::RemoteListing(entries, path) => {
                self.remote_path = path;
                self.remote_entries = remote_to_file_entries(&entries, self.show_hidden);
                self.remote_selected.clear();
            }

            Message::RemoteListingFailed(err) => {
                self.status = format!("Error: {}", err);
            }

            // Download: remote selected -> local path (batch)
            Message::DownloadSelected => {
                if let Some(conn) = self.sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.remote_selected.iter()
                        .filter_map(|&idx| self.remote_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.remote_path, e.name),
                            format!("{}/{}", self.local_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.transfer_active = true;
                        self.transfer_progress = 0.0;
                        self.transfer_start = Some(Instant::now());
                        self.transfer_bytes = 0;
                        self.transfer_total = 0;
                        self.transfer_status = format!("Downloading {} item(s)...", count);

                        return spawn_transfer(move |progress_tx| async move {
                            let mut _total_bytes = 0u64;
                            for (remote_src, local_dst, is_dir) in &items {
                                if *is_dir {
                                    _total_bytes += conn.download_dir(remote_src, local_dst, progress_tx.clone()).await?;
                                } else {
                                    conn.download(remote_src, local_dst, progress_tx.clone()).await?;
                                }
                            }
                            Ok(format!("Downloaded {} item(s)", count))
                        });
                    }
                }
            }

            // Upload: local selected -> remote path (batch)
            Message::UploadSelected => {
                if let Some(conn) = self.sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.local_selected.iter()
                        .filter_map(|&idx| self.local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.local_path, e.name),
                            format!("{}/{}", self.remote_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.transfer_active = true;
                        self.transfer_progress = 0.0;
                        self.transfer_start = Some(Instant::now());
                        self.transfer_bytes = 0;
                        self.transfer_total = 0;
                        self.transfer_status = format!("Uploading {} item(s)...", count);

                        return spawn_transfer(move |progress_tx| async move {
                            for (local_src, remote_dst, is_dir) in &items {
                                if *is_dir {
                                    conn.upload_dir(local_src, remote_dst, progress_tx.clone()).await?;
                                } else {
                                    conn.upload(local_src, remote_dst, progress_tx.clone()).await?;
                                }
                            }
                            Ok(format!("Uploaded {} item(s)", count))
                        });
                    }
                }
            }

            Message::TransferProgress(p) => {
                if p.total_size > 0 {
                    self.transfer_progress = p.transferred as f32 / p.total_size as f32;
                    self.transfer_bytes = p.transferred;
                    self.transfer_total = p.total_size;
                }
            }

            Message::TransferComplete(msg) => {
                self.transfer_active = false;
                self.transfer_progress = 1.0;
                self.transfer_start = None;
                self.transfer_status = msg.clone();
                self.status = msg.clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.transfer_log.push(TransferLogEntry {
                    message: msg,
                    is_error: false,
                    timestamp: format_timestamp(now),
                });

                // Refresh both panels
                self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                if let Some(conn) = self.sftp.clone() {
                    let path = self.remote_path.clone();
                    return Task::perform(
                        async move {
                            let entries = conn.list_dir(&path).await?;
                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                        },
                        |result| match result {
                            Ok((entries, path)) => Message::RemoteListing(entries, path),
                            Err(e) => Message::RemoteListingFailed(e.to_string()),
                        },
                    );
                }
            }

            Message::TransferFailed(err) => {
                self.transfer_active = false;
                self.transfer_progress = 0.0;
                self.transfer_start = None;
                self.transfer_status = String::new();
                let msg = format!("Transfer failed: {}", err);
                self.status = msg.clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.transfer_log.push(TransferLogEntry {
                    message: msg,
                    is_error: true,
                    timestamp: format_timestamp(now),
                });
            }

            Message::Disconnect => {
                let sftp = self.sftp.take();
                self.screen = Screen::Connect;
                self.status = "Disconnecting...".to_string();
                self.remote_entries.clear();

                if let Some(conn) = sftp {
                    return Task::perform(
                        async move {
                            let _ = conn.close().await;
                        },
                        |_| Message::Disconnected,
                    );
                }
            }

            Message::Disconnected => {
                self.status = "Disconnected".to_string();
            }

            Message::CycleTheme => {
                let themes = AppTheme::all();
                let idx = themes.iter().position(|t| *t == self.theme).unwrap_or(0);
                self.theme = themes[(idx + 1) % themes.len()];
            }

            // Remote operations
            Message::RemoteMkdir => {
                if self.screen == Screen::Browser {
                    self.dialog = Dialog::Mkdir(String::new());
                }
            }
            Message::MkdirNameChanged(name) => {
                if let Dialog::Mkdir(ref mut n) = self.dialog {
                    *n = name;
                }
            }
            Message::MkdirConfirm => {
                if let Dialog::Mkdir(ref name) = self.dialog {
                    if !name.is_empty() {
                        if let Some(conn) = self.sftp.clone() {
                            let path = format!("{}/{}", self.remote_path, name);
                            let refresh_path = self.remote_path.clone();
                            self.dialog = Dialog::None;
                            return Task::perform(
                                async move {
                                    conn.mkdir(&path).await?;
                                    let entries = conn.list_dir(&refresh_path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, refresh_path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteOpFailed(e.to_string()),
                                },
                            );
                        }
                    }
                }
            }
            Message::MkdirCancel => {
                self.dialog = Dialog::None;
            }
            Message::RemoteDelete => {
                if self.screen == Screen::Browser {
                    let names: Vec<String> = self.remote_selected.iter()
                        .filter_map(|&idx| self.remote_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| e.name.clone())
                        .collect();
                    if !names.is_empty() {
                        let label = if names.len() == 1 {
                            names[0].clone()
                        } else {
                            format!("{} items", names.len())
                        };
                        self.dialog = Dialog::ConfirmDelete(label);
                    }
                }
            }
            Message::DeleteConfirm => {
                if let Dialog::ConfirmDelete(_) = self.dialog {
                    if let Some(conn) = self.sftp.clone() {
                        let items: Vec<(String, bool)> = self.remote_selected.iter()
                            .filter_map(|&idx| self.remote_entries.get(idx))
                            .filter(|e| e.name != "..")
                            .map(|e| (format!("{}/{}", self.remote_path, e.name), e.is_dir))
                            .collect();
                        let refresh_path = self.remote_path.clone();
                        self.dialog = Dialog::None;
                        self.remote_selected.clear();
                        return Task::perform(
                            async move {
                                for (path, is_dir) in &items {
                                    if *is_dir {
                                        conn.remove_dir_all(path).await?;
                                    } else {
                                        conn.remove_file(path).await?;
                                    }
                                }
                                let entries = conn.list_dir(&refresh_path).await?;
                                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, refresh_path))
                            },
                            |result| match result {
                                Ok((entries, path)) => Message::RemoteListing(entries, path),
                                Err(e) => Message::RemoteOpFailed(e.to_string()),
                            },
                        );
                    }
                }
            }
            Message::DeleteCancel => {
                self.dialog = Dialog::None;
            }
            Message::RemoteRename => {
                if self.screen == Screen::Browser {
                    if let Some(&idx) = self.remote_selected.iter().next() {
                        if let Some(entry) = self.remote_entries.get(idx) {
                            if entry.name != ".." {
                                self.dialog = Dialog::Rename(entry.name.clone(), entry.name.clone());
                            }
                        }
                    }
                }
            }
            Message::RenameNameChanged(name) => {
                if let Dialog::Rename(_, ref mut new) = self.dialog {
                    *new = name;
                }
            }
            Message::RenameConfirm => {
                if let Dialog::Rename(ref old, ref new) = self.dialog {
                    if !new.is_empty() && old != new {
                        if let Some(conn) = self.sftp.clone() {
                            let from = format!("{}/{}", self.remote_path, old);
                            let to = format!("{}/{}", self.remote_path, new);
                            let refresh_path = self.remote_path.clone();
                            self.dialog = Dialog::None;
                            return Task::perform(
                                async move {
                                    conn.rename(&from, &to).await?;
                                    let entries = conn.list_dir(&refresh_path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, refresh_path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteOpFailed(e.to_string()),
                                },
                            );
                        }
                    }
                }
            }
            Message::RenameCancel => {
                self.dialog = Dialog::None;
            }
            Message::RemoteOpDone(msg) => {
                self.status = msg;
            }
            Message::RemoteOpFailed(err) => {
                self.status = format!("Error: {}", err);
            }

            // Local file operations
            Message::LocalMkdir => {
                if self.screen == Screen::Browser {
                    self.dialog = Dialog::LocalMkdir(String::new());
                }
            }
            Message::LocalMkdirNameChanged(name) => {
                if let Dialog::LocalMkdir(ref mut n) = self.dialog {
                    *n = name;
                }
            }
            Message::LocalMkdirConfirm => {
                if let Dialog::LocalMkdir(ref name) = self.dialog {
                    if !name.is_empty() {
                        let path = format!("{}/{}", self.local_path, name);
                        match std::fs::create_dir_all(&path) {
                            Ok(_) => {
                                self.status = format!("Created directory: {}", name);
                                self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                            }
                            Err(e) => {
                                self.status = format!("Error: {}", e);
                            }
                        }
                    }
                    self.dialog = Dialog::None;
                }
            }
            Message::LocalMkdirCancel => {
                self.dialog = Dialog::None;
            }
            Message::LocalDelete => {
                if self.screen == Screen::Browser {
                    let names: Vec<String> = self.local_selected.iter()
                        .filter_map(|&idx| self.local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| e.name.clone())
                        .collect();
                    if !names.is_empty() {
                        let label = if names.len() == 1 {
                            names[0].clone()
                        } else {
                            format!("{} items", names.len())
                        };
                        self.dialog = Dialog::LocalConfirmDelete(label);
                    }
                }
            }
            Message::LocalDeleteConfirm => {
                if let Dialog::LocalConfirmDelete(_) = self.dialog {
                    let items: Vec<(String, bool)> = self.local_selected.iter()
                        .filter_map(|&idx| self.local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (format!("{}/{}", self.local_path, e.name), e.is_dir))
                        .collect();
                    let mut deleted = 0;
                    for (path, is_dir) in &items {
                        let result = if *is_dir {
                            std::fs::remove_dir_all(path)
                        } else {
                            std::fs::remove_file(path)
                        };
                        match result {
                            Ok(_) => deleted += 1,
                            Err(e) => {
                                self.status = format!("Error: {}", e);
                                break;
                            }
                        }
                    }
                    if deleted > 0 {
                        self.status = format!("Deleted {} item(s)", deleted);
                        self.local_selected.clear();
                        self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                    }
                    self.dialog = Dialog::None;
                }
            }
            Message::LocalDeleteCancel => {
                self.dialog = Dialog::None;
            }
            Message::LocalRename => {
                if self.screen == Screen::Browser {
                    if let Some(&idx) = self.local_selected.iter().next() {
                        if let Some(entry) = self.local_entries.get(idx) {
                            if entry.name != ".." {
                                self.dialog = Dialog::LocalRename(entry.name.clone(), entry.name.clone());
                            }
                        }
                    }
                }
            }
            Message::LocalRenameNameChanged(name) => {
                if let Dialog::LocalRename(_, ref mut new) = self.dialog {
                    *new = name;
                }
            }
            Message::LocalRenameConfirm => {
                if let Dialog::LocalRename(ref old, ref new) = self.dialog {
                    if !new.is_empty() && old != new {
                        let from = format!("{}/{}", self.local_path, old);
                        let to = format!("{}/{}", self.local_path, new);
                        match std::fs::rename(&from, &to) {
                            Ok(_) => {
                                self.status = format!("Renamed: {} -> {}", old, new);
                                self.local_selected.clear();
                                self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                            }
                            Err(e) => {
                                self.status = format!("Error: {}", e);
                            }
                        }
                    }
                    self.dialog = Dialog::None;
                }
            }
            Message::LocalRenameCancel => {
                self.dialog = Dialog::None;
            }

            // Save connection as profile
            Message::SaveConnection => {
                if self.screen == Screen::Browser {
                    let default_name = format!("{}@{}", self.connect_state.username, self.connect_state.host);
                    self.dialog = Dialog::SaveProfile(default_name);
                }
            }
            Message::SaveConnectionName(name) => {
                if let Dialog::SaveProfile(ref mut n) = self.dialog {
                    *n = name;
                }
            }
            Message::SaveConnectionConfirm => {
                if let Dialog::SaveProfile(ref name) = self.dialog {
                    if !name.is_empty() {
                        use crate::core::connection::ConnectionProfile;
                        let profile = ConnectionProfile {
                            name: name.clone(),
                            host: self.connect_state.host.clone(),
                            port: self.connect_state.port.parse().unwrap_or(22),
                            username: self.connect_state.username.clone(),
                            auth: self.connect_state.auth_method(),
                        };
                        let mut profiles = self.connect_state.profiles.clone();
                        // Replace existing or add new
                        if let Some(existing) = profiles.iter_mut().find(|p| p.name == *name) {
                            *existing = profile;
                        } else {
                            profiles.push(profile);
                        }
                        let _ = ConnectionProfile::save_all(&profiles);
                        self.connect_state.profiles = profiles;
                        self.status = format!("Saved connection \"{}\"", name);
                    }
                    self.dialog = Dialog::None;
                }
            }
            Message::SaveConnectionCancel => {
                self.dialog = Dialog::None;
            }

            Message::ToggleLog => {
                self.show_log = !self.show_log;
            }

            Message::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                // Re-fetch remote listing with new filter
                if let Some(conn) = self.sftp.clone() {
                    let path = self.remote_path.clone();
                    let show_hidden = self.show_hidden;
                    return Task::perform(
                        async move {
                            let entries = conn.list_dir(&path).await?;
                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                        },
                        move |result| match result {
                            Ok((entries, path)) => Message::RemoteListing(entries, path),
                            Err(e) => Message::RemoteListingFailed(e.to_string()),
                        },
                    );
                }
            }

            // Keyboard shortcuts
            Message::KeyPressed(key, mods) => {
                self.modifiers = mods;
                if self.dialog != Dialog::None {
                    if matches!(key, Key::Named(keyboard::key::Named::Escape)) {
                        self.dialog = Dialog::None;
                    }
                    return Task::none();
                }
                if self.screen != Screen::Browser {
                    return Task::none();
                }
                match key {
                    Key::Named(keyboard::key::Named::F5) => {
                        self.local_entries = read_local_dir(&self.local_path, self.show_hidden);
                        if let Some(conn) = self.sftp.clone() {
                            let path = self.remote_path.clone();
                            return Task::perform(
                                async move {
                                    let entries = conn.list_dir(&path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteListingFailed(e.to_string()),
                                },
                            );
                        }
                    }
                    Key::Named(keyboard::key::Named::Delete) => {
                        return match self.active_panel {
                            ActivePanel::Local => self.update(Message::LocalDelete),
                            ActivePanel::Remote => self.update(Message::RemoteDelete),
                        };
                    }
                    Key::Named(keyboard::key::Named::F2) => {
                        return match self.active_panel {
                            ActivePanel::Local => self.update(Message::LocalRename),
                            ActivePanel::Remote => self.update(Message::RemoteRename),
                        };
                    }
                    Key::Named(keyboard::key::Named::F7) => {
                        return match self.active_panel {
                            ActivePanel::Local => self.update(Message::LocalMkdir),
                            ActivePanel::Remote => self.update(Message::RemoteMkdir),
                        };
                    }
                    Key::Character(ref c) if c.as_str() == "h" && mods.control() => {
                        return self.update(Message::ToggleHidden);
                    }
                    Key::Named(keyboard::key::Named::ArrowDown) => {
                        match self.active_panel {
                            ActivePanel::Local => {
                                let max = self.local_entries.len().saturating_sub(1);
                                let current = self.local_selected.iter().next().copied().unwrap_or(0);
                                let next = (current + 1).min(max);
                                self.local_selected.clear();
                                self.local_selected.insert(next);
                                self.last_anchor_local = Some(next);
                            }
                            ActivePanel::Remote => {
                                let max = self.remote_entries.len().saturating_sub(1);
                                let current = self.remote_selected.iter().next().copied().unwrap_or(0);
                                let next = (current + 1).min(max);
                                self.remote_selected.clear();
                                self.remote_selected.insert(next);
                                self.last_anchor_remote = Some(next);
                            }
                        }
                    }
                    Key::Named(keyboard::key::Named::ArrowUp) => {
                        match self.active_panel {
                            ActivePanel::Local => {
                                let current = self.local_selected.iter().next().copied().unwrap_or(0);
                                let next = current.saturating_sub(1);
                                self.local_selected.clear();
                                self.local_selected.insert(next);
                                self.last_anchor_local = Some(next);
                            }
                            ActivePanel::Remote => {
                                let current = self.remote_selected.iter().next().copied().unwrap_or(0);
                                let next = current.saturating_sub(1);
                                self.remote_selected.clear();
                                self.remote_selected.insert(next);
                                self.last_anchor_remote = Some(next);
                            }
                        }
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        // Open directory or trigger double-click action
                        match self.active_panel {
                            ActivePanel::Local => {
                                if let Some(&idx) = self.local_selected.iter().next() {
                                    if let Some(entry) = self.local_entries.get(idx) {
                                        if entry.is_dir {
                                            let new_path = if entry.name == ".." {
                                                std::path::Path::new(&self.local_path)
                                                    .parent()
                                                    .map(|p| p.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| "/".to_string())
                                            } else {
                                                format!("{}/{}", self.local_path, entry.name)
                                            };
                                            self.local_entries = read_local_dir(&new_path, self.show_hidden);
                                            self.local_path = new_path;
                                            self.local_selected.clear();
                                            self.last_anchor_local = None;
                                        }
                                    }
                                }
                            }
                            ActivePanel::Remote => {
                                if let Some(&idx) = self.remote_selected.iter().next() {
                                    if let Some(entry) = self.remote_entries.get(idx) {
                                        if entry.is_dir {
                                            let new_path = if entry.name == ".." {
                                                parent_path(&self.remote_path)
                                            } else {
                                                format!("{}/{}", self.remote_path, entry.name)
                                            };
                                            self.remote_path = new_path.clone();
                                            self.remote_selected.clear();
                                            self.last_anchor_remote = None;
                                            if let Some(conn) = self.sftp.clone() {
                                                return Task::perform(
                                                    async move {
                                                        let entries = conn.list_dir(&new_path).await?;
                                                        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, new_path))
                                                    },
                                                    |result| match result {
                                                        Ok((entries, path)) => Message::RemoteListing(entries, path),
                                                        Err(e) => Message::RemoteListingFailed(e.to_string()),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Key::Named(keyboard::key::Named::Tab) => {
                        self.active_panel = match self.active_panel {
                            ActivePanel::Local => ActivePanel::Remote,
                            ActivePanel::Remote => ActivePanel::Local,
                        };
                    }
                    _ => {}
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Connect => {
                let form = connect_dialog::view_connect_dialog(&self.connect_state)
                    .map(Message::Connect);

                if self.status.starts_with("Failed") {
                    container(
                        column![
                            text(&self.status).size(13).color(iced::Color::from_rgb(0.9, 0.2, 0.2)),
                            form,
                        ]
                        .spacing(8)
                        .padding(8),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    form
                }
            }
            Screen::Connecting => {
                container(
                    column![
                        text(&self.status).size(16),
                    ]
                    .spacing(12),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
            }
            Screen::Browser => {
                let toolbar = container(
                    row![
                        text(&self.status).size(13),
                        iced::widget::horizontal_space(),
                        button(text(if self.show_hidden { "Hide .*" } else { "Show .*" }).size(12))
                            .on_press(Message::ToggleHidden)
                            .padding([4, 8]),
                        button(text("Save").size(12))
                            .on_press(Message::SaveConnection)
                            .padding([4, 8]),
                        button(text(self.theme.name()).size(12))
                            .on_press(Message::CycleTheme)
                            .padding([4, 8]),
                        button(text(format!("Log ({})", self.transfer_log.len())).size(12))
                            .on_press(Message::ToggleLog)
                            .padding([4, 8]),
                        button(text("Disconnect").size(12))
                            .on_press(Message::Disconnect)
                            .padding([4, 8]),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding(6)
                .width(Length::Fill)
                .style(|theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color {
                        a: 0.3,
                        ..theme.extended_palette().background.strong.color
                    })),
                    ..Default::default()
                });

                // Transfer buttons between panels (files and directories)
                let can_upload = !self.local_selected.is_empty()
                    && self.local_selected.iter()
                        .any(|&idx| self.local_entries.get(idx).map_or(false, |e| e.name != ".."))
                    && !self.transfer_active;
                let can_download = !self.remote_selected.is_empty()
                    && self.remote_selected.iter()
                        .any(|&idx| self.remote_entries.get(idx).map_or(false, |e| e.name != ".."))
                    && !self.transfer_active;

                let mut upload_btn = button(
                    text("Upload \u{25B6}").size(12)
                ).padding([6, 14]);
                if can_upload {
                    upload_btn = upload_btn.on_press(Message::UploadSelected);
                }

                let mut download_btn = button(
                    text("\u{25C0} Download").size(12)
                ).padding([6, 14]);
                if can_download {
                    download_btn = download_btn.on_press(Message::DownloadSelected);
                }

                let transfer_controls = container(
                    column![
                        upload_btn,
                        download_btn,
                    ]
                    .spacing(12)
                    .align_x(iced::Alignment::Center),
                )
                .center_y(Length::Fill)
                .width(Length::Shrink)
                .padding([8, 12]);

                let panels = row![
                    file_panel::view_panel(
                        "Local",
                        &self.local_path,
                        &self.local_entries,
                        &self.local_selected,
                        &self.local_filter,
                        self.active_panel == ActivePanel::Local,
                        self.sort_column,
                        self.sort_ascending,
                    )
                    .map(Message::LocalPanel),
                    transfer_controls,
                    file_panel::view_panel(
                        "Remote",
                        &self.remote_path,
                        &self.remote_entries,
                        &self.remote_selected,
                        &self.remote_filter,
                        self.active_panel == ActivePanel::Remote,
                        self.sort_column,
                        self.sort_ascending,
                    )
                    .map(Message::RemotePanel),
                ]
                .spacing(4);

                // Transfer progress bar
                let transfer_bar: Element<'_, Message> = if self.transfer_active {
                    let speed_text = self.transfer_start
                        .map(|start| {
                            let elapsed = start.elapsed().as_secs_f64();
                            if elapsed > 0.1 {
                                let speed = self.transfer_bytes as f64 / elapsed;
                                format!(
                                    "{} / {} ({}/s)",
                                    format_size(self.transfer_bytes),
                                    format_size(self.transfer_total),
                                    format_size(speed as u64),
                                )
                            } else {
                                self.transfer_status.clone()
                            }
                        })
                        .unwrap_or_else(|| self.transfer_status.clone());
                    container(
                        column![
                            text(speed_text).size(12),
                            progress_bar(0.0..=1.0, self.transfer_progress)
                                .height(12),
                        ]
                        .spacing(4)
                        .padding([6, 12]),
                    )
                    .width(Length::Fill)
                    .into()
                } else {
                    container(text("")).width(Length::Fill).into()
                };

                // Transfer log panel
                let log_entries = &self.transfer_log;
                let show_log = self.show_log;
                let make_log_panel = || -> Element<'_, Message> {
                    if show_log && !log_entries.is_empty() {
                        let log_rows: Vec<Element<'_, Message>> = log_entries.iter().rev().take(50).map(|entry| {
                            let color = if entry.is_error {
                                iced::Color::from_rgb(0.9, 0.3, 0.3)
                            } else {
                                iced::Color::from_rgb(0.5, 0.9, 0.5)
                            };
                            text(format!("[{}] {}", entry.timestamp, entry.message))
                                .size(11)
                                .color(color)
                                .into()
                        }).collect();
                        container(
                            scrollable(column(log_rows).spacing(2).padding(6))
                                .height(Length::Fixed(120.0)),
                        )
                        .width(Length::Fill)
                        .padding([4, 8])
                        .style(|theme: &Theme| container::Style {
                            background: Some(iced::Background::Color(iced::Color {
                                a: 0.15,
                                ..theme.extended_palette().background.strong.color
                            })),
                            border: iced::Border {
                                color: theme.extended_palette().background.strong.color,
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        })
                        .into()
                    } else {
                        container(text("")).height(Length::Shrink).width(Length::Fill).into()
                    }
                };

                // Status bar
                let local_count = self.local_entries.len().saturating_sub(1);
                let remote_count = self.remote_entries.len().saturating_sub(1);
                let local_sel = self.local_selected.len();
                let remote_sel = self.remote_selected.len();
                let local_sel_size: u64 = self.local_selected.iter()
                    .filter_map(|&idx| self.local_entries.get(idx))
                    .filter(|e| !e.is_dir && e.name != "..")
                    .map(|e| e.size)
                    .sum();
                let remote_sel_size: u64 = self.remote_selected.iter()
                    .filter_map(|&idx| self.remote_entries.get(idx))
                    .filter(|e| !e.is_dir && e.name != "..")
                    .map(|e| e.size)
                    .sum();
                let sel_info = |count: usize, size: u64| -> String {
                    if count == 0 { return String::new(); }
                    if size > 0 {
                        format!(" ({} sel, {})", count, format_size(size))
                    } else {
                        format!(" ({} selected)", count)
                    }
                };
                let status_text = format!(
                    "Local: {} items{} | Remote: {} items{}",
                    local_count,
                    sel_info(local_sel, local_sel_size),
                    remote_count,
                    sel_info(remote_sel, remote_sel_size),
                );
                let status_bar = container(
                    text(status_text).size(12),
                )
                .padding([4, 12])
                .width(Length::Fill)
                .style(|theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color {
                        a: 0.2,
                        ..theme.extended_palette().background.strong.color
                    })),
                    ..Default::default()
                });

                // Dialog overlay
                let active = &self.active_panel;
                let make_fkey_bar = || -> Element<'_, Message> {
                    container(
                        row![
                            button(text("F2 Rename").size(12))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalRename,
                                    ActivePanel::Remote => Message::RemoteRename,
                                })
                                .padding([4, 12])
                                .width(Length::FillPortion(1)),
                            button(text("F5 Refresh").size(12))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalPanel(PanelMessage::Refresh),
                                    ActivePanel::Remote => Message::RemotePanel(PanelMessage::Refresh),
                                })
                                .padding([4, 12])
                                .width(Length::FillPortion(1)),
                            button(text("F7 Mkdir").size(12))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalMkdir,
                                    ActivePanel::Remote => Message::RemoteMkdir,
                                })
                                .padding([4, 12])
                                .width(Length::FillPortion(1)),
                            button(text("F8 Delete").size(12))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalDelete,
                                    ActivePanel::Remote => Message::RemoteDelete,
                                })
                                .padding([4, 12])
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(4),
                    )
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(|theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            a: 0.3,
                            ..theme.extended_palette().background.strong.color
                        })),
                        ..Default::default()
                    })
                    .into()
                };

                let base_layout = || -> Element<'_, Message> {
                    column![toolbar, horizontal_rule(1), panels, transfer_bar, make_log_panel(), horizontal_rule(1), status_bar, make_fkey_bar()]
                        .spacing(0)
                        .into()
                };

                fn wrap_dialog(inner: Element<'_, Message>) -> Element<'_, Message> {
                    let card = container(inner)
                        .padding(20)
                        .style(|theme: &Theme| container::Style {
                            background: Some(iced::Background::Color(
                                theme.extended_palette().background.base.color,
                            )),
                            border: iced::Border {
                                color: theme.extended_palette().primary.strong.color,
                                width: 2.0,
                                radius: 8.0.into(),
                            },
                            ..Default::default()
                        });
                    container(card)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(iced::Background::Color(iced::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 })),
                            ..Default::default()
                        })
                        .into()
                }

                let content: Element<'_, Message> = match &self.dialog {
                    Dialog::None => base_layout(),
                    Dialog::Mkdir(name) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("New Directory").size(16),
                                text_input("Directory name", name)
                                    .on_input(Message::MkdirNameChanged)
                                    .on_submit(Message::MkdirConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::MkdirCancel).padding(6),
                                    button("Create").on_press(Message::MkdirConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::ConfirmDelete(name) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("Delete").size(16),
                                text(format!("Delete \"{}\"?", name)).size(14),
                                row![
                                    button("Cancel").on_press(Message::DeleteCancel).padding(6),
                                    button("Delete").on_press(Message::DeleteConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::Rename(old, new) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text(format!("Rename \"{}\"", old)).size(16),
                                text_input("New name", new)
                                    .on_input(Message::RenameNameChanged)
                                    .on_submit(Message::RenameConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::RenameCancel).padding(6),
                                    button("Rename").on_press(Message::RenameConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::LocalMkdir(name) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("New Local Directory").size(16),
                                text_input("Directory name", name)
                                    .on_input(Message::LocalMkdirNameChanged)
                                    .on_submit(Message::LocalMkdirConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::LocalMkdirCancel).padding(6),
                                    button("Create").on_press(Message::LocalMkdirConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::LocalConfirmDelete(name) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("Delete Local").size(16),
                                text(format!("Delete \"{}\"?", name)).size(14),
                                row![
                                    button("Cancel").on_press(Message::LocalDeleteCancel).padding(6),
                                    button("Delete").on_press(Message::LocalDeleteConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::LocalRename(old, new) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text(format!("Rename Local \"{}\"", old)).size(16),
                                text_input("New name", new)
                                    .on_input(Message::LocalRenameNameChanged)
                                    .on_submit(Message::LocalRenameConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::LocalRenameCancel).padding(6),
                                    button("Rename").on_press(Message::LocalRenameConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::SaveProfile(name) => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("Save Connection").size(16),
                                text_input("Profile name", name)
                                    .on_input(Message::SaveConnectionName)
                                    .on_submit(Message::SaveConnectionConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::SaveConnectionCancel).padding(6),
                                    button("Save").on_press(Message::SaveConnectionConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                };

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        }
    }

    pub fn theme(&self) -> Theme {
        self.theme.to_iced_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyPressed(key, modifiers))
        })
    }
}

/// Spawn a transfer task and return a streaming Task for live progress.
fn spawn_transfer<F, Fut>(transfer_fn: F) -> Task<Message>
where
    F: FnOnce(mpsc::UnboundedSender<TransferProgress>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send,
{
    let (event_tx, event_rx) = mpsc::unbounded_channel::<TransferEvent>();

    tokio::spawn(async move {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<TransferProgress>();

        let fwd_tx = event_tx.clone();
        let fwd = tokio::spawn(async move {
            while let Some(p) = progress_rx.recv().await {
                let _ = fwd_tx.send(TransferEvent::Progress {
                    transferred: p.transferred,
                    total_size: p.total_size,
                });
            }
        });

        let result = transfer_fn(progress_tx).await;
        let _ = fwd.await;

        match result {
            Ok(msg) => { let _ = event_tx.send(TransferEvent::Complete(msg)); }
            Err(e) => { let _ = event_tx.send(TransferEvent::Failed(e.to_string())); }
        }
    });

    let event_stream = stream::unfold(event_rx, |mut rx| async {
        rx.recv().await.map(|ev| (ev, rx))
    });

    Task::run(event_stream, |ev| match ev {
        TransferEvent::Progress { transferred, total_size } => {
            Message::TransferProgress(TransferProgress { transferred, total_size, done: false })
        }
        TransferEvent::Complete(msg) => Message::TransferComplete(msg),
        TransferEvent::Failed(msg) => Message::TransferFailed(msg),
    })
}

fn format_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in &units {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

fn format_permissions(mode: u32) -> String {
    if mode == 0 {
        return String::new();
    }
    let mut s = String::with_capacity(9);
    let flags = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    for (bit, ch) in &flags {
        s.push(if mode & bit != 0 { *ch } else { '-' });
    }
    s
}

fn remote_to_file_entries(entries: &[RemoteEntry], show_hidden: bool) -> Vec<FileEntry> {
    let mut result = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
        permissions: String::new(),
    }];

    result.extend(entries.iter()
        .filter(|e| show_hidden || !e.name.starts_with('.'))
        .map(|e| FileEntry {
            name: e.name.clone(),
            is_dir: e.is_dir,
            size: e.size,
            modified: format_timestamp(e.mtime),
            permissions: format_permissions(e.permissions),
        }));

    result
}

fn parent_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => path[..i].to_string(),
        None => "/".to_string(),
    }
}

fn sort_entries(entries: &mut [FileEntry], column: SortColumn, ascending: bool) {
    // Skip the ".." entry at index 0
    if entries.len() <= 1 {
        return;
    }
    let rest = &mut entries[1..];
    rest.sort_by(|a, b| {
        // Dirs always first
        let dir_cmp = b.is_dir.cmp(&a.is_dir);
        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }
        let cmp = match column {
            SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortColumn::Size => a.size.cmp(&b.size),
            SortColumn::Modified => a.modified.cmp(&b.modified),
            SortColumn::Permissions => a.permissions.cmp(&b.permissions),
        };
        if ascending { cmp } else { cmp.reverse() }
    });
}

fn read_local_dir(path: &str, show_hidden: bool) -> Vec<FileEntry> {
    let mut entries = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
        permissions: String::new(),
    }];

    if let Ok(read_dir) = std::fs::read_dir(path) {
        let mut items: Vec<FileEntry> = read_dir
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                let modified = meta
                    .modified()
                    .ok()
                    .map(|t| {
                        let elapsed = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        format_timestamp(elapsed)
                    })
                    .unwrap_or_default();

                #[cfg(unix)]
                let permissions = {
                    use std::os::unix::fs::PermissionsExt;
                    format_permissions(meta.permissions().mode())
                };
                #[cfg(not(unix))]
                let permissions = String::new();

                Some(FileEntry {
                    name: e.file_name().to_string_lossy().to_string(),
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                    modified,
                    permissions,
                })
            })
            .collect();

        items.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
        });

        if !show_hidden {
            items.retain(|e| !e.name.starts_with('.'));
        }

        entries.extend(items);
    }

    entries
}

fn format_timestamp(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    // Civil time from unix timestamp (no external crate needed)
    let mut days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, m, d, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "");
    }

    #[test]
    fn format_timestamp_epoch() {
        assert_eq!(format_timestamp(1), "1970-01-01 00:00");
    }

    #[test]
    fn format_timestamp_known_date() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(format_timestamp(1704067200), "2024-01-01 00:00");
    }

    #[test]
    fn format_timestamp_with_time() {
        // 2024-06-15 13:30:00 UTC = 1718458200
        assert_eq!(format_timestamp(1718458200), "2024-06-15 13:30");
    }

    #[test]
    fn format_timestamp_leap_year() {
        // 2024-02-29 12:00:00 UTC = 1709208000
        assert_eq!(format_timestamp(1709208000), "2024-02-29 12:00");
    }

    #[test]
    fn format_permissions_empty() {
        assert_eq!(format_permissions(0), "");
    }

    #[test]
    fn format_permissions_755() {
        assert_eq!(format_permissions(0o755), "rwxr-xr-x");
    }

    #[test]
    fn format_permissions_644() {
        assert_eq!(format_permissions(0o644), "rw-r--r--");
    }

    #[test]
    fn format_permissions_700() {
        assert_eq!(format_permissions(0o700), "rwx------");
    }

    #[test]
    fn parent_path_root() {
        assert_eq!(parent_path("/"), "/");
    }

    #[test]
    fn parent_path_one_level() {
        assert_eq!(parent_path("/home"), "/");
    }

    #[test]
    fn parent_path_nested() {
        assert_eq!(parent_path("/home/user/docs"), "/home/user");
    }

    #[test]
    fn sort_entries_by_name() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "zebra".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "alpha".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Name, true);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[1].name, "alpha");
        assert_eq!(entries[2].name, "zebra");
    }

    #[test]
    fn sort_entries_dirs_first() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "file.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
            FileEntry { name: "adir".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Name, true);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[1].name, "adir");
        assert_eq!(entries[2].name, "file.txt");
    }

    #[test]
    fn sort_entries_by_size() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "big".into(), is_dir: false, size: 1000, modified: String::new(), permissions: String::new() },
            FileEntry { name: "small".into(), is_dir: false, size: 10, modified: String::new(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Size, true);
        assert_eq!(entries[1].name, "small");
        assert_eq!(entries[2].name, "big");
    }

    // ===== SMOKE TESTS =====

    // --- App default state ---
    #[test]
    fn smoke_app_default_starts_on_connect_screen() {
        let app = App::default();
        assert_eq!(app.screen, Screen::Connect);
        assert!(app.sftp.is_none());
        assert!(!app.transfer_active);
        assert_eq!(app.dialog, Dialog::None);
        assert_eq!(app.active_panel, ActivePanel::Local);
    }

    #[test]
    fn smoke_app_default_local_path_exists() {
        let app = App::default();
        assert!(!app.local_path.is_empty());
        assert!(std::path::Path::new(&app.local_path).exists());
    }

    #[test]
    fn smoke_app_default_local_entries_populated() {
        let app = App::default();
        // Should at least have ".." entry
        assert!(!app.local_entries.is_empty());
        assert_eq!(app.local_entries[0].name, "..");
        assert!(app.local_entries[0].is_dir);
    }

    #[test]
    fn smoke_app_default_remote_empty() {
        let app = App::default();
        assert!(app.remote_entries.is_empty());
        assert!(app.remote_path.is_empty());
        assert!(app.remote_selected.is_empty());
    }

    #[test]
    fn smoke_app_default_theme_is_dark() {
        let app = App::default();
        assert_eq!(app.theme, AppTheme::Dark);
    }

    #[test]
    fn smoke_app_default_filters_empty() {
        let app = App::default();
        assert!(app.local_filter.is_empty());
        assert!(app.remote_filter.is_empty());
    }

    #[test]
    fn smoke_app_default_hidden_off() {
        let app = App::default();
        assert!(!app.show_hidden);
        assert!(!app.show_log);
    }

    #[test]
    fn smoke_app_default_sort_name_asc() {
        let app = App::default();
        assert_eq!(app.sort_column, SortColumn::Name);
        assert!(app.sort_ascending);
    }

    // --- State transitions (no SFTP needed) ---
    #[test]
    fn smoke_cycle_theme_wraps() {
        let mut app = App::default();
        let initial = app.theme;
        let theme_count = AppTheme::all().len();
        for _ in 0..theme_count {
            app.update(Message::CycleTheme);
        }
        assert_eq!(app.theme, initial);
    }

    #[test]
    fn smoke_cycle_theme_advances() {
        let mut app = App::default();
        assert_eq!(app.theme, AppTheme::Dark);
        app.update(Message::CycleTheme);
        assert_eq!(app.theme, AppTheme::Light);
        app.update(Message::CycleTheme);
        assert_eq!(app.theme, AppTheme::Nord);
    }

    #[test]
    fn smoke_toggle_hidden() {
        let mut app = App::default();
        assert!(!app.show_hidden);
        let entries_before = app.local_entries.len();
        app.show_hidden = true;
        app.local_entries = read_local_dir(&app.local_path, true);
        // With hidden shown, should have >= entries as before
        assert!(app.local_entries.len() >= entries_before);
    }

    #[test]
    fn smoke_toggle_log() {
        let mut app = App::default();
        assert!(!app.show_log);
        app.update(Message::ToggleLog);
        assert!(app.show_log);
        app.update(Message::ToggleLog);
        assert!(!app.show_log);
    }

    #[test]
    fn smoke_connection_failed_returns_to_connect() {
        let mut app = App::default();
        app.screen = Screen::Connecting;
        app.update(Message::ConnectionFailed("timeout".into()));
        assert_eq!(app.screen, Screen::Connect);
        assert!(app.status.contains("timeout"));
    }

    #[test]
    fn smoke_disconnect_clears_remote() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![FileEntry {
            name: "test.txt".into(), is_dir: false, size: 100,
            modified: String::new(), permissions: String::new(),
        }];
        app.update(Message::Disconnect);
        assert_eq!(app.screen, Screen::Connect);
        assert!(app.remote_entries.is_empty());
    }

    #[test]
    fn smoke_disconnected_status() {
        let mut app = App::default();
        app.update(Message::Disconnected);
        assert_eq!(app.status, "Disconnected");
    }

    // --- Dialog state machine ---
    #[test]
    fn smoke_remote_mkdir_dialog_not_on_connect() {
        let mut app = App::default();
        app.screen = Screen::Connect;
        app.update(Message::RemoteMkdir);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_mkdir_dialog_on_browser() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.update(Message::RemoteMkdir);
        assert!(matches!(app.dialog, Dialog::Mkdir(_)));
    }

    #[test]
    fn smoke_mkdir_name_changes() {
        let mut app = App::default();
        app.dialog = Dialog::Mkdir(String::new());
        app.update(Message::MkdirNameChanged("new_dir".into()));
        assert!(matches!(app.dialog, Dialog::Mkdir(ref n) if n == "new_dir"));
    }

    #[test]
    fn smoke_mkdir_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::Mkdir("something".into());
        app.update(Message::MkdirCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_delete_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::ConfirmDelete("file.txt".into());
        app.update(Message::DeleteCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_rename_name_changes() {
        let mut app = App::default();
        app.dialog = Dialog::Rename("old.txt".into(), "old.txt".into());
        app.update(Message::RenameNameChanged("new.txt".into()));
        assert!(matches!(app.dialog, Dialog::Rename(_, ref n) if n == "new.txt"));
    }

    #[test]
    fn smoke_rename_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::Rename("old.txt".into(), "new.txt".into());
        app.update(Message::RenameCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    // --- Local dialogs ---
    #[test]
    fn smoke_local_mkdir_dialog() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.update(Message::LocalMkdir);
        assert!(matches!(app.dialog, Dialog::LocalMkdir(_)));
    }

    #[test]
    fn smoke_local_mkdir_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::LocalMkdir("test".into());
        app.update(Message::LocalMkdirCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_local_delete_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::LocalConfirmDelete("file.txt".into());
        app.update(Message::LocalDeleteCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_local_rename_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::LocalRename("old.txt".into(), "new.txt".into());
        app.update(Message::LocalRenameCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_save_connection_dialog() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.connect_state.username = "user".into();
        app.connect_state.host = "host.com".into();
        app.update(Message::SaveConnection);
        assert!(matches!(app.dialog, Dialog::SaveProfile(ref n) if n == "user@host.com"));
    }

    #[test]
    fn smoke_save_connection_cancel() {
        let mut app = App::default();
        app.dialog = Dialog::SaveProfile("test".into());
        app.update(Message::SaveConnectionCancel);
        assert_eq!(app.dialog, Dialog::None);
    }

    // --- Selection and navigation ---
    #[test]
    fn smoke_local_click_selects() {
        let mut app = App::default();
        // Entry 1 should exist (first file/dir after "..")
        if app.local_entries.len() > 1 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            assert!(app.local_selected.contains(&1));
            assert_eq!(app.active_panel, ActivePanel::Local);
        }
    }

    #[test]
    fn smoke_local_click_replaces_selection() {
        let mut app = App::default();
        if app.local_entries.len() > 2 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(2, false, false)));
            assert!(!app.local_selected.contains(&1));
            assert!(app.local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_local_ctrl_click_adds() {
        let mut app = App::default();
        if app.local_entries.len() > 2 {
            app.modifiers = Modifiers::CTRL;
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(2, false, false)));
            assert!(app.local_selected.contains(&1));
            assert!(app.local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_local_shift_click_range() {
        let mut app = App::default();
        if app.local_entries.len() > 3 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.modifiers = Modifiers::SHIFT;
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(3, false, false)));
            assert!(app.local_selected.contains(&1));
            assert!(app.local_selected.contains(&2));
            assert!(app.local_selected.contains(&3));
        }
    }

    #[test]
    fn smoke_local_go_up() {
        let mut app = App::default();
        let original = app.local_path.clone();
        if original != "/" {
            app.update(Message::LocalPanel(PanelMessage::GoUp));
            assert_ne!(app.local_path, original);
            assert!(app.local_selected.is_empty());
        }
    }

    #[test]
    fn smoke_local_filter() {
        let mut app = App::default();
        app.update(Message::LocalPanel(PanelMessage::FilterChanged("test".into())));
        assert_eq!(app.local_filter, "test");
        assert!(app.local_selected.is_empty());
    }

    #[test]
    fn smoke_local_sort_toggles() {
        let mut app = App::default();
        assert_eq!(app.sort_column, SortColumn::Name);
        assert!(app.sort_ascending);
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Name)));
        assert!(!app.sort_ascending);
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert_eq!(app.sort_column, SortColumn::Size);
        assert!(app.sort_ascending);
    }

    #[test]
    fn smoke_local_path_segment_click() {
        let mut app = App::default();
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/tmp".into())));
        assert_eq!(app.local_path, "/tmp");
        assert!(app.local_selected.is_empty());
    }

    // --- Remote panel (without connection) ---
    #[test]
    fn smoke_remote_delete_no_selection() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.update(Message::RemoteDelete);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_delete_skips_dotdot() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![FileEntry {
            name: "..".into(), is_dir: true, size: 0,
            modified: String::new(), permissions: String::new(),
        }];
        app.remote_selected.insert(0);
        app.update(Message::RemoteDelete);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_delete_single() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "file.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];
        app.remote_selected.insert(1);
        app.update(Message::RemoteDelete);
        assert!(matches!(app.dialog, Dialog::ConfirmDelete(ref n) if n == "file.txt"));
    }

    #[test]
    fn smoke_remote_delete_batch_label() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "a.txt".into(), is_dir: false, size: 10, modified: String::new(), permissions: String::new() },
            FileEntry { name: "b.txt".into(), is_dir: false, size: 20, modified: String::new(), permissions: String::new() },
        ];
        app.remote_selected.insert(1);
        app.remote_selected.insert(2);
        app.update(Message::RemoteDelete);
        assert!(matches!(app.dialog, Dialog::ConfirmDelete(ref n) if n == "2 items"));
    }

    #[test]
    fn smoke_remote_rename_no_selection() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.update(Message::RemoteRename);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_rename_skips_dotdot() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![FileEntry {
            name: "..".into(), is_dir: true, size: 0,
            modified: String::new(), permissions: String::new(),
        }];
        app.remote_selected.insert(0);
        app.update(Message::RemoteRename);
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_rename_opens_dialog() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "data.csv".into(), is_dir: false, size: 50, modified: String::new(), permissions: String::new() },
        ];
        app.remote_selected.insert(1);
        app.update(Message::RemoteRename);
        assert!(matches!(app.dialog, Dialog::Rename(ref old, ref new) if old == "data.csv" && new == "data.csv"));
    }

    // --- Transfer state ---
    #[test]
    fn smoke_transfer_progress_updates() {
        let mut app = App::default();
        app.transfer_active = true;
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 500, total_size: 1000, done: false,
        }));
        assert!((app.transfer_progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn smoke_transfer_progress_zero_total() {
        let mut app = App::default();
        app.transfer_active = true;
        app.transfer_progress = 0.3;
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 0, total_size: 0, done: false,
        }));
        // Should not update (div by zero guard)
        assert!((app.transfer_progress - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn smoke_transfer_complete_logs() {
        let mut app = App::default();
        app.transfer_active = true;
        app.update(Message::TransferComplete("Downloaded 3 items".into()));
        assert!(!app.transfer_active);
        assert_eq!(app.transfer_progress, 1.0);
        assert_eq!(app.transfer_log.len(), 1);
        assert!(!app.transfer_log[0].is_error);
        assert!(app.transfer_log[0].message.contains("Downloaded"));
    }

    #[test]
    fn smoke_transfer_failed_logs_error() {
        let mut app = App::default();
        app.transfer_active = true;
        app.update(Message::TransferFailed("connection lost".into()));
        assert!(!app.transfer_active);
        assert_eq!(app.transfer_progress, 0.0);
        assert_eq!(app.transfer_log.len(), 1);
        assert!(app.transfer_log[0].is_error);
        assert!(app.transfer_log[0].message.contains("connection lost"));
    }

    #[test]
    fn smoke_multiple_transfers_accumulate_log() {
        let mut app = App::default();
        app.update(Message::TransferComplete("file1".into()));
        app.update(Message::TransferComplete("file2".into()));
        app.update(Message::TransferFailed("err".into()));
        assert_eq!(app.transfer_log.len(), 3);
        assert!(!app.transfer_log[0].is_error);
        assert!(!app.transfer_log[1].is_error);
        assert!(app.transfer_log[2].is_error);
    }

    // --- Remote op messages ---
    #[test]
    fn smoke_remote_op_done_sets_status() {
        let mut app = App::default();
        app.update(Message::RemoteOpDone("Created dir".into()));
        assert_eq!(app.status, "Created dir");
    }

    #[test]
    fn smoke_remote_op_failed_sets_status() {
        let mut app = App::default();
        app.update(Message::RemoteOpFailed("Permission denied".into()));
        assert!(app.status.contains("Permission denied"));
    }

    // --- Helper functions ---
    #[test]
    fn smoke_read_local_dir_root() {
        let entries = read_local_dir("/", false);
        assert!(!entries.is_empty());
        assert_eq!(entries[0].name, "..");
    }

    #[test]
    fn smoke_read_local_dir_nonexistent() {
        let entries = read_local_dir("/nonexistent_path_12345", false);
        // Should still have ".." entry
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "..");
    }

    #[test]
    fn smoke_read_local_dir_hidden_filtering() {
        let entries_no_hidden = read_local_dir("/home", false);
        let entries_with_hidden = read_local_dir("/home", true);
        assert!(entries_with_hidden.len() >= entries_no_hidden.len());
    }

    #[test]
    fn smoke_read_local_dir_sorts_dirs_first() {
        let entries = read_local_dir("/", false);
        // After "..", dirs should come before files
        let mut seen_file = false;
        for e in entries.iter().skip(1) {
            if !e.is_dir { seen_file = true; }
            if e.is_dir && seen_file {
                panic!("Directory '{}' appeared after a file", e.name);
            }
        }
    }

    #[test]
    fn smoke_remote_to_file_entries_adds_dotdot() {
        let entries = remote_to_file_entries(&[], false);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "..");
    }

    #[test]
    fn smoke_remote_to_file_entries_filters_hidden() {
        use crate::core::sftp::RemoteEntry;
        let remote = vec![
            RemoteEntry { name: ".hidden".into(), is_dir: false, size: 10, mtime: 0, permissions: 0o644 },
            RemoteEntry { name: "visible".into(), is_dir: false, size: 20, mtime: 0, permissions: 0o644 },
        ];
        let no_hidden = remote_to_file_entries(&remote, false);
        let with_hidden = remote_to_file_entries(&remote, true);
        assert_eq!(no_hidden.len(), 2);  // ".." + "visible"
        assert_eq!(with_hidden.len(), 3); // ".." + ".hidden" + "visible"
    }

    #[test]
    fn smoke_remote_to_file_entries_converts_permissions() {
        use crate::core::sftp::RemoteEntry;
        let remote = vec![
            RemoteEntry { name: "script.sh".into(), is_dir: false, size: 100, mtime: 1704067200, permissions: 0o755 },
        ];
        let entries = remote_to_file_entries(&remote, false);
        assert_eq!(entries[1].permissions, "rwxr-xr-x");
        assert_eq!(entries[1].modified, "2024-01-01 00:00");
    }

    #[test]
    fn smoke_sort_descending() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "alpha".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
            FileEntry { name: "beta".into(), is_dir: false, size: 200, modified: String::new(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Name, false);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[1].name, "beta");
        assert_eq!(entries[2].name, "alpha");
    }

    #[test]
    fn smoke_sort_by_modified() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "new".into(), is_dir: false, size: 0, modified: "2024-06-01 12:00".into(), permissions: String::new() },
            FileEntry { name: "old".into(), is_dir: false, size: 0, modified: "2024-01-01 00:00".into(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Modified, true);
        assert_eq!(entries[1].name, "old");
        assert_eq!(entries[2].name, "new");
    }

    #[test]
    fn smoke_sort_by_permissions() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "strict".into(), is_dir: false, size: 0, modified: String::new(), permissions: "r--------".into() },
            FileEntry { name: "open".into(), is_dir: false, size: 0, modified: String::new(), permissions: "rwxrwxrwx".into() },
        ];
        sort_entries(&mut entries, SortColumn::Permissions, true);
        assert_eq!(entries[1].name, "strict");
        assert_eq!(entries[2].name, "open");
    }

    #[test]
    fn smoke_sort_single_entry() {
        let mut entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
        ];
        sort_entries(&mut entries, SortColumn::Name, true);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn smoke_sort_empty() {
        let mut entries: Vec<FileEntry> = vec![];
        sort_entries(&mut entries, SortColumn::Name, true);
        assert!(entries.is_empty());
    }

    #[test]
    fn smoke_parent_path_trailing_slash() {
        assert_eq!(parent_path("/home/user"), "/home");
    }

    #[test]
    fn smoke_parent_path_empty() {
        assert_eq!(parent_path(""), "/");
    }

    #[test]
    fn smoke_parent_path_deep() {
        assert_eq!(parent_path("/a/b/c/d/e"), "/a/b/c/d");
    }

    // --- Timestamp edge cases ---
    #[test]
    fn smoke_format_timestamp_y2k() {
        // 2000-01-01 00:00:00 = 946684800
        assert_eq!(format_timestamp(946684800), "2000-01-01 00:00");
    }

    #[test]
    fn smoke_format_timestamp_far_future() {
        // 2038-01-19 03:14:07 (32-bit overflow)
        assert_eq!(format_timestamp(2147483647), "2038-01-19 03:14");
    }

    #[test]
    fn smoke_format_timestamp_end_of_year() {
        // 2024-12-31 23:59:00 = 1735689540
        assert_eq!(format_timestamp(1735689540), "2024-12-31 23:59");
    }

    // --- Permissions edge cases ---
    #[test]
    fn smoke_format_permissions_000() {
        assert_eq!(format_permissions(0o000), "");
    }

    #[test]
    fn smoke_format_permissions_777() {
        assert_eq!(format_permissions(0o777), "rwxrwxrwx");
    }

    #[test]
    fn smoke_format_permissions_400() {
        assert_eq!(format_permissions(0o400), "r--------");
    }

    #[test]
    fn smoke_format_permissions_111() {
        assert_eq!(format_permissions(0o111), "--x--x--x");
    }

    // --- Keyboard handling ---
    #[test]
    fn smoke_key_down_selects_next() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Local;
        if app.local_entries.len() > 2 {
            app.local_selected.clear();
            app.local_selected.insert(0);
            app.update(Message::KeyPressed(
                Key::Named(keyboard::key::Named::ArrowDown),
                Modifiers::empty(),
            ));
            assert!(app.local_selected.contains(&1));
            assert!(!app.local_selected.contains(&0));
        }
    }

    #[test]
    fn smoke_key_up_selects_prev() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Local;
        if app.local_entries.len() > 2 {
            app.local_selected.clear();
            app.local_selected.insert(2);
            app.last_anchor_local = Some(2);
            app.update(Message::KeyPressed(
                Key::Named(keyboard::key::Named::ArrowUp),
                Modifiers::empty(),
            ));
            assert!(app.local_selected.contains(&1));
            assert!(!app.local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_key_down_clamps_at_end() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Local;
        let max = app.local_entries.len().saturating_sub(1);
        app.local_selected.clear();
        app.local_selected.insert(max);
        app.last_anchor_local = Some(max);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        ));
        assert!(app.local_selected.contains(&max));
    }

    #[test]
    fn smoke_key_up_clamps_at_zero() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Local;
        app.local_selected.clear();
        app.local_selected.insert(0);
        app.last_anchor_local = Some(0);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowUp),
            Modifiers::empty(),
        ));
        assert!(app.local_selected.contains(&0));
    }

    // --- Connect state ---
    #[test]
    fn smoke_connect_state_fields_update() {
        let mut app = App::default();
        app.update(Message::Connect(ConnectMessage::HostChanged("server.com".into())));
        assert_eq!(app.connect_state.host, "server.com");
        app.update(Message::Connect(ConnectMessage::PortChanged("2222".into())));
        assert_eq!(app.connect_state.port, "2222");
        app.update(Message::Connect(ConnectMessage::UsernameChanged("admin".into())));
        assert_eq!(app.connect_state.username, "admin");
        app.update(Message::Connect(ConnectMessage::PasswordChanged("pass123".into())));
        assert_eq!(app.connect_state.password, "pass123");
    }

    #[test]
    fn smoke_connect_starts_connecting_screen() {
        let mut app = App::default();
        app.connect_state.host = "example.com".into();
        app.connect_state.port = "22".into();
        app.connect_state.username = "user".into();
        app.update(Message::Connect(ConnectMessage::Connect));
        assert_eq!(app.screen, Screen::Connecting);
        assert!(app.status.contains("example.com"));
    }

    #[test]
    fn smoke_connect_cancel_resets() {
        let mut app = App::default();
        app.connect_state.host = "old.com".into();
        app.update(Message::Connect(ConnectMessage::Cancel));
        // Cancel creates new state
        assert!(app.connect_state.host.is_empty());
    }

    #[test]
    fn smoke_remote_listing_updates_entries() {
        use crate::core::sftp::RemoteEntry;
        let mut app = App::default();
        let entries = vec![
            RemoteEntry { name: "docs".into(), is_dir: true, size: 0, mtime: 0, permissions: 0o755 },
            RemoteEntry { name: "readme.md".into(), is_dir: false, size: 500, mtime: 1704067200, permissions: 0o644 },
        ];
        app.update(Message::RemoteListing(entries, "/home/user".into()));
        assert_eq!(app.remote_path, "/home/user");
        assert_eq!(app.remote_entries.len(), 3); // ".." + 2
        assert!(app.remote_selected.is_empty());
    }

    #[test]
    fn smoke_remote_listing_failed() {
        let mut app = App::default();
        app.update(Message::RemoteListingFailed("timeout".into()));
        assert!(app.status.contains("timeout"));
    }

    // --- Local mkdir confirm (actual filesystem) ---
    #[test]
    fn smoke_local_mkdir_confirm() {
        let tmp = std::env::temp_dir().join("eurosftp_smoke_mkdir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.dialog = Dialog::LocalMkdir("testdir".into());
        app.update(Message::LocalMkdirConfirm);
        assert_eq!(app.dialog, Dialog::None);
        assert!(tmp.join("testdir").exists());
        assert!(app.status.contains("Created"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn smoke_local_mkdir_empty_name() {
        let mut app = App::default();
        app.dialog = Dialog::LocalMkdir(String::new());
        app.update(Message::LocalMkdirConfirm);
        assert_eq!(app.dialog, Dialog::None);
        // Should not have set status about creating
    }

    // --- Local rename (actual filesystem) ---
    #[test]
    fn smoke_local_rename_confirm() {
        let tmp = std::env::temp_dir().join("eurosftp_smoke_rename");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("old.txt"), "content").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.dialog = Dialog::LocalRename("old.txt".into(), "new.txt".into());
        app.update(Message::LocalRenameConfirm);
        assert_eq!(app.dialog, Dialog::None);
        assert!(!tmp.join("old.txt").exists());
        assert!(tmp.join("new.txt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn smoke_local_rename_same_name_noop() {
        let mut app = App::default();
        app.dialog = Dialog::LocalRename("same.txt".into(), "same.txt".into());
        app.update(Message::LocalRenameConfirm);
        assert_eq!(app.dialog, Dialog::None);
    }

    // --- Local delete (actual filesystem) ---
    #[test]
    fn smoke_local_delete_confirm() {
        let tmp = std::env::temp_dir().join("eurosftp_smoke_delete");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("todelete.txt"), "bye").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);
        // Find the index of "todelete.txt"
        let idx = app.local_entries.iter().position(|e| e.name == "todelete.txt").unwrap();
        app.local_selected.insert(idx);
        app.dialog = Dialog::LocalConfirmDelete("todelete.txt".into());
        app.update(Message::LocalDeleteConfirm);
        assert_eq!(app.dialog, Dialog::None);
        assert!(!tmp.join("todelete.txt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ===== INTEGRATION TESTS =====

    // --- Full local file lifecycle: create dir → create file → rename → delete ---
    #[test]
    fn integ_local_file_lifecycle() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_lifecycle");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();

        // 1. Create subdirectory via dialog
        app.update(Message::LocalMkdir);
        assert!(matches!(app.dialog, Dialog::LocalMkdir(_)));
        app.update(Message::LocalMkdirNameChanged("subdir".into()));
        app.update(Message::LocalMkdirConfirm);
        assert!(tmp.join("subdir").is_dir());
        assert!(app.status.contains("Created"));

        // 2. Verify entries refreshed and subdir is present
        let subdir_idx = app.local_entries.iter()
            .position(|e| e.name == "subdir")
            .expect("subdir should appear in entries after mkdir");
        assert!(app.local_entries[subdir_idx].is_dir);

        // 3. Create a file inside via filesystem, then refresh
        std::fs::write(tmp.join("data.txt"), "hello").unwrap();
        app.update(Message::LocalPanel(PanelMessage::Refresh));
        let file_idx = app.local_entries.iter()
            .position(|e| e.name == "data.txt")
            .expect("data.txt should appear after refresh");
        assert!(!app.local_entries[file_idx].is_dir);

        // 4. Rename file
        app.local_selected.clear();
        app.local_selected.insert(file_idx);
        app.update(Message::LocalRename);
        assert!(matches!(app.dialog, Dialog::LocalRename(ref old, _) if old == "data.txt"));
        app.update(Message::LocalRenameNameChanged("renamed.txt".into()));
        app.update(Message::LocalRenameConfirm);
        assert!(!tmp.join("data.txt").exists());
        assert!(tmp.join("renamed.txt").exists());

        // 5. Delete the renamed file
        app.local_entries = read_local_dir(&app.local_path, false);
        let renamed_idx = app.local_entries.iter()
            .position(|e| e.name == "renamed.txt").unwrap();
        app.local_selected.clear();
        app.local_selected.insert(renamed_idx);
        app.update(Message::LocalDelete);
        assert!(matches!(app.dialog, Dialog::LocalConfirmDelete(ref n) if n == "renamed.txt"));
        app.update(Message::LocalDeleteConfirm);
        assert!(!tmp.join("renamed.txt").exists());

        // 6. Delete the subdirectory
        app.local_entries = read_local_dir(&app.local_path, false);
        let subdir_idx = app.local_entries.iter()
            .position(|e| e.name == "subdir").unwrap();
        app.local_selected.clear();
        app.local_selected.insert(subdir_idx);
        app.dialog = Dialog::LocalConfirmDelete("subdir".into());
        app.update(Message::LocalDeleteConfirm);
        assert!(!tmp.join("subdir").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Navigation: breadcrumb click → go up → path segment → verify entries ---
    #[test]
    fn integ_local_navigation_flow() {
        let mut app = App::default();
        app.screen = Screen::Browser;

        // Navigate to /tmp
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/tmp".into())));
        assert_eq!(app.local_path, "/tmp");
        assert!(app.local_selected.is_empty());
        assert!(app.local_filter.is_empty());

        // Go up → should be /
        app.update(Message::LocalPanel(PanelMessage::GoUp));
        assert_eq!(app.local_path, "/");

        // Navigate to /usr/bin
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/usr".into())));
        assert_eq!(app.local_path, "/usr");
        let has_bin = app.local_entries.iter().any(|e| e.name == "bin" && e.is_dir);
        assert!(has_bin, "/usr should contain bin/");

        // Enter bin via double-click simulation (entry click on dir)
        let bin_idx = app.local_entries.iter().position(|e| e.name == "bin").unwrap();
        // First click sets selection
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(bin_idx, false, false)));
        // Immediate second click within 400ms triggers navigation
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(bin_idx, false, false)));
        assert_eq!(app.local_path, "/usr/bin");
    }

    // --- Keyboard navigation flow: arrows + enter ---
    #[test]
    fn integ_keyboard_navigation_enter_dir() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_kbd_nav");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("inner")).unwrap();
        std::fs::write(tmp.join("file.txt"), "x").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);
        app.active_panel = ActivePanel::Local;

        // Arrow down from top to select first real entry
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        ));
        let selected = *app.local_selected.iter().next().unwrap();
        assert!(selected >= 1);

        // Find the "inner" dir index
        let inner_idx = app.local_entries.iter()
            .position(|e| e.name == "inner").unwrap();

        // Navigate to inner via arrow keys
        app.local_selected.clear();
        app.local_selected.insert(inner_idx);
        app.last_anchor_local = Some(inner_idx);

        // Press Enter to open directory
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        assert!(app.local_path.ends_with("inner"));
        assert!(app.local_selected.is_empty());

        // Press Enter on ".." to go back up
        app.local_selected.insert(0); // ".." is always index 0
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        assert_eq!(app.local_path, tmp.to_string_lossy().to_string());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Keyboard shortcuts: F7 mkdir, F2 rename, Delete ---
    #[test]
    fn integ_keyboard_shortcuts_open_dialogs() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Local;

        // F7 → local mkdir dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert!(matches!(app.dialog, Dialog::LocalMkdir(_)));
        app.dialog = Dialog::None;

        // F2 with selection → local rename dialog
        app.local_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F2),
            Modifiers::empty(),
        ));
        if app.local_entries.len() > 1 && app.local_entries[1].name != ".." {
            assert!(matches!(app.dialog, Dialog::LocalRename(_, _)));
        }
        app.dialog = Dialog::None;

        // Delete with selection → local delete dialog
        app.local_selected.clear();
        app.local_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Delete),
            Modifiers::empty(),
        ));
        if app.local_entries.len() > 1 && app.local_entries[1].name != ".." {
            assert!(matches!(app.dialog, Dialog::LocalConfirmDelete(_)));
        }
    }

    // --- Keyboard shortcuts with remote active panel ---
    #[test]
    fn integ_keyboard_shortcuts_remote_panel() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.active_panel = ActivePanel::Remote;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "remote.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];

        // F7 → remote mkdir dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert!(matches!(app.dialog, Dialog::Mkdir(_)));
        app.dialog = Dialog::None;

        // F2 with selection → remote rename dialog
        app.remote_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F2),
            Modifiers::empty(),
        ));
        assert!(matches!(app.dialog, Dialog::Rename(ref old, _) if old == "remote.txt"));
        app.dialog = Dialog::None;

        // Delete → remote delete dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Delete),
            Modifiers::empty(),
        ));
        assert!(matches!(app.dialog, Dialog::ConfirmDelete(ref n) if n == "remote.txt"));
    }

    // --- Tab switches active panel ---
    #[test]
    fn integ_tab_switches_panel() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        assert_eq!(app.active_panel, ActivePanel::Local);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::empty(),
        ));
        assert_eq!(app.active_panel, ActivePanel::Remote);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::empty(),
        ));
        assert_eq!(app.active_panel, ActivePanel::Local);
    }

    // --- Dialogs block keyboard shortcuts ---
    #[test]
    fn integ_dialog_blocks_shortcuts() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.dialog = Dialog::Mkdir("test".into());

        // F7 should be blocked by open dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        // Dialog should still be Mkdir, not LocalMkdir
        assert!(matches!(app.dialog, Dialog::Mkdir(_)));
    }

    // --- Shortcuts blocked on Connect screen ---
    #[test]
    fn integ_shortcuts_blocked_on_connect_screen() {
        let mut app = App::default();
        assert_eq!(app.screen, Screen::Connect);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    // --- Sort → selection → filter pipeline ---
    #[test]
    fn integ_sort_clears_selection_and_filter_preserves() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_sort_filter");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("alpha.txt"), "a").unwrap();
        std::fs::write(tmp.join("beta.log"), "b").unwrap();
        std::fs::write(tmp.join("gamma.txt"), "ccc").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);

        // Select some items
        app.local_selected.insert(1);
        app.local_selected.insert(2);
        assert_eq!(app.local_selected.len(), 2);

        // Sort by size → should clear selection
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert!(app.local_selected.is_empty());

        // Apply filter
        app.update(Message::LocalPanel(PanelMessage::FilterChanged(".txt".into())));
        assert_eq!(app.local_filter, ".txt");
        assert!(app.local_selected.is_empty());

        // Entries still exist (filter is applied in view, not data)
        assert!(app.local_entries.iter().any(|e| e.name == "beta.log"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Multi-select → batch delete flow ---
    #[test]
    fn integ_multiselect_batch_delete() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_batch_del");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.txt"), "a").unwrap();
        std::fs::write(tmp.join("b.txt"), "b").unwrap();
        std::fs::write(tmp.join("c.txt"), "c").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);

        // Ctrl-click to select a.txt and c.txt
        let a_idx = app.local_entries.iter().position(|e| e.name == "a.txt").unwrap();
        let c_idx = app.local_entries.iter().position(|e| e.name == "c.txt").unwrap();

        app.modifiers = Modifiers::CTRL;
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(a_idx, false, false)));
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(c_idx, false, false)));
        assert!(app.local_selected.contains(&a_idx));
        assert!(app.local_selected.contains(&c_idx));

        // Trigger delete
        app.modifiers = Modifiers::empty();
        app.update(Message::LocalDelete);
        assert!(matches!(app.dialog, Dialog::LocalConfirmDelete(ref n) if n == "2 items"));

        // Confirm
        app.update(Message::LocalDeleteConfirm);
        assert!(!tmp.join("a.txt").exists());
        assert!(tmp.join("b.txt").exists());
        assert!(!tmp.join("c.txt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Remote entries conversion + sort + filter pipeline ---
    #[test]
    fn integ_remote_entries_sort_filter_pipeline() {
        use crate::core::sftp::RemoteEntry;
        let remote = vec![
            RemoteEntry { name: ".config".into(), is_dir: true, size: 0, mtime: 1700000000, permissions: 0o755 },
            RemoteEntry { name: "docs".into(), is_dir: true, size: 0, mtime: 1704067200, permissions: 0o755 },
            RemoteEntry { name: "readme.md".into(), is_dir: false, size: 500, mtime: 1718458200, permissions: 0o644 },
            RemoteEntry { name: "data.csv".into(), is_dir: false, size: 10000, mtime: 1709208000, permissions: 0o644 },
            RemoteEntry { name: ".hidden".into(), is_dir: false, size: 10, mtime: 0, permissions: 0o600 },
        ];

        // Without hidden
        let mut entries = remote_to_file_entries(&remote, false);
        assert_eq!(entries.len(), 4); // ".." + docs + readme + data
        assert_eq!(entries[0].name, "..");

        // Sort by size ascending
        sort_entries(&mut entries, SortColumn::Size, true);
        assert_eq!(entries[0].name, ".."); // always first
        // dirs before files
        assert_eq!(entries[1].name, "docs");
        // then files by size
        assert_eq!(entries[2].name, "readme.md"); // 500
        assert_eq!(entries[3].name, "data.csv");  // 10000

        // Sort by size descending
        sort_entries(&mut entries, SortColumn::Size, false);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[1].name, "docs"); // dirs still first
        assert_eq!(entries[2].name, "data.csv");
        assert_eq!(entries[3].name, "readme.md");

        // With hidden
        let entries_hidden = remote_to_file_entries(&remote, true);
        assert_eq!(entries_hidden.len(), 6); // ".." + all 5

        // Verify permissions converted correctly
        let readme = entries_hidden.iter().find(|e| e.name == "readme.md").unwrap();
        assert_eq!(readme.permissions, "rw-r--r--");
        let config = entries_hidden.iter().find(|e| e.name == ".config").unwrap();
        assert_eq!(config.permissions, "rwxr-xr-x");

        // Verify timestamps converted
        let readme = entries_hidden.iter().find(|e| e.name == "readme.md").unwrap();
        assert!(readme.modified.starts_with("2024-"));
        let hidden = entries_hidden.iter().find(|e| e.name == ".hidden").unwrap();
        assert!(hidden.modified.is_empty()); // mtime=0 → empty
    }

    // --- Profile save → load → apply round-trip through App ---
    #[test]
    fn integ_profile_save_load_apply() {
        use crate::core::connection::ConnectionProfile;
        let tmp = std::env::temp_dir().join("eurosftp_integ_profile");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Prepare a profile and save it
        let profile = ConnectionProfile {
            name: "integration-test".into(),
            host: "10.0.0.42".into(),
            port: 2222,
            username: "deploy".into(),
            auth: crate::core::connection::AuthMethod::Password("s3cret".into()),
        };
        let path = tmp.join("connections.json");
        let json = serde_json::to_string_pretty(&vec![profile]).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Load and apply
        let content = std::fs::read_to_string(&path).unwrap();
        let profiles: Vec<ConnectionProfile> = serde_json::from_str(&content).unwrap();

        let mut app = App::default();
        app.connect_state.profiles = profiles;
        app.connect_state.apply_profile("integration-test");

        assert_eq!(app.connect_state.host, "10.0.0.42");
        assert_eq!(app.connect_state.port, "2222");
        assert_eq!(app.connect_state.username, "deploy");
        assert_eq!(app.connect_state.password, "s3cret");
        assert_eq!(app.connect_state.selected_profile, Some("integration-test".into()));

        // Verify auth_method() returns correct type
        let auth = app.connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Password(ref p) if p == "s3cret"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Theme cycle through all + verify iced theme conversion ---
    #[test]
    fn integ_theme_full_cycle_with_conversion() {
        let mut app = App::default();
        let mut seen_themes = Vec::new();

        for _ in 0..AppTheme::all().len() {
            seen_themes.push(app.theme);
            // Verify the iced conversion doesn't panic
            let _ = app.theme();
            app.update(Message::CycleTheme);
        }
        // Should have seen all themes exactly once
        assert_eq!(seen_themes.len(), AppTheme::all().len());
        for t in AppTheme::all() {
            assert!(seen_themes.contains(t), "Missing theme: {:?}", t);
        }
        // Back to start
        assert_eq!(app.theme, AppTheme::Dark);
    }

    // --- Transfer log + toggle log interaction ---
    #[test]
    fn integ_transfer_log_accumulation_and_toggle() {
        let mut app = App::default();

        // Accumulate mixed log entries
        app.update(Message::TransferComplete("Downloaded file1.txt".into()));
        app.update(Message::TransferFailed("Permission denied on file2".into()));
        app.update(Message::TransferComplete("Uploaded 3 items".into()));

        assert_eq!(app.transfer_log.len(), 3);
        assert!(!app.transfer_log[0].is_error);
        assert!(app.transfer_log[1].is_error);
        assert!(!app.transfer_log[2].is_error);

        // All entries should have timestamps
        for entry in &app.transfer_log {
            assert!(!entry.timestamp.is_empty());
        }

        // Toggle log visibility
        assert!(!app.show_log);
        app.update(Message::ToggleLog);
        assert!(app.show_log);

        // Log content persists after toggle
        assert_eq!(app.transfer_log.len(), 3);
    }

    // --- Connection flow: fill form → connect → fail → back to form ---
    #[test]
    fn integ_connection_failure_roundtrip() {
        let mut app = App::default();

        // Fill connection form
        app.update(Message::Connect(ConnectMessage::HostChanged("192.168.1.99".into())));
        app.update(Message::Connect(ConnectMessage::PortChanged("22".into())));
        app.update(Message::Connect(ConnectMessage::UsernameChanged("testuser".into())));
        app.update(Message::Connect(ConnectMessage::PasswordChanged("badpass".into())));

        // Start connecting
        app.update(Message::Connect(ConnectMessage::Connect));
        assert_eq!(app.screen, Screen::Connecting);
        assert!(app.status.contains("192.168.1.99"));

        // Simulate failure
        app.update(Message::ConnectionFailed("Connection refused".into()));
        assert_eq!(app.screen, Screen::Connect);
        assert!(app.status.contains("Connection refused"));

        // Form values should still be there for retry
        assert_eq!(app.connect_state.host, "192.168.1.99");
        assert_eq!(app.connect_state.username, "testuser");
    }

    // --- Remote listing + navigate + selection state ---
    #[test]
    fn integ_remote_listing_navigation_state() {
        use crate::core::sftp::RemoteEntry;
        let mut app = App::default();
        app.screen = Screen::Browser;

        // Simulate initial listing
        let entries = vec![
            RemoteEntry { name: "docs".into(), is_dir: true, size: 0, mtime: 0, permissions: 0o755 },
            RemoteEntry { name: "src".into(), is_dir: true, size: 0, mtime: 0, permissions: 0o755 },
            RemoteEntry { name: "main.rs".into(), is_dir: false, size: 200, mtime: 1704067200, permissions: 0o644 },
        ];
        app.update(Message::RemoteListing(entries, "/home/user".into()));
        assert_eq!(app.remote_path, "/home/user");
        assert_eq!(app.remote_entries.len(), 4); // ".." + 3
        assert!(app.remote_selected.is_empty());

        // Select an entry
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(1, false, false)));
        assert!(app.remote_selected.contains(&1));
        assert_eq!(app.active_panel, ActivePanel::Remote);

        // New listing clears selection
        let new_entries = vec![
            RemoteEntry { name: "README.md".into(), is_dir: false, size: 100, mtime: 0, permissions: 0o644 },
        ];
        app.update(Message::RemoteListing(new_entries, "/home/user/docs".into()));
        assert_eq!(app.remote_path, "/home/user/docs");
        assert!(app.remote_selected.is_empty());
    }

    // --- Hidden toggle refreshes local entries ---
    #[test]
    fn integ_hidden_toggle_refreshes() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_hidden");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join(".hidden"), "secret").unwrap();
        std::fs::write(tmp.join("visible"), "public").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);

        let count_no_hidden = app.local_entries.len();
        assert!(!app.local_entries.iter().any(|e| e.name == ".hidden"));

        // Toggle hidden on (simulate ToggleHidden but without sftp)
        app.show_hidden = true;
        app.local_entries = read_local_dir(&app.local_path, true);
        assert!(app.local_entries.len() > count_no_hidden);
        assert!(app.local_entries.iter().any(|e| e.name == ".hidden"));

        // Toggle back
        app.show_hidden = false;
        app.local_entries = read_local_dir(&app.local_path, false);
        assert!(!app.local_entries.iter().any(|e| e.name == ".hidden"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Ctrl+H shortcut toggles hidden ---
    #[test]
    fn integ_ctrl_h_toggles_hidden() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        assert!(!app.show_hidden);

        app.update(Message::KeyPressed(
            Key::Character("h".into()),
            Modifiers::CTRL,
        ));
        assert!(app.show_hidden);

        app.update(Message::KeyPressed(
            Key::Character("h".into()),
            Modifiers::CTRL,
        ));
        assert!(!app.show_hidden);
    }

    // --- Selection + sort interaction: sort clears both panels ---
    #[test]
    fn integ_sort_clears_both_panels_selection() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "remote.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];

        app.local_selected.insert(1);
        app.remote_selected.insert(1);

        // Sort from local panel
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert!(app.local_selected.is_empty());
        assert!(app.remote_selected.is_empty());

        // Re-select
        app.local_selected.insert(1);
        app.remote_selected.insert(1);

        // Sort from remote panel
        app.update(Message::RemotePanel(PanelMessage::SortBy(SortColumn::Name)));
        assert!(app.local_selected.is_empty());
        assert!(app.remote_selected.is_empty());
    }

    // --- Save profile replaces existing with same name ---
    #[test]
    fn integ_save_profile_upsert() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.connect_state.host = "old.com".into();
        app.connect_state.port = "22".into();
        app.connect_state.username = "user".into();

        // Manually add a profile
        use crate::core::connection::{ConnectionProfile, AuthMethod};
        app.connect_state.profiles = vec![ConnectionProfile {
            name: "myserver".into(),
            host: "old.com".into(),
            port: 22,
            username: "user".into(),
            auth: AuthMethod::Agent,
        }];

        // Open save dialog and update host
        app.connect_state.host = "new.com".into();
        app.dialog = Dialog::SaveProfile("myserver".into());
        app.update(Message::SaveConnectionConfirm);

        // Should have updated, not duplicated
        assert_eq!(app.connect_state.profiles.len(), 1);
        assert_eq!(app.connect_state.profiles[0].host, "new.com");
    }

    // --- Shift-select on remote panel ---
    #[test]
    fn integ_shift_select_remote() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "a".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "b".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "c".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "d".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
        ];

        // Click entry 1 to set anchor
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(1, false, false)));
        assert_eq!(app.remote_selected.len(), 1);

        // Shift-click entry 3 to select range
        app.modifiers = Modifiers::SHIFT;
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(3, false, false)));
        assert_eq!(app.remote_selected.len(), 3);
        assert!(app.remote_selected.contains(&1));
        assert!(app.remote_selected.contains(&2));
        assert!(app.remote_selected.contains(&3));
    }

    // --- Auth type switching ---
    #[test]
    fn integ_auth_type_switch() {
        use crate::ui::connect_dialog::AuthType;
        let mut app = App::default();

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::Agent)));
        assert_eq!(app.connect_state.auth_type, AuthType::Agent);
        let auth = app.connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Agent));

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::KeyFile)));
        assert_eq!(app.connect_state.auth_type, AuthType::KeyFile);
        app.update(Message::Connect(ConnectMessage::KeyPathChanged("/tmp/key".into())));
        let auth = app.connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::KeyFile(ref p) if p.to_string_lossy() == "/tmp/key"));

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::Password)));
        app.update(Message::Connect(ConnectMessage::PasswordChanged("mypass".into())));
        let auth = app.connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Password(ref p) if p == "mypass"));
    }

    // --- Download/upload without connection is no-op ---
    #[test]
    fn integ_transfer_without_connection_noop() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "file.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];
        app.remote_selected.insert(1);
        app.local_selected.insert(1);

        // No sftp connection → should not panic or change state
        app.update(Message::DownloadSelected);
        assert!(!app.transfer_active);

        app.update(Message::UploadSelected);
        assert!(!app.transfer_active);
    }

    // --- Nested dir delete on filesystem ---
    #[test]
    fn integ_local_delete_nested_dir() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_nested_del");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("parent/child/grandchild")).unwrap();
        std::fs::write(tmp.join("parent/child/file.txt"), "data").unwrap();

        let mut app = App::default();
        app.screen = Screen::Browser;
        app.local_path = tmp.to_string_lossy().to_string();
        app.local_entries = read_local_dir(&app.local_path, false);

        let parent_idx = app.local_entries.iter().position(|e| e.name == "parent").unwrap();
        app.local_selected.insert(parent_idx);
        app.dialog = Dialog::LocalConfirmDelete("parent".into());
        app.update(Message::LocalDeleteConfirm);

        assert!(!tmp.join("parent").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ===== BATCH 4 TESTS =====

    // --- format_size ---
    #[test]
    fn b4_format_size_bytes() {
        assert_eq!(format_size(0), "0.0 B");
        assert_eq!(format_size(512), "512.0 B");
        assert_eq!(format_size(1023), "1023.0 B");
    }

    #[test]
    fn b4_format_size_kb() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn b4_format_size_mb() {
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn b4_format_size_gb() {
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn b4_format_size_tb() {
        assert_eq!(format_size(3 * 1024 * 1024 * 1024 * 1024), "3.0 TB");
    }

    // --- Escape closes dialogs ---
    #[test]
    fn b4_escape_closes_mkdir() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.dialog = Dialog::Mkdir("test".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_confirm_delete() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.dialog = Dialog::ConfirmDelete("file.txt".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_rename() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.dialog = Dialog::Rename("old".into(), "new".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_mkdir() {
        let mut app = App::default();
        app.dialog = Dialog::LocalMkdir("dir".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_delete() {
        let mut app = App::default();
        app.dialog = Dialog::LocalConfirmDelete("x".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_rename() {
        let mut app = App::default();
        app.dialog = Dialog::LocalRename("a".into(), "b".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_save_profile() {
        let mut app = App::default();
        app.dialog = Dialog::SaveProfile("name".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_noop_when_no_dialog() {
        let mut app = App::default();
        app.screen = Screen::Browser;
        app.dialog = Dialog::None;
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.dialog, Dialog::None);
    }

    // --- Transfer speed tracking state ---
    #[test]
    fn b4_transfer_start_state() {
        let mut app = App::default();
        assert!(app.transfer_start.is_none());
        assert_eq!(app.transfer_bytes, 0);
        assert_eq!(app.transfer_total, 0);
    }

    #[test]
    fn b4_transfer_progress_tracks_bytes() {
        let mut app = App::default();
        app.transfer_active = true;
        app.transfer_start = Some(Instant::now());
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 50000, total_size: 100000, done: false,
        }));
        assert_eq!(app.transfer_bytes, 50000);
        assert_eq!(app.transfer_total, 100000);
    }

    #[test]
    fn b4_transfer_complete_resets_start() {
        let mut app = App::default();
        app.transfer_active = true;
        app.transfer_start = Some(Instant::now());
        app.transfer_bytes = 1000;
        app.update(Message::TransferComplete("done".into()));
        assert!(app.transfer_start.is_none());
    }

    #[test]
    fn b4_transfer_failed_resets_start() {
        let mut app = App::default();
        app.transfer_active = true;
        app.transfer_start = Some(Instant::now());
        app.transfer_bytes = 500;
        app.update(Message::TransferFailed("err".into()));
        assert!(app.transfer_start.is_none());
    }

    // --- Selection size calculation ---
    #[test]
    fn b4_selection_size_files_only() {
        let entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "dir".into(), is_dir: true, size: 4096, modified: String::new(), permissions: String::new() },
            FileEntry { name: "a.txt".into(), is_dir: false, size: 1000, modified: String::new(), permissions: String::new() },
            FileEntry { name: "b.txt".into(), is_dir: false, size: 2000, modified: String::new(), permissions: String::new() },
        ];
        let mut selected = BTreeSet::new();
        selected.insert(1); // dir
        selected.insert(2); // a.txt
        selected.insert(3); // b.txt

        let sel_size: u64 = selected.iter()
            .filter_map(|&idx| entries.get(idx))
            .filter(|e| !e.is_dir && e.name != "..")
            .map(|e| e.size)
            .sum();

        assert_eq!(sel_size, 3000); // only files, not dir
    }

    #[test]
    fn b4_selection_size_no_selection() {
        let entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
        ];
        let selected: BTreeSet<usize> = BTreeSet::new();
        let sel_size: u64 = selected.iter()
            .filter_map(|&idx| entries.get(idx))
            .filter(|e| !e.is_dir && e.name != "..")
            .map(|e| e.size)
            .sum();
        assert_eq!(sel_size, 0);
    }
}
