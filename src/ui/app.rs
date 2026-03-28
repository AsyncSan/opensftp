use std::sync::Arc;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, horizontal_rule, progress_bar, row, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};
use iced::futures::stream;
use tokio::sync::mpsc;

use crate::core::sftp::{RemoteEntry, SftpConnection, TransferProgress};
use crate::theme::AppTheme;
use crate::ui::connect_dialog::{self, ConnectMessage, ConnectState};
use crate::ui::file_panel::{self, FileEntry, PanelMessage};

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
    Rename(String, String), // (old_name, new_name)
}

pub struct App {
    screen: Screen,
    theme: AppTheme,
    connect_state: ConnectState,
    local_path: String,
    remote_path: String,
    local_entries: Vec<FileEntry>,
    remote_entries: Vec<FileEntry>,
    local_selected: Option<usize>,
    remote_selected: Option<usize>,
    status: String,
    sftp: Option<Arc<SftpConnection>>,
    // Transfer state
    transfer_active: bool,
    transfer_progress: f32,
    transfer_status: String,
    // UI state
    active_panel: ActivePanel,
    dialog: Dialog,
}

impl Default for App {
    fn default() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());

        let local_entries = read_local_dir(&home);

        Self {
            screen: Screen::Connect,
            theme: AppTheme::default(),
            connect_state: ConnectState::new(),
            local_path: home,
            remote_path: String::new(),
            local_entries,
            remote_entries: Vec::new(),
            local_selected: None,
            remote_selected: None,
            status: "Not connected".to_string(),
            sftp: None,
            transfer_active: false,
            transfer_progress: 0.0,
            transfer_status: String::new(),
            active_panel: ActivePanel::Local,
            dialog: Dialog::None,
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
                self.remote_entries = remote_to_file_entries(&entries);
            }

            Message::ConnectionFailed(err) => {
                self.screen = Screen::Connect;
                self.status = format!("Failed: {}", err);
            }

            Message::LocalPanel(msg) => match msg {
                PanelMessage::EntryClicked(i) => {
                    if let Some(entry) = self.local_entries.get(i) {
                        if entry.is_dir {
                            let new_path = if entry.name == ".." {
                                std::path::Path::new(&self.local_path)
                                    .parent()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "/".to_string())
                            } else {
                                format!("{}/{}", self.local_path, entry.name)
                            };
                            self.local_entries = read_local_dir(&new_path);
                            self.local_path = new_path;
                            self.local_selected = None;
                        } else {
                            self.local_selected = Some(i);
                        }
                    }
                }
                PanelMessage::GoUp => {
                    if let Some(parent) = std::path::Path::new(&self.local_path).parent() {
                        let new_path = parent.to_string_lossy().to_string();
                        self.local_entries = read_local_dir(&new_path);
                        self.local_path = new_path;
                        self.local_selected = None;
                    }
                }
                PanelMessage::Refresh => {
                    self.local_entries = read_local_dir(&self.local_path);
                }
            },

            Message::RemotePanel(msg) => {
                let sftp = self.sftp.clone();
                match msg {
                    PanelMessage::EntryClicked(i) => {
                        if let Some(entry) = self.remote_entries.get(i) {
                            if entry.is_dir {
                                let new_path = if entry.name == ".." {
                                    parent_path(&self.remote_path)
                                } else {
                                    format!("{}/{}", self.remote_path, entry.name)
                                };
                                self.remote_path = new_path.clone();
                                self.remote_selected = None;

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
                            } else {
                                self.remote_selected = Some(i);
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
                }
            }

            Message::RemoteListing(entries, path) => {
                self.remote_path = path;
                self.remote_entries = remote_to_file_entries(&entries);
                self.remote_selected = None;
            }

            Message::RemoteListingFailed(err) => {
                self.status = format!("Error: {}", err);
            }

            // Download: remote selected file -> local path
            Message::DownloadSelected => {
                if let (Some(idx), Some(conn)) = (self.remote_selected, self.sftp.clone()) {
                    if let Some(entry) = self.remote_entries.get(idx) {
                        if !entry.is_dir {
                            let remote_file = format!("{}/{}", self.remote_path, entry.name);
                            let local_file = format!("{}/{}", self.local_path, entry.name);
                            let file_name = entry.name.clone();

                            self.transfer_active = true;
                            self.transfer_progress = 0.0;
                            self.transfer_status = format!("Downloading {}...", file_name);

                            let (event_tx, event_rx) = mpsc::unbounded_channel::<TransferEvent>();

                            // Spawn transfer, forward progress as events
                            tokio::spawn(async move {
                                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();

                                let dl = tokio::spawn({
                                    let conn = conn.clone();
                                    async move {
                                        conn.download(&remote_file, &local_file, progress_tx).await
                                    }
                                });

                                let fwd_tx = event_tx.clone();
                                let fwd = tokio::spawn(async move {
                                    let mut last = 0u64;
                                    while let Some(p) = progress_rx.recv().await {
                                        last = p.transferred;
                                        let _ = fwd_tx.send(TransferEvent::Progress {
                                            transferred: p.transferred,
                                            total_size: p.total_size,
                                        });
                                    }
                                    last
                                });

                                let transferred = fwd.await.unwrap_or(0);
                                match dl.await {
                                    Ok(Ok(())) => {
                                        let _ = event_tx.send(TransferEvent::Complete(
                                            format!("Downloaded {} ({} bytes)", file_name, transferred),
                                        ));
                                    }
                                    Ok(Err(e)) => {
                                        let _ = event_tx.send(TransferEvent::Failed(e.to_string()));
                                    }
                                    Err(e) => {
                                        let _ = event_tx.send(TransferEvent::Failed(e.to_string()));
                                    }
                                }
                            });

                            // Stream events into Iced messages
                            let event_stream = stream::unfold(event_rx, |mut rx| async {
                                rx.recv().await.map(|ev| (ev, rx))
                            });

                            return Task::run(event_stream, |ev| match ev {
                                TransferEvent::Progress { transferred, total_size } => {
                                    Message::TransferProgress(TransferProgress {
                                        transferred,
                                        total_size,
                                        done: false,
                                    })
                                }
                                TransferEvent::Complete(msg) => Message::TransferComplete(msg),
                                TransferEvent::Failed(msg) => Message::TransferFailed(msg),
                            });
                        }
                    }
                }
            }

            // Upload: local selected file -> remote path
            Message::UploadSelected => {
                if let (Some(idx), Some(conn)) = (self.local_selected, self.sftp.clone()) {
                    if let Some(entry) = self.local_entries.get(idx) {
                        if !entry.is_dir {
                            let local_file = format!("{}/{}", self.local_path, entry.name);
                            let remote_file = format!("{}/{}", self.remote_path, entry.name);
                            let file_name = entry.name.clone();

                            self.transfer_active = true;
                            self.transfer_progress = 0.0;
                            self.transfer_status = format!("Uploading {}...", file_name);

                            let (event_tx, event_rx) = mpsc::unbounded_channel::<TransferEvent>();

                            tokio::spawn(async move {
                                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();

                                let ul = tokio::spawn({
                                    let conn = conn.clone();
                                    async move {
                                        conn.upload(&local_file, &remote_file, progress_tx).await
                                    }
                                });

                                let fwd_tx = event_tx.clone();
                                let fwd = tokio::spawn(async move {
                                    let mut last = 0u64;
                                    while let Some(p) = progress_rx.recv().await {
                                        last = p.transferred;
                                        let _ = fwd_tx.send(TransferEvent::Progress {
                                            transferred: p.transferred,
                                            total_size: p.total_size,
                                        });
                                    }
                                    last
                                });

                                let transferred = fwd.await.unwrap_or(0);
                                match ul.await {
                                    Ok(Ok(())) => {
                                        let _ = event_tx.send(TransferEvent::Complete(
                                            format!("Uploaded {} ({} bytes)", file_name, transferred),
                                        ));
                                    }
                                    Ok(Err(e)) => {
                                        let _ = event_tx.send(TransferEvent::Failed(e.to_string()));
                                    }
                                    Err(e) => {
                                        let _ = event_tx.send(TransferEvent::Failed(e.to_string()));
                                    }
                                }
                            });

                            let event_stream = stream::unfold(event_rx, |mut rx| async {
                                rx.recv().await.map(|ev| (ev, rx))
                            });

                            return Task::run(event_stream, |ev| match ev {
                                TransferEvent::Progress { transferred, total_size } => {
                                    Message::TransferProgress(TransferProgress {
                                        transferred,
                                        total_size,
                                        done: false,
                                    })
                                }
                                TransferEvent::Complete(msg) => Message::TransferComplete(msg),
                                TransferEvent::Failed(msg) => Message::TransferFailed(msg),
                            });
                        }
                    }
                }
            }

            Message::TransferProgress(p) => {
                if p.total_size > 0 {
                    self.transfer_progress = p.transferred as f32 / p.total_size as f32;
                }
            }

            Message::TransferComplete(msg) => {
                self.transfer_active = false;
                self.transfer_progress = 1.0;
                self.transfer_status = msg;
                self.status = self.transfer_status.clone();

                // Refresh both panels
                self.local_entries = read_local_dir(&self.local_path);
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
                self.transfer_status = String::new();
                self.status = format!("Transfer failed: {}", err);
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
                    if let Some(idx) = self.remote_selected {
                        if let Some(entry) = self.remote_entries.get(idx) {
                            if entry.name != ".." {
                                self.dialog = Dialog::ConfirmDelete(entry.name.clone());
                            }
                        }
                    }
                }
            }
            Message::DeleteConfirm => {
                if let Dialog::ConfirmDelete(ref name) = self.dialog {
                    if let Some(conn) = self.sftp.clone() {
                        let is_dir = self.remote_entries.iter()
                            .find(|e| e.name == *name)
                            .map_or(false, |e| e.is_dir);
                        let path = format!("{}/{}", self.remote_path, name);
                        let refresh_path = self.remote_path.clone();
                        self.dialog = Dialog::None;
                        self.remote_selected = None;
                        return Task::perform(
                            async move {
                                if is_dir {
                                    conn.remove_dir(&path).await?;
                                } else {
                                    conn.remove_file(&path).await?;
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
                    if let Some(idx) = self.remote_selected {
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

            // Keyboard shortcuts
            Message::KeyPressed(key, _modifiers) => {
                if self.dialog != Dialog::None {
                    // Don't handle shortcuts when a dialog is open
                    return Task::none();
                }
                if self.screen != Screen::Browser {
                    return Task::none();
                }
                match key {
                    Key::Named(keyboard::key::Named::F5) => {
                        // Refresh both panels
                        self.local_entries = read_local_dir(&self.local_path);
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
                        return self.update(Message::RemoteDelete);
                    }
                    Key::Named(keyboard::key::Named::F2) => {
                        return self.update(Message::RemoteRename);
                    }
                    Key::Named(keyboard::key::Named::F7) => {
                        return self.update(Message::RemoteMkdir);
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
                let has_remote_selection = self.remote_selected.is_some()
                    && self.remote_entries.get(self.remote_selected.unwrap_or(0))
                        .map_or(false, |e| e.name != "..");

                let toolbar = row![
                    text(&self.status).size(13),
                    iced::widget::horizontal_space(),
                    button("F7 Mkdir").on_press(Message::RemoteMkdir).padding(4),
                    {
                        let mut btn = button("F2 Rename").padding(4);
                        if has_remote_selection { btn = btn.on_press(Message::RemoteRename); }
                        btn
                    },
                    {
                        let mut btn = button("Del").padding(4);
                        if has_remote_selection { btn = btn.on_press(Message::RemoteDelete); }
                        btn
                    },
                    button(self.theme.name())
                        .on_press(Message::CycleTheme)
                        .padding(4),
                    button("Disconnect")
                        .on_press(Message::Disconnect)
                        .padding(4),
                ]
                .spacing(8)
                .padding(8);

                // Transfer buttons between panels
                let can_upload = self.local_selected.is_some()
                    && self.local_entries.get(self.local_selected.unwrap_or(0))
                        .map_or(false, |e| !e.is_dir)
                    && !self.transfer_active;
                let can_download = self.remote_selected.is_some()
                    && self.remote_entries.get(self.remote_selected.unwrap_or(0))
                        .map_or(false, |e| !e.is_dir)
                    && !self.transfer_active;

                let mut upload_btn = button("Upload >>").padding(6);
                if can_upload {
                    upload_btn = upload_btn.on_press(Message::UploadSelected);
                }

                let mut download_btn = button("<< Download").padding(6);
                if can_download {
                    download_btn = download_btn.on_press(Message::DownloadSelected);
                }

                let transfer_controls = container(
                    column![
                        upload_btn,
                        download_btn,
                    ]
                    .spacing(8)
                    .align_x(iced::Alignment::Center),
                )
                .center_y(Length::Fill)
                .width(Length::Shrink)
                .padding(8);

                let panels = row![
                    file_panel::view_panel(
                        "Local",
                        &self.local_path,
                        &self.local_entries,
                        self.local_selected,
                    )
                    .map(Message::LocalPanel),
                    transfer_controls,
                    file_panel::view_panel(
                        "Remote",
                        &self.remote_path,
                        &self.remote_entries,
                        self.remote_selected,
                    )
                    .map(Message::RemotePanel),
                ]
                .spacing(0);

                // Transfer progress bar
                let transfer_bar: Element<'_, Message> = if self.transfer_active {
                    container(
                        column![
                            text(&self.transfer_status).size(12),
                            progress_bar(0.0..=1.0, self.transfer_progress)
                                .height(8),
                        ]
                        .spacing(4)
                        .padding(8),
                    )
                    .width(Length::Fill)
                    .into()
                } else {
                    container(text("")).width(Length::Fill).into()
                };

                // Dialog overlay
                let content: Element<'_, Message> = match &self.dialog {
                    Dialog::None => {
                        column![toolbar, horizontal_rule(1), panels, transfer_bar]
                            .spacing(0)
                            .into()
                    }
                    Dialog::Mkdir(name) => {
                        let dialog = container(
                            column![
                                text("New Directory").size(16),
                                text_input("Directory name", name)
                                    .on_input(Message::MkdirNameChanged)
                                    .on_submit(Message::MkdirConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::MkdirCancel).padding(6),
                                    button("Create").on_press(Message::MkdirConfirm).padding(6),
                                ]
                                .spacing(12),
                            ]
                            .spacing(12)
                            .max_width(300),
                        )
                        .padding(20)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill);

                        iced::widget::stack![
                            column![toolbar, horizontal_rule(1), panels, transfer_bar].spacing(0),
                            dialog,
                        ]
                        .into()
                    }
                    Dialog::ConfirmDelete(name) => {
                        let dialog = container(
                            column![
                                text("Delete").size(16),
                                text(format!("Delete \"{}\"?", name)).size(14),
                                row![
                                    button("Cancel").on_press(Message::DeleteCancel).padding(6),
                                    button("Delete").on_press(Message::DeleteConfirm).padding(6),
                                ]
                                .spacing(12),
                            ]
                            .spacing(12)
                            .max_width(300),
                        )
                        .padding(20)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill);

                        iced::widget::stack![
                            column![toolbar, horizontal_rule(1), panels, transfer_bar].spacing(0),
                            dialog,
                        ]
                        .into()
                    }
                    Dialog::Rename(old, new) => {
                        let dialog = container(
                            column![
                                text(format!("Rename \"{}\"", old)).size(16),
                                text_input("New name", new)
                                    .on_input(Message::RenameNameChanged)
                                    .on_submit(Message::RenameConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::RenameCancel).padding(6),
                                    button("Rename").on_press(Message::RenameConfirm).padding(6),
                                ]
                                .spacing(12),
                            ]
                            .spacing(12)
                            .max_width(300),
                        )
                        .padding(20)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill);

                        iced::widget::stack![
                            column![toolbar, horizontal_rule(1), panels, transfer_bar].spacing(0),
                            dialog,
                        ]
                        .into()
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

fn remote_to_file_entries(entries: &[RemoteEntry]) -> Vec<FileEntry> {
    let mut result = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
    }];

    result.extend(entries.iter().map(|e| FileEntry {
        name: e.name.clone(),
        is_dir: e.is_dir,
        size: e.size,
        modified: format_timestamp(e.mtime),
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

fn read_local_dir(path: &str) -> Vec<FileEntry> {
    let mut entries = vec![FileEntry {
        name: "..".to_string(),
        is_dir: true,
        size: 0,
        modified: String::new(),
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

                Some(FileEntry {
                    name: e.file_name().to_string_lossy().to_string(),
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                    modified,
                })
            })
            .collect();

        items.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
        });

        entries.extend(items);
    }

    entries
}

fn format_timestamp(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let days = secs / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}
