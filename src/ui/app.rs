use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, horizontal_rule, progress_bar, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};
use iced::futures::stream;
use tokio::sync::mpsc;
use super::icons;

use crate::core::sftp::{RemoteEntry, SftpConnection, TransferProgress};
use crate::core::connection::JumpHostConfig;
use crate::theme::{AppTheme, InstallMode, Settings};
use crate::ui::connect_dialog::{self, ConnectMessage, ConnectState};
use crate::ui::file_panel::{self, FileEntry, PanelMessage, SortColumn};
#[allow(unused_imports)]
use super::helpers::{format_size, format_permissions, format_timestamp, remote_to_file_entries, parent_path, sort_entries, read_local_dir};

/// Events yielded by the transfer stream for live progress.
#[derive(Debug, Clone)]
enum TransferEvent {
    Progress { transferred: u64, total_size: u64 },
    Complete(String),
    Failed(String),
}

#[derive(Clone)]
#[allow(dead_code)]
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
    MoveDownload,
    MoveUpload,
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
    // Theme
    OpenThemePicker,
    ThemeSelected(AppTheme),
    // Chmod
    RemoteChmod,
    ChmodValueChanged(String),
    ChmodConfirm,
    ChmodCancel,
    // Toggle
    ToggleHidden,
    ToggleBookmarks,
    ToggleLog,
    ToggleTerminal,
    // Terminal
    TerminalInputChanged(String),
    TerminalSubmit,
    TerminalOutput(String),
    TerminalError(String),
    TerminalClear,
    // Context menu
    ShowContextMenu(ContextAction),
    DismissContextMenu,
    // Keyboard
    KeyPressed(Key, Modifiers),
    // Drag and drop / file picker
    FilesDropped(Vec<PathBuf>),
    DragHovering(bool),
    BrowseUpload,
    BrowseUploadChosen(Vec<PathBuf>),
    // First-run / Settings
    FirstRunPortable,
    FirstRunInstall,
    OpenSettings,
    CloseSettings,
    SettingsSetInstallMode(InstallMode),
    InstallComplete(Result<String, String>),
    UninstallComplete(Result<String, String>),
    // Tab management
    NewTab,
    CloseTab(usize),
    SwitchTab(usize),
    NextTab,
    PrevTab,
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
            Self::MoveDownload => write!(f, "MoveDownload"),
            Self::MoveUpload => write!(f, "MoveUpload"),
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
            Self::OpenThemePicker => write!(f, "OpenThemePicker"),
            Self::ThemeSelected(t) => write!(f, "ThemeSelected({:?})", t),
            Self::RemoteChmod => write!(f, "RemoteChmod"),
            Self::ChmodValueChanged(v) => write!(f, "ChmodValueChanged({})", v),
            Self::ChmodConfirm => write!(f, "ChmodConfirm"),
            Self::ChmodCancel => write!(f, "ChmodCancel"),
            Self::ToggleHidden => write!(f, "ToggleHidden"),
            Self::ToggleBookmarks => write!(f, "ToggleBookmarks"),
            Self::ToggleLog => write!(f, "ToggleLog"),
            Self::ToggleTerminal => write!(f, "ToggleTerminal"),
            Self::TerminalInputChanged(s) => write!(f, "TerminalInputChanged({})", s),
            Self::TerminalSubmit => write!(f, "TerminalSubmit"),
            Self::TerminalOutput(s) => write!(f, "TerminalOutput({}..)", &s[..s.len().min(40)]),
            Self::TerminalError(e) => write!(f, "TerminalError({})", e),
            Self::TerminalClear => write!(f, "TerminalClear"),
            Self::ShowContextMenu(a) => write!(f, "ShowContextMenu({:?})", a),
            Self::DismissContextMenu => write!(f, "DismissContextMenu"),
            Self::KeyPressed(k, m) => write!(f, "KeyPressed({:?}, {:?})", k, m),
            Self::FilesDropped(p) => write!(f, "FilesDropped({} files)", p.len()),
            Self::DragHovering(h) => write!(f, "DragHovering({})", h),
            Self::BrowseUpload => write!(f, "BrowseUpload"),
            Self::BrowseUploadChosen(p) => write!(f, "BrowseUploadChosen({} files)", p.len()),
            Self::FirstRunPortable => write!(f, "FirstRunPortable"),
            Self::FirstRunInstall => write!(f, "FirstRunInstall"),
            Self::OpenSettings => write!(f, "OpenSettings"),
            Self::CloseSettings => write!(f, "CloseSettings"),
            Self::SettingsSetInstallMode(m) => write!(f, "SettingsSetInstallMode({:?})", m),
            Self::InstallComplete(r) => write!(f, "InstallComplete({:?})", r),
            Self::UninstallComplete(r) => write!(f, "UninstallComplete({:?})", r),
            Self::NewTab => write!(f, "NewTab"),
            Self::CloseTab(i) => write!(f, "CloseTab({})", i),
            Self::SwitchTab(i) => write!(f, "SwitchTab({})", i),
            Self::NextTab => write!(f, "NextTab"),
            Self::PrevTab => write!(f, "PrevTab"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Connecting,
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivePanel {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    None,
    Mkdir(String),
    ConfirmDelete(String),
    Rename(String, String),      // (old_name, new_name)
    LocalMkdir(String),
    LocalConfirmDelete(String),
    LocalRename(String, String), // (old_name, new_name)
    SaveProfile(String),         // profile name
    ThemePicker,
    Chmod(String, String),       // (path, octal_value)
    FirstRun,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenu {
    None,
    Local(usize),
    Remote(usize),
}

#[derive(Debug, Clone)]
pub(crate) enum ContextAction {
    Open,
    Download,
    Upload,
    Rename,
    Delete,
    NewDir,
    CopyPath,
    Refresh,
    Chmod,
}

#[derive(Debug, Clone)]
pub struct TransferLogEntry {
    message: String,
    is_error: bool,
    timestamp: String,
}

#[derive(Debug, Clone)]
pub struct TerminalEntry {
    command: String,
    output: String,
    is_error: bool,
}

pub type SessionId = u64;

pub struct Session {
    pub id: SessionId,
    pub label: String,
    pub screen: Screen,
    pub connect_state: ConnectState,
    pub local_path: String,
    pub remote_path: String,
    pub local_entries: Vec<FileEntry>,
    pub remote_entries: Vec<FileEntry>,
    pub local_selected: BTreeSet<usize>,
    pub remote_selected: BTreeSet<usize>,
    pub last_anchor_local: Option<usize>,
    pub last_anchor_remote: Option<usize>,
    pub status: String,
    pub sftp: Option<Arc<SftpConnection>>,
    pub transfer_active: bool,
    pub transfer_progress: f32,
    pub transfer_status: String,
    pub transfer_start: Option<Instant>,
    pub transfer_bytes: u64,
    pub transfer_total: u64,
    pub active_panel: ActivePanel,
    pub dialog: Dialog,
    pub show_hidden: bool,
    pub local_filter: String,
    pub remote_filter: String,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
    pub last_local_click: Option<(usize, Instant)>,
    pub last_remote_click: Option<(usize, Instant)>,
    pub transfer_log: Vec<TransferLogEntry>,
    pub context_menu: ContextMenu,
    pub connected_at: Option<Instant>,
    pub show_terminal: bool,
    pub terminal_input: String,
    pub terminal_history: Vec<TerminalEntry>,
    pub terminal_running: bool,
    pub terminal_cmd_history: Vec<String>,
    pub terminal_history_idx: Option<usize>,
    pub drop_hovering: bool,
    pub drop_pending: Vec<PathBuf>,
}

pub struct App {
    pub sessions: Vec<Session>,
    pub active_tab: usize,
    pub next_session_id: SessionId,
    // Global state
    theme: AppTheme,
    settings: Settings,
    modifiers: Modifiers,
    show_bookmarks_bar: bool,
    show_log: bool,
}

impl Session {
    pub fn new(id: SessionId) -> Self {
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

        let initial_dialog = Dialog::None;

        Self {
            id,
            label: "New Connection".to_string(),
            screen: demo_screen,
            connect_state: ConnectState::new(),
            local_path: home,
            remote_path: demo_remote_path,
            local_entries,
            remote_entries: demo_remote_entries,
            local_selected: BTreeSet::new(),
            remote_selected: BTreeSet::new(),
            last_anchor_local: None,
            last_anchor_remote: None,
            status: demo_status,
            sftp: None,
            transfer_active: false,
            transfer_progress: 0.0,
            transfer_status: String::new(),
            transfer_start: None,
            transfer_bytes: 0,
            transfer_total: 0,
            active_panel: ActivePanel::Local,
            dialog: initial_dialog,
            show_hidden: false,
            local_filter: String::new(),
            remote_filter: String::new(),
            sort_column: SortColumn::Name,
            sort_ascending: true,
            last_local_click: None,
            last_remote_click: None,
            transfer_log: Vec::new(),
            context_menu: ContextMenu::None,
            connected_at: None,
            show_terminal: false,
            terminal_input: String::new(),
            terminal_history: Vec::new(),
            terminal_running: false,
            terminal_cmd_history: Vec::new(),
            terminal_history_idx: None,
            drop_hovering: false,
            drop_pending: Vec::new(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        let settings = Settings::load();
        let mut session = Session::new(0);

        // First-run dialog goes on the session for now
        if !cfg!(test) && settings.is_first_run() {
            session.dialog = Dialog::FirstRun;
        }

        Self {
            sessions: vec![session],
            active_tab: 0,
            next_session_id: 1,
            theme: settings.theme,
            settings,
            modifiers: Modifiers::default(),
            show_bookmarks_bar: true,
            show_log: false,
        }
    }
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Connect(msg) => match msg {
                ConnectMessage::HostChanged(v) => self.sessions[self.active_tab].connect_state.host = v,
                ConnectMessage::PortChanged(v) => self.sessions[self.active_tab].connect_state.port = v,
                ConnectMessage::UsernameChanged(v) => self.sessions[self.active_tab].connect_state.username = v,
                ConnectMessage::PasswordChanged(v) => self.sessions[self.active_tab].connect_state.password = v,
                ConnectMessage::AuthTypeSelected(t) => self.sessions[self.active_tab].connect_state.auth_type = t,
                ConnectMessage::KeyPathChanged(v) => self.sessions[self.active_tab].connect_state.key_path = v,
                ConnectMessage::JumpHostToggled(enabled) => self.sessions[self.active_tab].connect_state.use_jump_host = enabled,
                ConnectMessage::JumpHostChanged(v) => self.sessions[self.active_tab].connect_state.jump_host = v,
                ConnectMessage::JumpPortChanged(v) => self.sessions[self.active_tab].connect_state.jump_port = v,
                ConnectMessage::JumpUsernameChanged(v) => self.sessions[self.active_tab].connect_state.jump_username = v,
                ConnectMessage::JumpPasswordChanged(v) => self.sessions[self.active_tab].connect_state.jump_password = v,
                ConnectMessage::JumpAuthTypeSelected(t) => self.sessions[self.active_tab].connect_state.jump_auth_type = t,
                ConnectMessage::JumpKeyPathChanged(v) => self.sessions[self.active_tab].connect_state.jump_key_path = v,
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
                    self.sessions[self.active_tab].connect_state.key_path = path;
                }
                ConnectMessage::KeyFileChosen(None) => {}
                ConnectMessage::ProfileSelected(name) => {
                    self.sessions[self.active_tab].connect_state.apply_profile(&name);
                }
                ConnectMessage::QuickConnect(name) => {
                    // If current tab is already connected, open a new tab
                    if self.sessions[self.active_tab].screen == Screen::Browser {
                        let id = self.next_session_id;
                        self.next_session_id += 1;
                        let mut new_session = Session::new(id);
                        // Copy profiles to new session
                        new_session.connect_state.profiles = self.sessions[self.active_tab].connect_state.profiles.clone();
                        self.sessions.push(new_session);
                        self.active_tab = self.sessions.len() - 1;
                    }
                    self.sessions[self.active_tab].connect_state.apply_profile(&name);
                    return self.update(Message::Connect(ConnectMessage::Connect));
                }
                ConnectMessage::DeleteProfile(name) => {
                    self.sessions[self.active_tab].connect_state.profiles.retain(|p| p.name != name);
                    let _ = crate::core::connection::ConnectionProfile::save_all(&self.sessions[self.active_tab].connect_state.profiles);
                    if self.sessions[self.active_tab].connect_state.selected_profile.as_ref() == Some(&name) {
                        self.sessions[self.active_tab].connect_state.selected_profile = None;
                    }
                    self.sessions[self.active_tab].status = format!("Deleted profile \"{}\"", name);
                }
                ConnectMessage::ToggleFavorite(name) => {
                    if let Some(profile) = self.sessions[self.active_tab].connect_state.profiles.iter_mut().find(|p| p.name == name) {
                        profile.favorite = !profile.favorite;
                    }
                    let _ = crate::core::connection::ConnectionProfile::save_all(&self.sessions[self.active_tab].connect_state.profiles);
                    if let Some(profile) = self.sessions[self.active_tab].connect_state.profiles.iter().find(|p| p.name == name) {
                        self.sessions[self.active_tab].status = if profile.favorite {
                            format!("Favorited \"{}\"", name)
                        } else {
                            format!("Unfavorited \"{}\"", name)
                        };
                    }
                }
                ConnectMessage::Connect => {
                    self.sessions[self.active_tab].screen = Screen::Connecting;
                    self.sessions[self.active_tab].status = format!(
                        "Connecting to {}:{}...",
                        self.sessions[self.active_tab].connect_state.host, self.sessions[self.active_tab].connect_state.port
                    );

                    let host = self.sessions[self.active_tab].connect_state.host.clone();
                    let port: u16 = self.sessions[self.active_tab].connect_state.port.parse().unwrap_or(22);
                    let username = self.sessions[self.active_tab].connect_state.username.clone();
                    let auth = self.sessions[self.active_tab].connect_state.auth_method();

                    let jump_host: Option<JumpHostConfig> = self.sessions[self.active_tab].connect_state.jump_host_config();
                    return Task::perform(
                        async move {
                            let conn = if let Some(jump) = jump_host {
                                SftpConnection::connect_with_jump_host(&host, port, &username, &auth, &jump).await?
                            } else {
                                SftpConnection::connect(&host, port, &username, &auth).await?
                            };
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
                    self.sessions[self.active_tab].connect_state = ConnectState::new();
                }
            },

            Message::Connected(conn, entries, home) => {
                self.sessions[self.active_tab].label = format!("{}@{}", self.sessions[self.active_tab].connect_state.username, self.sessions[self.active_tab].connect_state.host);
                self.sessions[self.active_tab].status = format!("Connected to {}", self.sessions[self.active_tab].connect_state.host);
                self.sessions[self.active_tab].screen = Screen::Browser;
                self.sessions[self.active_tab].sftp = Some(conn);
                self.sessions[self.active_tab].remote_path = home;
                self.sessions[self.active_tab].remote_entries = remote_to_file_entries(&entries, self.sessions[self.active_tab].show_hidden);
                self.sessions[self.active_tab].connected_at = Some(Instant::now());

                // Auto-save connection profile
                {
                    use crate::core::connection::ConnectionProfile;
                    let host = &self.sessions[self.active_tab].connect_state.host;
                    let username = &self.sessions[self.active_tab].connect_state.username;
                    let port: u16 = self.sessions[self.active_tab].connect_state.port.parse().unwrap_or(22);
                    let auto_name = format!("{}@{}", username, host);
                    let profile = ConnectionProfile {
                        name: auto_name.clone(),
                        host: host.clone(),
                        port,
                        username: username.clone(),
                        auth: self.sessions[self.active_tab].connect_state.auth_method(),
                        favorite: false,
                        jump_host: self.sessions[self.active_tab].connect_state.jump_host_config(),
                    };
                    let mut profiles = self.sessions[self.active_tab].connect_state.profiles.clone();
                    if let Some(existing) = profiles.iter_mut().find(|p| p.host == *host && p.username == *username && p.port == port) {
                        existing.auth = profile.auth;
                        existing.jump_host = profile.jump_host;
                    } else {
                        profiles.push(profile);
                    }
                    let _ = ConnectionProfile::save_all(&profiles);
                    self.sessions[self.active_tab].connect_state.profiles = profiles;
                }
            }

            Message::ConnectionFailed(err) => {
                self.sessions[self.active_tab].screen = Screen::Connect;
                self.sessions[self.active_tab].status = format!("Failed: {}", err);
            }

            Message::LocalPanel(msg) => {
                self.sessions[self.active_tab].active_panel = ActivePanel::Local;
                match msg {
                    PanelMessage::EntryClicked(i, _, _) => {
                        let ctrl = self.modifiers.control();
                        let shift = self.modifiers.shift();

                        let is_double = self.sessions[self.active_tab].last_local_click
                            .map(|(idx, t)| idx == i && t.elapsed().as_millis() < 400)
                            .unwrap_or(false);
                        self.sessions[self.active_tab].last_local_click = Some((i, Instant::now()));

                        if let Some(entry) = self.sessions[self.active_tab].local_entries.get(i) {
                            if entry.is_dir && (is_double || entry.name == "..") {
                                let new_path = if entry.name == ".." {
                                    std::path::Path::new(&self.sessions[self.active_tab].local_path)
                                        .parent()
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "/".to_string())
                                } else {
                                    format!("{}/{}", self.sessions[self.active_tab].local_path, entry.name)
                                };
                                self.sessions[self.active_tab].local_entries = read_local_dir(&new_path, self.sessions[self.active_tab].show_hidden);
                                self.sessions[self.active_tab].local_path = new_path;
                                self.sessions[self.active_tab].local_selected.clear();
                                self.sessions[self.active_tab].last_anchor_local = None;
                                self.sessions[self.active_tab].last_local_click = None;
                            } else if shift {
                                if let Some(anchor) = self.sessions[self.active_tab].last_anchor_local {
                                    let (lo, hi) = if anchor <= i { (anchor, i) } else { (i, anchor) };
                                    self.sessions[self.active_tab].local_selected.clear();
                                    for idx in lo..=hi {
                                        self.sessions[self.active_tab].local_selected.insert(idx);
                                    }
                                } else {
                                    self.sessions[self.active_tab].local_selected.clear();
                                    self.sessions[self.active_tab].local_selected.insert(i);
                                    self.sessions[self.active_tab].last_anchor_local = Some(i);
                                }
                            } else if ctrl {
                                if self.sessions[self.active_tab].local_selected.contains(&i) {
                                    self.sessions[self.active_tab].local_selected.remove(&i);
                                } else {
                                    self.sessions[self.active_tab].local_selected.insert(i);
                                }
                                self.sessions[self.active_tab].last_anchor_local = Some(i);
                            } else {
                                self.sessions[self.active_tab].local_selected.clear();
                                self.sessions[self.active_tab].local_selected.insert(i);
                                self.sessions[self.active_tab].last_anchor_local = Some(i);
                            }
                        }
                    }
                    PanelMessage::GoUp => {
                        if let Some(parent) = std::path::Path::new(&self.sessions[self.active_tab].local_path).parent() {
                            let new_path = parent.to_string_lossy().to_string();
                            self.sessions[self.active_tab].local_entries = read_local_dir(&new_path, self.sessions[self.active_tab].show_hidden);
                            self.sessions[self.active_tab].local_path = new_path;
                            self.sessions[self.active_tab].local_selected.clear();
                            self.sessions[self.active_tab].local_filter.clear();
                        }
                    }
                    PanelMessage::GoHome => {
                        let home = dirs::home_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "/".to_string());
                        self.sessions[self.active_tab].local_entries = read_local_dir(&home, self.sessions[self.active_tab].show_hidden);
                        self.sessions[self.active_tab].local_path = home;
                        self.sessions[self.active_tab].local_selected.clear();
                        self.sessions[self.active_tab].local_filter.clear();
                    }
                    PanelMessage::Refresh => {
                        self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                    }
                    PanelMessage::PathSegmentClicked(path) => {
                        self.sessions[self.active_tab].local_entries = read_local_dir(&path, self.sessions[self.active_tab].show_hidden);
                        self.sessions[self.active_tab].local_path = path;
                        self.sessions[self.active_tab].local_selected.clear();
                        self.sessions[self.active_tab].local_filter.clear();
                    }
                    PanelMessage::FilterChanged(f) => {
                        self.sessions[self.active_tab].local_filter = f;
                        self.sessions[self.active_tab].local_selected.clear();
                    }
                    PanelMessage::EntryRightClicked(i) => {
                        if !self.sessions[self.active_tab].local_selected.contains(&i) {
                            self.sessions[self.active_tab].local_selected.clear();
                            self.sessions[self.active_tab].local_selected.insert(i);
                            self.sessions[self.active_tab].last_anchor_local = Some(i);
                        }
                        self.sessions[self.active_tab].context_menu = ContextMenu::Local(i);
                    }
                    PanelMessage::SortBy(col) => {
                        if self.sessions[self.active_tab].sort_column == col {
                            self.sessions[self.active_tab].sort_ascending = !self.sessions[self.active_tab].sort_ascending;
                        } else {
                            self.sessions[self.active_tab].sort_column = col;
                            self.sessions[self.active_tab].sort_ascending = true;
                        }
                        {
                            let s = &mut self.sessions[self.active_tab];
                            let col = s.sort_column;
                            let asc = s.sort_ascending;
                            sort_entries(&mut s.local_entries, col, asc);
                            sort_entries(&mut s.remote_entries, col, asc);
                            s.local_selected.clear();
                            s.remote_selected.clear();
                        }
                    }
                }
            }

            Message::RemotePanel(msg) => {
                self.sessions[self.active_tab].active_panel = ActivePanel::Remote;
                let sftp = self.sessions[self.active_tab].sftp.clone();
                match msg {
                    PanelMessage::EntryClicked(i, _, _) => {
                        let ctrl = self.modifiers.control();
                        let shift = self.modifiers.shift();

                        let is_double = self.sessions[self.active_tab].last_remote_click
                            .map(|(idx, t)| idx == i && t.elapsed().as_millis() < 400)
                            .unwrap_or(false);
                        self.sessions[self.active_tab].last_remote_click = Some((i, Instant::now()));

                        if let Some(entry) = self.sessions[self.active_tab].remote_entries.get(i) {
                            if entry.is_dir && (is_double || entry.name == "..") {
                                let new_path = if entry.name == ".." {
                                    parent_path(&self.sessions[self.active_tab].remote_path)
                                } else {
                                    format!("{}/{}", self.sessions[self.active_tab].remote_path, entry.name)
                                };
                                self.sessions[self.active_tab].remote_path = new_path.clone();
                                self.sessions[self.active_tab].remote_selected.clear();
                                self.sessions[self.active_tab].last_anchor_remote = None;
                                self.sessions[self.active_tab].last_remote_click = None;

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
                                if let Some(anchor) = self.sessions[self.active_tab].last_anchor_remote {
                                    let (lo, hi) = if anchor <= i { (anchor, i) } else { (i, anchor) };
                                    self.sessions[self.active_tab].remote_selected.clear();
                                    for idx in lo..=hi {
                                        self.sessions[self.active_tab].remote_selected.insert(idx);
                                    }
                                } else {
                                    self.sessions[self.active_tab].remote_selected.clear();
                                    self.sessions[self.active_tab].remote_selected.insert(i);
                                    self.sessions[self.active_tab].last_anchor_remote = Some(i);
                                }
                            } else if ctrl {
                                if self.sessions[self.active_tab].remote_selected.contains(&i) {
                                    self.sessions[self.active_tab].remote_selected.remove(&i);
                                } else {
                                    self.sessions[self.active_tab].remote_selected.insert(i);
                                }
                                self.sessions[self.active_tab].last_anchor_remote = Some(i);
                            } else {
                                self.sessions[self.active_tab].remote_selected.clear();
                                self.sessions[self.active_tab].remote_selected.insert(i);
                                self.sessions[self.active_tab].last_anchor_remote = Some(i);
                            }
                        }
                    }
                    PanelMessage::GoUp => {
                        let new_path = parent_path(&self.sessions[self.active_tab].remote_path);
                        self.sessions[self.active_tab].remote_path = new_path.clone();

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
                    PanelMessage::GoHome => {
                        // Navigate to remote home via canonicalize(".")
                        if let Some(conn) = sftp.clone() {
                            return Task::perform(
                                async move {
                                    let home = conn.sftp.canonicalize(".").await
                                        .unwrap_or_else(|_| "/".to_string());
                                    let entries = conn.list_dir(&home).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, home))
                                },
                                |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteListingFailed(e.to_string()),
                                },
                            );
                        }
                    }
                    PanelMessage::Refresh => {
                        let path = self.sessions[self.active_tab].remote_path.clone();
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
                        self.sessions[self.active_tab].remote_path = path.clone();
                        self.sessions[self.active_tab].remote_selected.clear();
                        self.sessions[self.active_tab].remote_filter.clear();
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
                    PanelMessage::EntryRightClicked(i) => {
                        if !self.sessions[self.active_tab].remote_selected.contains(&i) {
                            self.sessions[self.active_tab].remote_selected.clear();
                            self.sessions[self.active_tab].remote_selected.insert(i);
                            self.sessions[self.active_tab].last_anchor_remote = Some(i);
                        }
                        self.sessions[self.active_tab].context_menu = ContextMenu::Remote(i);
                    }
                    PanelMessage::FilterChanged(f) => {
                        self.sessions[self.active_tab].remote_filter = f;
                        self.sessions[self.active_tab].remote_selected.clear();
                    }
                    PanelMessage::SortBy(col) => {
                        if self.sessions[self.active_tab].sort_column == col {
                            self.sessions[self.active_tab].sort_ascending = !self.sessions[self.active_tab].sort_ascending;
                        } else {
                            self.sessions[self.active_tab].sort_column = col;
                            self.sessions[self.active_tab].sort_ascending = true;
                        }
                        {
                            let s = &mut self.sessions[self.active_tab];
                            let col = s.sort_column;
                            let asc = s.sort_ascending;
                            sort_entries(&mut s.local_entries, col, asc);
                            sort_entries(&mut s.remote_entries, col, asc);
                            s.local_selected.clear();
                            s.remote_selected.clear();
                        }
                    }
                }
            }

            Message::RemoteListing(entries, path) => {
                self.sessions[self.active_tab].remote_path = path;
                self.sessions[self.active_tab].remote_entries = remote_to_file_entries(&entries, self.sessions[self.active_tab].show_hidden);
                self.sessions[self.active_tab].remote_selected.clear();
            }

            Message::RemoteListingFailed(err) => {
                self.sessions[self.active_tab].status = format!("Error: {}", err);
            }

            // Download: remote selected -> local path (batch)
            Message::DownloadSelected => {
                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.sessions[self.active_tab].remote_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name),
                            format!("{}/{}", self.sessions[self.active_tab].local_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.sessions[self.active_tab].transfer_active = true;
                        self.sessions[self.active_tab].transfer_progress = 0.0;
                        self.sessions[self.active_tab].transfer_start = Some(Instant::now());
                        self.sessions[self.active_tab].transfer_bytes = 0;
                        self.sessions[self.active_tab].transfer_total = 0;
                        self.sessions[self.active_tab].transfer_status = format!("Downloading {} item(s)...", count);

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
                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.sessions[self.active_tab].local_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.sessions[self.active_tab].local_path, e.name),
                            format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.sessions[self.active_tab].transfer_active = true;
                        self.sessions[self.active_tab].transfer_progress = 0.0;
                        self.sessions[self.active_tab].transfer_start = Some(Instant::now());
                        self.sessions[self.active_tab].transfer_bytes = 0;
                        self.sessions[self.active_tab].transfer_total = 0;
                        self.sessions[self.active_tab].transfer_status = format!("Uploading {} item(s)...", count);

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

            // Move: download remote -> local, then delete remote sources
            Message::MoveDownload => {
                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.sessions[self.active_tab].remote_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name),
                            format!("{}/{}", self.sessions[self.active_tab].local_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.sessions[self.active_tab].transfer_active = true;
                        self.sessions[self.active_tab].transfer_progress = 0.0;
                        self.sessions[self.active_tab].transfer_start = Some(Instant::now());
                        self.sessions[self.active_tab].transfer_bytes = 0;
                        self.sessions[self.active_tab].transfer_total = 0;
                        self.sessions[self.active_tab].transfer_status = format!("Moving {} item(s)...", count);

                        return spawn_transfer(move |progress_tx| async move {
                            for (remote_src, local_dst, is_dir) in &items {
                                if *is_dir {
                                    conn.download_dir(remote_src, local_dst, progress_tx.clone()).await?;
                                } else {
                                    conn.download(remote_src, local_dst, progress_tx.clone()).await?;
                                }
                            }
                            // Delete remote sources after successful download
                            for (remote_src, _, is_dir) in &items {
                                if *is_dir {
                                    conn.remove_dir_all(remote_src).await?;
                                } else {
                                    conn.remove_file(remote_src).await?;
                                }
                            }
                            Ok(format!("Moved {} item(s)", count))
                        });
                    }
                }
            }

            // Move: upload local -> remote, then delete local sources
            Message::MoveUpload => {
                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let items: Vec<(String, String, bool)> = self.sessions[self.active_tab].local_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (
                            format!("{}/{}", self.sessions[self.active_tab].local_path, e.name),
                            format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name),
                            e.is_dir,
                        ))
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.sessions[self.active_tab].transfer_active = true;
                        self.sessions[self.active_tab].transfer_progress = 0.0;
                        self.sessions[self.active_tab].transfer_start = Some(Instant::now());
                        self.sessions[self.active_tab].transfer_bytes = 0;
                        self.sessions[self.active_tab].transfer_total = 0;
                        self.sessions[self.active_tab].transfer_status = format!("Moving {} item(s)...", count);

                        return spawn_transfer(move |progress_tx| async move {
                            for (local_src, remote_dst, is_dir) in &items {
                                if *is_dir {
                                    conn.upload_dir(local_src, remote_dst, progress_tx.clone()).await?;
                                } else {
                                    conn.upload(local_src, remote_dst, progress_tx.clone()).await?;
                                }
                            }
                            // Delete local sources after successful upload
                            for (local_src, _, is_dir) in &items {
                                if *is_dir {
                                    std::fs::remove_dir_all(local_src)?;
                                } else {
                                    std::fs::remove_file(local_src)?;
                                }
                            }
                            Ok(format!("Moved {} item(s)", count))
                        });
                    }
                }
            }

            Message::TransferProgress(p) => {
                if p.total_size > 0 {
                    self.sessions[self.active_tab].transfer_progress = p.transferred as f32 / p.total_size as f32;
                    self.sessions[self.active_tab].transfer_bytes = p.transferred;
                    self.sessions[self.active_tab].transfer_total = p.total_size;
                }
            }

            Message::TransferComplete(msg) => {
                self.sessions[self.active_tab].transfer_active = false;
                self.sessions[self.active_tab].transfer_progress = 1.0;
                self.sessions[self.active_tab].transfer_start = None;
                self.sessions[self.active_tab].transfer_status = msg.clone();
                self.sessions[self.active_tab].status = msg.clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.sessions[self.active_tab].transfer_log.push(TransferLogEntry {
                    message: msg,
                    is_error: false,
                    timestamp: format_timestamp(now),
                });

                // Refresh both panels
                self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                let mut tasks: Vec<Task<Message>> = Vec::new();

                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let path = self.sessions[self.active_tab].remote_path.clone();
                    tasks.push(Task::perform(
                        async move {
                            let entries = conn.list_dir(&path).await?;
                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                        },
                        |result| match result {
                            Ok((entries, path)) => Message::RemoteListing(entries, path),
                            Err(e) => Message::RemoteListingFailed(e.to_string()),
                        },
                    ));
                }

                // Process queued drop files
                if !self.sessions[self.active_tab].drop_pending.is_empty() {
                    let pending = std::mem::take(&mut self.sessions[self.active_tab].drop_pending);
                    tasks.push(Task::done(Message::FilesDropped(pending)));
                }

                if !tasks.is_empty() {
                    return Task::batch(tasks);
                }
            }

            Message::TransferFailed(err) => {
                self.sessions[self.active_tab].transfer_active = false;
                self.sessions[self.active_tab].transfer_progress = 0.0;
                self.sessions[self.active_tab].transfer_start = None;
                self.sessions[self.active_tab].transfer_status = String::new();
                let msg = format!("Transfer failed: {}", err);
                self.sessions[self.active_tab].status = msg.clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.sessions[self.active_tab].transfer_log.push(TransferLogEntry {
                    message: msg,
                    is_error: true,
                    timestamp: format_timestamp(now),
                });
            }

            Message::Disconnect => {
                let sftp = self.sessions[self.active_tab].sftp.take();
                self.sessions[self.active_tab].screen = Screen::Connect;
                self.sessions[self.active_tab].status = "Disconnecting...".to_string();
                self.sessions[self.active_tab].remote_entries.clear();
                self.sessions[self.active_tab].connected_at = None;
                self.sessions[self.active_tab].terminal_history.clear();
                self.sessions[self.active_tab].terminal_input.clear();
                self.sessions[self.active_tab].terminal_cmd_history.clear();
                self.sessions[self.active_tab].show_terminal = false;

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
                self.sessions[self.active_tab].status = "Disconnected".to_string();
            }

            Message::CycleTheme => {
                let themes = AppTheme::all();
                let idx = themes.iter().position(|t| *t == self.theme).unwrap_or(0);
                self.theme = themes[(idx + 1) % themes.len()];
                self.theme.save();
            }

            Message::OpenThemePicker => {
                self.sessions[self.active_tab].dialog = Dialog::Settings;
            }

            Message::ThemeSelected(theme) => {
                self.theme = theme;
                self.theme.save();
                self.sessions[self.active_tab].dialog = Dialog::None;
            }

            // Remote operations
            Message::RemoteMkdir => {
                if self.sessions[self.active_tab].screen == Screen::Browser {
                    self.sessions[self.active_tab].dialog = Dialog::Mkdir(String::new());
                }
            }
            Message::MkdirNameChanged(name) => {
                if let Dialog::Mkdir(ref mut n) = self.sessions[self.active_tab].dialog {
                    *n = name;
                }
            }
            Message::MkdirConfirm => {
                if let Dialog::Mkdir(ref name) = self.sessions[self.active_tab].dialog
                    && !name.is_empty()
                        && let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                            let path = format!("{}/{}", self.sessions[self.active_tab].remote_path, name);
                            let refresh_path = self.sessions[self.active_tab].remote_path.clone();
                            self.sessions[self.active_tab].dialog = Dialog::None;
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
            Message::MkdirCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::RemoteDelete => {
                if self.sessions[self.active_tab].screen == Screen::Browser {
                    let names: Vec<String> = self.sessions[self.active_tab].remote_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| e.name.clone())
                        .collect();
                    if !names.is_empty() {
                        let label = if names.len() == 1 {
                            names[0].clone()
                        } else {
                            format!("{} items", names.len())
                        };
                        self.sessions[self.active_tab].dialog = Dialog::ConfirmDelete(label);
                    }
                }
            }
            Message::DeleteConfirm => {
                if let Dialog::ConfirmDelete(_) = self.sessions[self.active_tab].dialog
                    && let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                        let items: Vec<(String, bool)> = self.sessions[self.active_tab].remote_selected.iter()
                            .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
                            .filter(|e| e.name != "..")
                            .map(|e| (format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name), e.is_dir))
                            .collect();
                        let refresh_path = self.sessions[self.active_tab].remote_path.clone();
                        self.sessions[self.active_tab].dialog = Dialog::None;
                        self.sessions[self.active_tab].remote_selected.clear();
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
            Message::DeleteCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::RemoteRename => {
                if self.sessions[self.active_tab].screen == Screen::Browser
                    && let Some(&idx) = self.sessions[self.active_tab].remote_selected.iter().next()
                        && let Some(entry) = self.sessions[self.active_tab].remote_entries.get(idx)
                            && entry.name != ".." {
                                self.sessions[self.active_tab].dialog = Dialog::Rename(entry.name.clone(), entry.name.clone());
                            }
            }
            Message::RenameNameChanged(name) => {
                if let Dialog::Rename(_, ref mut new) = self.sessions[self.active_tab].dialog {
                    *new = name;
                }
            }
            Message::RenameConfirm => {
                if let Dialog::Rename(ref old, ref new) = self.sessions[self.active_tab].dialog
                    && !new.is_empty() && old != new
                        && let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                            let from = format!("{}/{}", self.sessions[self.active_tab].remote_path, old);
                            let to = format!("{}/{}", self.sessions[self.active_tab].remote_path, new);
                            let refresh_path = self.sessions[self.active_tab].remote_path.clone();
                            self.sessions[self.active_tab].dialog = Dialog::None;
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
            Message::RenameCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::RemoteOpDone(msg) => {
                self.sessions[self.active_tab].status = msg;
            }
            Message::RemoteOpFailed(err) => {
                self.sessions[self.active_tab].status = format!("Error: {}", err);
            }

            // Local file operations
            Message::LocalMkdir => {
                if self.sessions[self.active_tab].screen == Screen::Browser {
                    self.sessions[self.active_tab].dialog = Dialog::LocalMkdir(String::new());
                }
            }
            Message::LocalMkdirNameChanged(name) => {
                if let Dialog::LocalMkdir(ref mut n) = self.sessions[self.active_tab].dialog {
                    *n = name;
                }
            }
            Message::LocalMkdirConfirm => {
                if let Dialog::LocalMkdir(ref name) = self.sessions[self.active_tab].dialog {
                    if !name.is_empty() {
                        let path = format!("{}/{}", self.sessions[self.active_tab].local_path, name);
                        match std::fs::create_dir_all(&path) {
                            Ok(_) => {
                                self.sessions[self.active_tab].status = format!("Created directory: {}", name);
                                self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                            }
                            Err(e) => {
                                self.sessions[self.active_tab].status = format!("Error: {}", e);
                            }
                        }
                    }
                    self.sessions[self.active_tab].dialog = Dialog::None;
                }
            }
            Message::LocalMkdirCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::LocalDelete => {
                if self.sessions[self.active_tab].screen == Screen::Browser {
                    let names: Vec<String> = self.sessions[self.active_tab].local_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| e.name.clone())
                        .collect();
                    if !names.is_empty() {
                        let label = if names.len() == 1 {
                            names[0].clone()
                        } else {
                            format!("{} items", names.len())
                        };
                        self.sessions[self.active_tab].dialog = Dialog::LocalConfirmDelete(label);
                    }
                }
            }
            Message::LocalDeleteConfirm => {
                if let Dialog::LocalConfirmDelete(_) = self.sessions[self.active_tab].dialog {
                    let items: Vec<(String, bool)> = self.sessions[self.active_tab].local_selected.iter()
                        .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                        .filter(|e| e.name != "..")
                        .map(|e| (format!("{}/{}", self.sessions[self.active_tab].local_path, e.name), e.is_dir))
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
                                self.sessions[self.active_tab].status = format!("Error: {}", e);
                                break;
                            }
                        }
                    }
                    if deleted > 0 {
                        self.sessions[self.active_tab].status = format!("Deleted {} item(s)", deleted);
                        self.sessions[self.active_tab].local_selected.clear();
                        self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                    }
                    self.sessions[self.active_tab].dialog = Dialog::None;
                }
            }
            Message::LocalDeleteCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::LocalRename => {
                if self.sessions[self.active_tab].screen == Screen::Browser
                    && let Some(&idx) = self.sessions[self.active_tab].local_selected.iter().next()
                        && let Some(entry) = self.sessions[self.active_tab].local_entries.get(idx)
                            && entry.name != ".." {
                                self.sessions[self.active_tab].dialog = Dialog::LocalRename(entry.name.clone(), entry.name.clone());
                            }
            }
            Message::LocalRenameNameChanged(name) => {
                if let Dialog::LocalRename(_, ref mut new) = self.sessions[self.active_tab].dialog {
                    *new = name;
                }
            }
            Message::LocalRenameConfirm => {
                if let Dialog::LocalRename(ref old, ref new) = self.sessions[self.active_tab].dialog {
                    if !new.is_empty() && old != new {
                        let from = format!("{}/{}", self.sessions[self.active_tab].local_path, old);
                        let to = format!("{}/{}", self.sessions[self.active_tab].local_path, new);
                        match std::fs::rename(&from, &to) {
                            Ok(_) => {
                                self.sessions[self.active_tab].status = format!("Renamed: {} -> {}", old, new);
                                self.sessions[self.active_tab].local_selected.clear();
                                self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                            }
                            Err(e) => {
                                self.sessions[self.active_tab].status = format!("Error: {}", e);
                            }
                        }
                    }
                    self.sessions[self.active_tab].dialog = Dialog::None;
                }
            }
            Message::LocalRenameCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }

            // Save connection as profile
            Message::SaveConnection => {
                if self.sessions[self.active_tab].screen == Screen::Browser {
                    let default_name = format!("{}@{}", self.sessions[self.active_tab].connect_state.username, self.sessions[self.active_tab].connect_state.host);
                    self.sessions[self.active_tab].dialog = Dialog::SaveProfile(default_name);
                }
            }
            Message::SaveConnectionName(name) => {
                if let Dialog::SaveProfile(ref mut n) = self.sessions[self.active_tab].dialog {
                    *n = name;
                }
            }
            Message::SaveConnectionConfirm => {
                let save_name = if let Dialog::SaveProfile(ref name) = self.sessions[self.active_tab].dialog {
                    if name.is_empty() { None } else { Some(name.clone()) }
                } else { None };
                if let Some(name) = save_name {
                    use crate::core::connection::ConnectionProfile;
                    let s = &self.sessions[self.active_tab];
                    let profile = ConnectionProfile {
                        name: name.clone(),
                        host: s.connect_state.host.clone(),
                        port: s.connect_state.port.parse().unwrap_or(22),
                        username: s.connect_state.username.clone(),
                        auth: s.connect_state.auth_method(),
                        favorite: false,
                        jump_host: s.connect_state.jump_host_config(),
                    };
                    let mut profiles = s.connect_state.profiles.clone();
                    if let Some(existing) = profiles.iter_mut().find(|p| p.name == name) {
                        *existing = profile;
                    } else {
                        profiles.push(profile);
                    }
                    let _ = ConnectionProfile::save_all(&profiles);
                    self.sessions[self.active_tab].connect_state.profiles = profiles;
                    self.sessions[self.active_tab].status = format!("Saved connection \"{}\"", name);
                }
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::SaveConnectionCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }

            Message::RemoteChmod => {
                if let Some(&idx) = self.sessions[self.active_tab].remote_selected.iter().next()
                    && let Some(entry) = self.sessions[self.active_tab].remote_entries.get(idx)
                        && entry.name != ".." {
                            let path = format!("{}/{}", self.sessions[self.active_tab].remote_path, entry.name);
                            // Parse current permissions to octal
                            let octal = if !entry.permissions.is_empty() {
                                let mut mode = 0u32;
                                let chars: Vec<char> = entry.permissions.chars().collect();
                                let flags = [0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001];
                                for (i, &flag) in flags.iter().enumerate() {
                                    if chars.get(i).is_some_and(|&c| c != '-') {
                                        mode |= flag;
                                    }
                                }
                                format!("{:o}", mode)
                            } else {
                                "644".to_string()
                            };
                            self.sessions[self.active_tab].dialog = Dialog::Chmod(path, octal);
                        }
            }

            Message::ChmodValueChanged(val) => {
                if let Dialog::Chmod(ref _path, ref mut octal) = self.sessions[self.active_tab].dialog {
                    *octal = val;
                }
            }

            Message::ChmodConfirm => {
                let (path, octal) = match &self.sessions[self.active_tab].dialog {
                    Dialog::Chmod(p, o) => (p.clone(), o.clone()),
                    _ => { return Task::none(); }
                };
                self.sessions[self.active_tab].dialog = Dialog::None;
                if let Ok(mode) = u32::from_str_radix(&octal, 8)
                    && mode <= 0o7777
                        && let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                            let remote_path = self.sessions[self.active_tab].remote_path.clone();
                            return Task::perform(
                                async move {
                                    conn.set_permissions(&path, mode).await?;
                                    let entries = conn.list_dir(&remote_path).await?;
                                    Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, remote_path))
                                },
                                move |result| match result {
                                    Ok((entries, path)) => Message::RemoteListing(entries, path),
                                    Err(e) => Message::RemoteOpFailed(e.to_string()),
                                },
                            );
                        }
                self.sessions[self.active_tab].status = format!("Invalid octal: {}", octal);
            }

            Message::ChmodCancel => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }

            Message::ToggleBookmarks => {
                self.show_bookmarks_bar = !self.show_bookmarks_bar;
            }
            Message::ToggleLog => {
                self.show_log = !self.show_log;
            }

            Message::ToggleHidden => {
                self.sessions[self.active_tab].show_hidden = !self.sessions[self.active_tab].show_hidden;
                self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                if !self.sessions[self.active_tab].show_hidden { self.sessions[self.active_tab].local_filter.clear(); }
                // Re-fetch remote listing with new filter
                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let path = self.sessions[self.active_tab].remote_path.clone();
                    let _show_hidden = self.sessions[self.active_tab].show_hidden;
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

            Message::ToggleTerminal => {
                self.sessions[self.active_tab].show_terminal = !self.sessions[self.active_tab].show_terminal;
            }

            Message::TerminalInputChanged(input) => {
                self.sessions[self.active_tab].terminal_input = input;
                self.sessions[self.active_tab].terminal_history_idx = None;
            }

            Message::TerminalSubmit => {
                let cmd = self.sessions[self.active_tab].terminal_input.trim().to_string();
                if cmd.is_empty() || self.sessions[self.active_tab].terminal_running {
                    return Task::none();
                }
                self.sessions[self.active_tab].terminal_cmd_history.push(cmd.clone());
                self.sessions[self.active_tab].terminal_history_idx = None;
                self.sessions[self.active_tab].terminal_input.clear();
                self.sessions[self.active_tab].terminal_running = true;

                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    return Task::perform(
                        async move {
                            conn.exec_command(&cmd).await
                        },
                        |result| match result {
                            Ok(output) => Message::TerminalOutput(output),
                            Err(e) => Message::TerminalError(e.to_string()),
                        },
                    );
                } else {
                    self.sessions[self.active_tab].terminal_history.push(TerminalEntry {
                        command: cmd,
                        output: "Not connected".to_string(),
                        is_error: true,
                    });
                    self.sessions[self.active_tab].terminal_running = false;
                }
            }

            Message::TerminalOutput(output) => {
                let cmd = self.sessions[self.active_tab].terminal_cmd_history.last().cloned().unwrap_or_default();
                self.sessions[self.active_tab].terminal_history.push(TerminalEntry {
                    command: cmd,
                    output,
                    is_error: false,
                });
                self.sessions[self.active_tab].terminal_running = false;
            }

            Message::TerminalError(err) => {
                let cmd = self.sessions[self.active_tab].terminal_cmd_history.last().cloned().unwrap_or_default();
                self.sessions[self.active_tab].terminal_history.push(TerminalEntry {
                    command: cmd,
                    output: err,
                    is_error: true,
                });
                self.sessions[self.active_tab].terminal_running = false;
            }

            Message::TerminalClear => {
                self.sessions[self.active_tab].terminal_history.clear();
            }

            Message::DismissContextMenu => {
                self.sessions[self.active_tab].context_menu = ContextMenu::None;
            }

            Message::ShowContextMenu(action) => {
                let ctx = self.sessions[self.active_tab].context_menu.clone();
                self.sessions[self.active_tab].context_menu = ContextMenu::None;
                match action {
                    ContextAction::Open => {
                        match ctx {
                            ContextMenu::Local(i) => {
                                if let Some(entry) = self.sessions[self.active_tab].local_entries.get(i)
                                    && entry.is_dir {
                                        let new_path = if entry.name == ".." {
                                            std::path::Path::new(&self.sessions[self.active_tab].local_path)
                                                .parent()
                                                .map(|p| p.to_string_lossy().to_string())
                                                .unwrap_or_else(|| "/".to_string())
                                        } else {
                                            format!("{}/{}", self.sessions[self.active_tab].local_path, entry.name)
                                        };
                                        self.sessions[self.active_tab].local_entries = read_local_dir(&new_path, self.sessions[self.active_tab].show_hidden);
                                        self.sessions[self.active_tab].local_path = new_path;
                                        self.sessions[self.active_tab].local_selected.clear();
                                        self.sessions[self.active_tab].last_anchor_local = None;
                                    }
                            }
                            ContextMenu::Remote(i) => {
                                if let Some(entry) = self.sessions[self.active_tab].remote_entries.get(i)
                                    && entry.is_dir {
                                        let new_path = if entry.name == ".." {
                                            parent_path(&self.sessions[self.active_tab].remote_path)
                                        } else {
                                            format!("{}/{}", self.sessions[self.active_tab].remote_path, entry.name)
                                        };
                                        self.sessions[self.active_tab].remote_path = new_path.clone();
                                        self.sessions[self.active_tab].remote_selected.clear();
                                        self.sessions[self.active_tab].last_anchor_remote = None;
                                        if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
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
                            ContextMenu::None => {}
                        }
                    }
                    ContextAction::Download => {
                        return self.update(Message::DownloadSelected);
                    }
                    ContextAction::Upload => {
                        return self.update(Message::UploadSelected);
                    }
                    ContextAction::Rename => {
                        return match ctx {
                            ContextMenu::Local(_) => self.update(Message::LocalRename),
                            ContextMenu::Remote(_) => self.update(Message::RemoteRename),
                            ContextMenu::None => Task::none(),
                        };
                    }
                    ContextAction::Delete => {
                        return match ctx {
                            ContextMenu::Local(_) => self.update(Message::LocalDelete),
                            ContextMenu::Remote(_) => self.update(Message::RemoteDelete),
                            ContextMenu::None => Task::none(),
                        };
                    }
                    ContextAction::NewDir => {
                        return match ctx {
                            ContextMenu::Local(_) => self.update(Message::LocalMkdir),
                            ContextMenu::Remote(_) => self.update(Message::RemoteMkdir),
                            ContextMenu::None => Task::none(),
                        };
                    }
                    ContextAction::CopyPath => {
                        let path = match ctx {
                            ContextMenu::Local(i) => {
                                self.sessions[self.active_tab].local_entries.get(i)
                                    .map(|e| format!("{}/{}", self.sessions[self.active_tab].local_path, e.name))
                            }
                            ContextMenu::Remote(i) => {
                                self.sessions[self.active_tab].remote_entries.get(i)
                                    .map(|e| format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name))
                            }
                            ContextMenu::None => None,
                        };
                        if let Some(p) = path {
                            self.sessions[self.active_tab].status = format!("Copied: {}", p);
                            return iced::clipboard::write(p);
                        }
                    }
                    ContextAction::Chmod => {
                        if let ContextMenu::Remote(i) = ctx {
                            self.sessions[self.active_tab].remote_selected.clear();
                            self.sessions[self.active_tab].remote_selected.insert(i);
                            return self.update(Message::RemoteChmod);
                        }
                    }
                    ContextAction::Refresh => {
                        return match ctx {
                            ContextMenu::Local(_) => {
                                self.sessions[self.active_tab].local_entries = read_local_dir(&self.sessions[self.active_tab].local_path, self.sessions[self.active_tab].show_hidden);
                                Task::none()
                            }
                            ContextMenu::Remote(_) => {
                                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                                    let path = self.sessions[self.active_tab].remote_path.clone();
                                    Task::perform(
                                        async move {
                                            let entries = conn.list_dir(&path).await?;
                                            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((entries, path))
                                        },
                                        |result| match result {
                                            Ok((entries, path)) => Message::RemoteListing(entries, path),
                                            Err(e) => Message::RemoteListingFailed(e.to_string()),
                                        },
                                    )
                                } else {
                                    Task::none()
                                }
                            }
                            ContextMenu::None => Task::none(),
                        };
                    }
                }
            }

            // First-run / Settings
            Message::FirstRunPortable => {
                self.settings.install_mode = Some(InstallMode::Portable);
                self.settings.save();
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::FirstRunInstall => {
                self.settings.install_mode = Some(InstallMode::Installed);
                self.settings.save();
                self.sessions[self.active_tab].dialog = Dialog::None;
                return Task::perform(
                    crate::core::desktop_integration::install(),
                    Message::InstallComplete,
                );
            }
            Message::OpenSettings => {
                self.sessions[self.active_tab].dialog = Dialog::Settings;
            }
            Message::CloseSettings => {
                self.sessions[self.active_tab].dialog = Dialog::None;
            }
            Message::SettingsSetInstallMode(mode) => {
                let old_mode = self.settings.install_mode;
                self.settings.install_mode = Some(mode);
                self.settings.save();
                match mode {
                    InstallMode::Installed => {
                        return Task::perform(
                            crate::core::desktop_integration::install(),
                            Message::InstallComplete,
                        );
                    }
                    InstallMode::Portable => {
                        if old_mode == Some(InstallMode::Installed) {
                            return Task::perform(
                                crate::core::desktop_integration::uninstall(),
                                Message::UninstallComplete,
                            );
                        }
                    }
                }
            }
            Message::InstallComplete(result) => {
                match result {
                    Ok(msg) => self.sessions[self.active_tab].status = msg,
                    Err(e) => self.sessions[self.active_tab].status = format!("Install failed: {e}"),
                }
            }
            Message::UninstallComplete(result) => {
                match result {
                    Ok(msg) => self.sessions[self.active_tab].status = msg,
                    Err(e) => self.sessions[self.active_tab].status = format!("Uninstall failed: {e}"),
                }
            }

            // Drag and drop: collect files, then batch-upload
            Message::DragHovering(hovering) => {
                self.sessions[self.active_tab].drop_hovering = hovering;
            }
            Message::FilesDropped(paths) => {
                self.sessions[self.active_tab].drop_hovering = false;

                // Guard: transfer already running, queue into pending
                if self.sessions[self.active_tab].transfer_active {
                    self.sessions[self.active_tab].drop_pending.extend(paths);
                    self.sessions[self.active_tab].status = format!("{} file(s) queued, waiting for current transfer...", self.sessions[self.active_tab].drop_pending.len());
                    return Task::none();
                }

                // Guard: not connected or not in browser
                if self.sessions[self.active_tab].screen != Screen::Browser || self.sessions[self.active_tab].sftp.is_none() {
                    self.sessions[self.active_tab].status = "Connect to a server before dropping files".to_string();
                    return Task::none();
                }

                // Merge any pending files with this batch
                let mut all_paths = std::mem::take(&mut self.sessions[self.active_tab].drop_pending);
                all_paths.extend(paths);

                if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
                    let items: Vec<(String, String, bool)> = all_paths.iter()
                        .filter_map(|p| {
                            let name = p.file_name()?.to_string_lossy().to_string();
                            let is_dir = p.is_dir();
                            let local_src = p.to_string_lossy().to_string();
                            let remote_dst = format!("{}/{}", self.sessions[self.active_tab].remote_path, name);
                            Some((local_src, remote_dst, is_dir))
                        })
                        .collect();

                    if !items.is_empty() {
                        let count = items.len();
                        self.sessions[self.active_tab].transfer_active = true;
                        self.sessions[self.active_tab].transfer_progress = 0.0;
                        self.sessions[self.active_tab].transfer_start = Some(Instant::now());
                        self.sessions[self.active_tab].transfer_bytes = 0;
                        self.sessions[self.active_tab].transfer_total = 0;
                        self.sessions[self.active_tab].transfer_status = format!("Uploading {} dropped item(s)...", count);

                        return spawn_transfer(move |progress_tx| async move {
                            for (local_src, remote_dst, is_dir) in &items {
                                if *is_dir {
                                    conn.upload_dir(local_src, remote_dst, progress_tx.clone()).await?;
                                } else {
                                    conn.upload(local_src, remote_dst, progress_tx.clone()).await?;
                                }
                            }
                            Ok(format!("Uploaded {} dropped item(s)", count))
                        });
                    }
                }
            }
            Message::BrowseUpload => {
                if self.sessions[self.active_tab].screen != Screen::Browser || self.sessions[self.active_tab].sftp.is_none() {
                    self.sessions[self.active_tab].status = "Connect to a server before uploading files".to_string();
                    return Task::none();
                }
                return Task::perform(
                    async {
                        let handles = rfd::AsyncFileDialog::new()
                            .set_title("Select files to upload")
                            .pick_files()
                            .await;
                        handles
                            .unwrap_or_default()
                            .into_iter()
                            .map(|h| h.path().to_path_buf())
                            .collect::<Vec<_>>()
                    },
                    Message::BrowseUploadChosen,
                );
            }
            Message::BrowseUploadChosen(paths) => {
                if !paths.is_empty() {
                    return self.update(Message::FilesDropped(paths));
                }
            }

            // Keyboard shortcuts
            Message::KeyPressed(key, mods) => {
                self.modifiers = mods;
                if self.sessions[self.active_tab].context_menu != ContextMenu::None {
                    self.sessions[self.active_tab].context_menu = ContextMenu::None;
                    if matches!(key, Key::Named(keyboard::key::Named::Escape)) {
                        return Task::none();
                    }
                }
                if self.sessions[self.active_tab].dialog != Dialog::None {
                    if matches!(key, Key::Named(keyboard::key::Named::Escape)) {
                        self.sessions[self.active_tab].dialog = Dialog::None;
                    }
                    return Task::none();
                }
                if self.sessions[self.active_tab].screen != Screen::Browser {
                    return Task::none();
                }
                match key {
                    // Ctrl+A: Select All
                    Key::Character(ref c) if c.as_str() == "a" && mods.control() => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                self.sessions[self.active_tab].local_selected.clear();
                                for i in 1..self.sessions[self.active_tab].local_entries.len() {
                                    self.sessions[self.active_tab].local_selected.insert(i);
                                }
                            }
                            ActivePanel::Remote => {
                                self.sessions[self.active_tab].remote_selected.clear();
                                for i in 1..self.sessions[self.active_tab].remote_entries.len() {
                                    self.sessions[self.active_tab].remote_selected.insert(i);
                                }
                            }
                        }
                    }
                    // Ctrl+I: Invert Selection
                    Key::Character(ref c) if c.as_str() == "i" && mods.control() => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                let mut new_sel = BTreeSet::new();
                                for i in 1..self.sessions[self.active_tab].local_entries.len() {
                                    if !self.sessions[self.active_tab].local_selected.contains(&i) {
                                        new_sel.insert(i);
                                    }
                                }
                                self.sessions[self.active_tab].local_selected = new_sel;
                            }
                            ActivePanel::Remote => {
                                let mut new_sel = BTreeSet::new();
                                for i in 1..self.sessions[self.active_tab].remote_entries.len() {
                                    if !self.sessions[self.active_tab].remote_selected.contains(&i) {
                                        new_sel.insert(i);
                                    }
                                }
                                self.sessions[self.active_tab].remote_selected = new_sel;
                            }
                        }
                    }
                    // Ctrl+C: Copy selected paths to clipboard
                    Key::Character(ref c) if c.as_str() == "c" && mods.control() => {
                        let paths: Vec<String> = match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                self.sessions[self.active_tab].local_selected.iter()
                                    .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                                    .filter(|e| e.name != "..")
                                    .map(|e| format!("{}/{}", self.sessions[self.active_tab].local_path, e.name))
                                    .collect()
                            }
                            ActivePanel::Remote => {
                                self.sessions[self.active_tab].remote_selected.iter()
                                    .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
                                    .filter(|e| e.name != "..")
                                    .map(|e| format!("{}/{}", self.sessions[self.active_tab].remote_path, e.name))
                                    .collect()
                            }
                        };
                        if !paths.is_empty() {
                            let text = paths.join("\n");
                            self.sessions[self.active_tab].status = format!("Copied {} path(s)", paths.len());
                            return iced::clipboard::write(text);
                        }
                    }
                    // Ctrl+H: Toggle hidden
                    Key::Character(ref c) if c.as_str() == "h" && mods.control() => {
                        return self.update(Message::ToggleHidden);
                    }
                    // Ctrl+B: Toggle bookmarks bar
                    Key::Character(ref c) if c.as_str() == "b" && mods.control() => {
                        return self.update(Message::ToggleBookmarks);
                    }
                    // F2: Rename
                    Key::Named(keyboard::key::Named::F2) => {
                        return match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => self.update(Message::LocalRename),
                            ActivePanel::Remote => self.update(Message::RemoteRename),
                        };
                    }
                    // F4: Toggle terminal
                    Key::Named(keyboard::key::Named::F4) => {
                        return self.update(Message::ToggleTerminal);
                    }
                    // F5: Copy selected to other panel (Norton Commander style)
                    Key::Named(keyboard::key::Named::F5) if !mods.control() => {
                        return match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => self.update(Message::UploadSelected),
                            ActivePanel::Remote => self.update(Message::DownloadSelected),
                        };
                    }
                    // F6: Move selected to other panel (copy + delete source)
                    Key::Named(keyboard::key::Named::F6) => {
                        return match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => self.update(Message::MoveUpload),
                            ActivePanel::Remote => self.update(Message::MoveDownload),
                        };
                    }
                    // F7: New directory
                    Key::Named(keyboard::key::Named::F7) => {
                        return match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => self.update(Message::LocalMkdir),
                            ActivePanel::Remote => self.update(Message::RemoteMkdir),
                        };
                    }
                    // F8 / Delete: Delete selected
                    Key::Named(keyboard::key::Named::Delete) | Key::Named(keyboard::key::Named::F8) => {
                        return match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => self.update(Message::LocalDelete),
                            ActivePanel::Remote => self.update(Message::RemoteDelete),
                        };
                    }
                    // Arrow Down (with Shift for extend selection)
                    Key::Named(keyboard::key::Named::ArrowDown) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                let max = self.sessions[self.active_tab].local_entries.len().saturating_sub(1);
                                let current = self.sessions[self.active_tab].last_anchor_local.or_else(|| self.sessions[self.active_tab].local_selected.iter().next().copied()).unwrap_or(0);
                                let next = (current + 1).min(max);
                                if mods.shift() {
                                    self.sessions[self.active_tab].local_selected.insert(next);
                                } else {
                                    self.sessions[self.active_tab].local_selected.clear();
                                    self.sessions[self.active_tab].local_selected.insert(next);
                                }
                                self.sessions[self.active_tab].last_anchor_local = Some(next);
                            }
                            ActivePanel::Remote => {
                                let max = self.sessions[self.active_tab].remote_entries.len().saturating_sub(1);
                                let current = self.sessions[self.active_tab].last_anchor_remote.or_else(|| self.sessions[self.active_tab].remote_selected.iter().next().copied()).unwrap_or(0);
                                let next = (current + 1).min(max);
                                if mods.shift() {
                                    self.sessions[self.active_tab].remote_selected.insert(next);
                                } else {
                                    self.sessions[self.active_tab].remote_selected.clear();
                                    self.sessions[self.active_tab].remote_selected.insert(next);
                                }
                                self.sessions[self.active_tab].last_anchor_remote = Some(next);
                            }
                        }
                    }
                    // Arrow Up (with Shift for extend selection)
                    Key::Named(keyboard::key::Named::ArrowUp) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                let current = self.sessions[self.active_tab].last_anchor_local.or_else(|| self.sessions[self.active_tab].local_selected.iter().next().copied()).unwrap_or(0);
                                let next = current.saturating_sub(1);
                                if mods.shift() {
                                    self.sessions[self.active_tab].local_selected.insert(next);
                                } else {
                                    self.sessions[self.active_tab].local_selected.clear();
                                    self.sessions[self.active_tab].local_selected.insert(next);
                                }
                                self.sessions[self.active_tab].last_anchor_local = Some(next);
                            }
                            ActivePanel::Remote => {
                                let current = self.sessions[self.active_tab].last_anchor_remote.or_else(|| self.sessions[self.active_tab].remote_selected.iter().next().copied()).unwrap_or(0);
                                let next = current.saturating_sub(1);
                                if mods.shift() {
                                    self.sessions[self.active_tab].remote_selected.insert(next);
                                } else {
                                    self.sessions[self.active_tab].remote_selected.clear();
                                    self.sessions[self.active_tab].remote_selected.insert(next);
                                }
                                self.sessions[self.active_tab].last_anchor_remote = Some(next);
                            }
                        }
                    }
                    // Home: Jump to first entry
                    Key::Named(keyboard::key::Named::Home) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                self.sessions[self.active_tab].local_selected.clear();
                                if !self.sessions[self.active_tab].local_entries.is_empty() {
                                    self.sessions[self.active_tab].local_selected.insert(0);
                                    self.sessions[self.active_tab].last_anchor_local = Some(0);
                                }
                            }
                            ActivePanel::Remote => {
                                self.sessions[self.active_tab].remote_selected.clear();
                                if !self.sessions[self.active_tab].remote_entries.is_empty() {
                                    self.sessions[self.active_tab].remote_selected.insert(0);
                                    self.sessions[self.active_tab].last_anchor_remote = Some(0);
                                }
                            }
                        }
                    }
                    // End: Jump to last entry
                    Key::Named(keyboard::key::Named::End) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                self.sessions[self.active_tab].local_selected.clear();
                                let last = self.sessions[self.active_tab].local_entries.len().saturating_sub(1);
                                self.sessions[self.active_tab].local_selected.insert(last);
                                self.sessions[self.active_tab].last_anchor_local = Some(last);
                            }
                            ActivePanel::Remote => {
                                self.sessions[self.active_tab].remote_selected.clear();
                                let last = self.sessions[self.active_tab].remote_entries.len().saturating_sub(1);
                                self.sessions[self.active_tab].remote_selected.insert(last);
                                self.sessions[self.active_tab].last_anchor_remote = Some(last);
                            }
                        }
                    }
                    // Enter: Open directory or trigger transfer for files
                    Key::Named(keyboard::key::Named::Enter) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                if let Some(&idx) = self.sessions[self.active_tab].local_selected.iter().next()
                                    && let Some(entry) = self.sessions[self.active_tab].local_entries.get(idx) {
                                        if entry.is_dir {
                                            let new_path = if entry.name == ".." {
                                                std::path::Path::new(&self.sessions[self.active_tab].local_path)
                                                    .parent()
                                                    .map(|p| p.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| "/".to_string())
                                            } else {
                                                format!("{}/{}", self.sessions[self.active_tab].local_path, entry.name)
                                            };
                                            self.sessions[self.active_tab].local_entries = read_local_dir(&new_path, self.sessions[self.active_tab].show_hidden);
                                            self.sessions[self.active_tab].local_path = new_path;
                                            self.sessions[self.active_tab].local_selected.clear();
                                            self.sessions[self.active_tab].last_anchor_local = None;
                                        } else {
                                            return self.update(Message::UploadSelected);
                                        }
                                    }
                            }
                            ActivePanel::Remote => {
                                if let Some(&idx) = self.sessions[self.active_tab].remote_selected.iter().next()
                                    && let Some(entry) = self.sessions[self.active_tab].remote_entries.get(idx) {
                                        if entry.is_dir {
                                            let new_path = if entry.name == ".." {
                                                parent_path(&self.sessions[self.active_tab].remote_path)
                                            } else {
                                                format!("{}/{}", self.sessions[self.active_tab].remote_path, entry.name)
                                            };
                                            self.sessions[self.active_tab].remote_path = new_path.clone();
                                            self.sessions[self.active_tab].remote_selected.clear();
                                            self.sessions[self.active_tab].last_anchor_remote = None;
                                            if let Some(conn) = self.sessions[self.active_tab].sftp.clone() {
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
                                            return self.update(Message::DownloadSelected);
                                        }
                                    }
                            }
                        }
                    }
                    // Ctrl+Tab: Next tab, Ctrl+Shift+Tab: Previous tab
                    Key::Named(keyboard::key::Named::Tab) if mods.control() => {
                        if mods.shift() {
                            return self.update(Message::PrevTab);
                        } else {
                            return self.update(Message::NextTab);
                        }
                    }
                    // Tab: Switch active panel
                    Key::Named(keyboard::key::Named::Tab) => {
                        self.sessions[self.active_tab].active_panel = match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => ActivePanel::Remote,
                            ActivePanel::Remote => ActivePanel::Local,
                        };
                    }
                    // Backspace: Go up directory
                    Key::Named(keyboard::key::Named::Backspace) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                return self.update(Message::LocalPanel(PanelMessage::GoUp));
                            }
                            ActivePanel::Remote => {
                                return self.update(Message::RemotePanel(PanelMessage::GoUp));
                            }
                        }
                    }
                    // Space: Toggle selection on current entry (Commander-style)
                    Key::Named(keyboard::key::Named::Space) => {
                        match self.sessions[self.active_tab].active_panel {
                            ActivePanel::Local => {
                                if let Some(anchor) = self.sessions[self.active_tab].last_anchor_local {
                                    if self.sessions[self.active_tab].local_selected.contains(&anchor) {
                                        self.sessions[self.active_tab].local_selected.remove(&anchor);
                                    } else {
                                        self.sessions[self.active_tab].local_selected.insert(anchor);
                                    }
                                    // Move down after toggle
                                    let max = self.sessions[self.active_tab].local_entries.len().saturating_sub(1);
                                    let next = (anchor + 1).min(max);
                                    self.sessions[self.active_tab].last_anchor_local = Some(next);
                                    self.sessions[self.active_tab].local_selected.insert(next);
                                }
                            }
                            ActivePanel::Remote => {
                                if let Some(anchor) = self.sessions[self.active_tab].last_anchor_remote {
                                    if self.sessions[self.active_tab].remote_selected.contains(&anchor) {
                                        self.sessions[self.active_tab].remote_selected.remove(&anchor);
                                    } else {
                                        self.sessions[self.active_tab].remote_selected.insert(anchor);
                                    }
                                    let max = self.sessions[self.active_tab].remote_entries.len().saturating_sub(1);
                                    let next = (anchor + 1).min(max);
                                    self.sessions[self.active_tab].last_anchor_remote = Some(next);
                                    self.sessions[self.active_tab].remote_selected.insert(next);
                                }
                            }
                        }
                    }
                    // Ctrl+T: New tab
                    Key::Character(ref c) if c.as_str() == "t" && mods.control() => {
                        return self.update(Message::NewTab);
                    }
                    // Ctrl+W: Close current tab
                    Key::Character(ref c) if c.as_str() == "w" && mods.control() => {
                        let idx = self.active_tab;
                        return self.update(Message::CloseTab(idx));
                    }
                    // Ctrl+1-9: Switch to tab N
                    Key::Character(ref c) if mods.control() => {
                        if let Ok(n) = c.parse::<usize>()
                            && n >= 1 && n <= self.sessions.len() {
                                return self.update(Message::SwitchTab(n - 1));
                            }
                    }
                    _ => {}
                }
            }

            // Tab management
            Message::NewTab => {
                let id = self.next_session_id;
                self.next_session_id += 1;
                self.sessions.push(Session::new(id));
                self.active_tab = self.sessions.len() - 1;
            }
            Message::CloseTab(idx) => {
                if idx < self.sessions.len() {
                    let session = self.sessions.remove(idx);
                    // Gracefully close SFTP connection if active
                    if let Some(conn) = session.sftp {
                        tokio::spawn(async move { let _ = conn.close().await; });
                    }
                    if self.sessions.is_empty() {
                        // Open a fresh tab if all closed
                        let id = self.next_session_id;
                        self.next_session_id += 1;
                        self.sessions.push(Session::new(id));
                        self.active_tab = 0;
                    } else if self.active_tab >= self.sessions.len() {
                        self.active_tab = self.sessions.len() - 1;
                    }
                }
            }
            Message::SwitchTab(idx) => {
                if idx < self.sessions.len() {
                    self.active_tab = idx;
                }
            }
            Message::NextTab => {
                if !self.sessions.is_empty() {
                    self.active_tab = (self.active_tab + 1) % self.sessions.len();
                }
            }
            Message::PrevTab => {
                if !self.sessions.is_empty() {
                    self.active_tab = if self.active_tab == 0 {
                        self.sessions.len() - 1
                    } else {
                        self.active_tab - 1
                    };
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let is_browser = self.sessions[self.active_tab].screen == Screen::Browser;

        // Toolbar: full when connected, minimal otherwise
        let toolbar = if is_browser {
            let uptime_text = self.sessions[self.active_tab].connected_at
                .map(|t| {
                    let secs = t.elapsed().as_secs();
                    let h = secs / 3600;
                    let m = (secs % 3600) / 60;
                    let s = secs % 60;
                    if h > 0 { format!("{}h {:02}m", h, m) } else { format!("{}m {:02}s", m, s) }
                })
                .unwrap_or_default();
            container(
                row![
                    text(&self.sessions[self.active_tab].status).size(13),
                    text(uptime_text).size(11),
                    iced::widget::horizontal_space(),
                    button(text(if self.sessions[self.active_tab].show_hidden { "Hide .*" } else { "Show .*" }).size(12))
                        .on_press(Message::ToggleHidden).padding([4, 8]),
                    button(text("Save").size(12))
                        .on_press(Message::SaveConnection).padding([4, 8]),
                    button(row![icons::terminal(12.0), text(if self.sessions[self.active_tab].show_terminal { " Terminal [x]" } else { " Terminal" }).size(12)].align_y(iced::Alignment::Center))
                        .on_press(Message::ToggleTerminal).padding([4, 8]),
                    button(text(format!("Log ({})", self.sessions[self.active_tab].transfer_log.len())).size(12))
                        .on_press(Message::ToggleLog).padding([4, 8]),
                    button(row![icons::settings(12.0), text(" Settings").size(12)].align_y(iced::Alignment::Center))
                        .on_press(Message::OpenSettings).padding([4, 8]),
                    button(text("Disconnect").size(12))
                        .on_press(Message::Disconnect).padding([4, 8]),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
        } else {
            container(
                row![
                    text(&self.sessions[self.active_tab].status).size(13),
                    iced::widget::horizontal_space(),
                    button(row![icons::settings(12.0), text(" Settings").size(12)].align_y(iced::Alignment::Center))
                        .on_press(Message::OpenSettings).padding([4, 8]),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
        }
        .padding(6)
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(iced::Background::Color(iced::Color {
                a: 0.3,
                ..theme.extended_palette().background.strong.color
            })),
            ..Default::default()
        });

        // Bookmarks bar: always visible
        let bookmarks_bar: Element<'_, Message> = if self.show_bookmarks_bar {
            let mut bookmarks: Vec<_> = self.sessions[self.active_tab].connect_state.profiles.iter().filter(|p| p.favorite).collect();
            bookmarks.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            if bookmarks.is_empty() {
                container(text("No bookmarks yet. Star a saved connection.").size(12)).width(Length::Fill).padding([4, 12]).into()
            } else {
                let chips: Vec<Element<'_, Message>> = bookmarks.into_iter().map(|p| {
                    let name = p.name.clone();
                    button(text(format!("★ {}", p.name)).size(12))
                        .on_press(Message::Connect(ConnectMessage::QuickConnect(name)))
                        .padding([4, 10])
                        .into()
                }).collect();
                container(row(chips).spacing(6).align_y(iced::Alignment::Center)).width(Length::Fill).padding([4, 8]).into()
            }
        } else {
            container(text("")).height(Length::Shrink).into()
        };

        // Local panel: always visible on the left
        let local_panel = file_panel::view_panel(
            "Local",
            &self.sessions[self.active_tab].local_path,
            &self.sessions[self.active_tab].local_entries,
            &self.sessions[self.active_tab].local_selected,
            &self.sessions[self.active_tab].local_filter,
            self.sessions[self.active_tab].active_panel == ActivePanel::Local,
            self.sessions[self.active_tab].sort_column,
            self.sessions[self.active_tab].sort_ascending,
        )
        .map(Message::LocalPanel);

        // Right panel: connect dialog, spinner, or remote browser
        let right_panel: Element<'_, Message> = match self.sessions[self.active_tab].screen {
            Screen::Connect => {
                let form = connect_dialog::view_connect_dialog(&self.sessions[self.active_tab].connect_state)
                    .map(Message::Connect);
                if self.sessions[self.active_tab].status.starts_with("Failed") {
                    container(
                        column![
                            text(&self.sessions[self.active_tab].status).size(13).color(iced::Color::from_rgb(0.9, 0.2, 0.2)),
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
                    column![text(&self.sessions[self.active_tab].status).size(16)].spacing(12),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
            }
            Screen::Browser => {
                // Transfer buttons between panels
                let can_upload = !self.sessions[self.active_tab].local_selected.is_empty()
                    && self.sessions[self.active_tab].local_selected.iter()
                        .any(|&idx| self.sessions[self.active_tab].local_entries.get(idx).is_some_and(|e| e.name != ".."))
                    && !self.sessions[self.active_tab].transfer_active;
                let can_download = !self.sessions[self.active_tab].remote_selected.is_empty()
                    && self.sessions[self.active_tab].remote_selected.iter()
                        .any(|&idx| self.sessions[self.active_tab].remote_entries.get(idx).is_some_and(|e| e.name != ".."))
                    && !self.sessions[self.active_tab].transfer_active;

                let mut upload_btn = button(row![icons::upload(12.0), text(" Upload").size(12)].align_y(iced::Alignment::Center)).padding([6, 14]);
                if can_upload { upload_btn = upload_btn.on_press(Message::UploadSelected); }

                let mut download_btn = button(row![icons::download(12.0), text(" Download").size(12)].align_y(iced::Alignment::Center)).padding([6, 14]);
                if can_download { download_btn = download_btn.on_press(Message::DownloadSelected); }

                let mut browse_btn = button(
                    row![icons::folder(12.0), text(" Browse...").size(12)].align_y(iced::Alignment::Center)
                ).padding([6, 14]);
                if !self.sessions[self.active_tab].transfer_active {
                    browse_btn = browse_btn.on_press(Message::BrowseUpload);
                }

                let transfer_controls = container(
                    column![upload_btn, browse_btn, download_btn]
                        .spacing(12)
                        .align_x(iced::Alignment::Center),
                )
                .center_y(Length::Fill)
                .width(Length::Shrink)
                .padding([8, 12]);

                row![
                    transfer_controls,
                    file_panel::view_panel(
                        "Remote",
                        &self.sessions[self.active_tab].remote_path,
                        &self.sessions[self.active_tab].remote_entries,
                        &self.sessions[self.active_tab].remote_selected,
                        &self.sessions[self.active_tab].remote_filter,
                        self.sessions[self.active_tab].active_panel == ActivePanel::Remote,
                        self.sessions[self.active_tab].sort_column,
                        self.sessions[self.active_tab].sort_ascending,
                    )
                    .map(Message::RemotePanel),
                ]
                .spacing(4)
                .into()
            }
        };

        // Dual-pane row: local always on left, right side changes
        let panels = row![local_panel, right_panel].spacing(4);

        // Transfer progress bar
                let transfer_bar: Element<'_, Message> = if self.sessions[self.active_tab].transfer_active {
                    let speed_text = self.sessions[self.active_tab].transfer_start
                        .map(|start| {
                            let elapsed = start.elapsed().as_secs_f64();
                            if elapsed > 0.1 {
                                let speed = self.sessions[self.active_tab].transfer_bytes as f64 / elapsed;
                                format!(
                                    "{} / {} ({}/s)",
                                    format_size(self.sessions[self.active_tab].transfer_bytes),
                                    format_size(self.sessions[self.active_tab].transfer_total),
                                    format_size(speed as u64),
                                )
                            } else {
                                self.sessions[self.active_tab].transfer_status.clone()
                            }
                        })
                        .unwrap_or_else(|| self.sessions[self.active_tab].transfer_status.clone());
                    container(
                        column![
                            text(speed_text).size(12),
                            progress_bar(0.0..=1.0, self.sessions[self.active_tab].transfer_progress)
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
                let log_entries = &self.sessions[self.active_tab].transfer_log;
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

                // Terminal panel
                let term_history = &self.sessions[self.active_tab].terminal_history;
                let show_terminal = self.sessions[self.active_tab].show_terminal;
                let terminal_input_val = self.sessions[self.active_tab].terminal_input.clone();
                let terminal_running = self.sessions[self.active_tab].terminal_running;
                let make_terminal_panel = move || -> Element<'_, Message> {
                    if !show_terminal {
                        return container(text("")).height(Length::Shrink).width(Length::Fill).into();
                    }

                    let mut output_rows: Vec<Element<'_, Message>> = Vec::new();
                    for entry in term_history.iter() {
                        output_rows.push(
                            text(format!("$ {}", entry.command))
                                .size(12)
                                .color(iced::Color::from_rgb(0.4, 0.8, 1.0))
                                .into(),
                        );
                        if !entry.output.is_empty() {
                            let color = if entry.is_error {
                                iced::Color::from_rgb(0.9, 0.3, 0.3)
                            } else {
                                iced::Color::from_rgb(0.8, 0.8, 0.8)
                            };
                            output_rows.push(
                                text(&entry.output).size(11).color(color).into(),
                            );
                        }
                    }

                    let output_scroll = scrollable(
                        column(output_rows).spacing(2).padding(6),
                    )
                    .height(Length::Fixed(150.0));

                    let prompt = if terminal_running { "Running..." } else { "$ " };
                    let mut input = text_input(prompt, &terminal_input_val)
                        .on_input(Message::TerminalInputChanged)
                        .padding(4)
                        .size(12)
                        .width(Length::Fill);
                    if !terminal_running {
                        input = input.on_submit(Message::TerminalSubmit);
                    }

                    let input_row = row![
                        input,
                        button(text("Clear").size(11))
                            .on_press(Message::TerminalClear)
                            .padding([4, 8]),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center);

                    container(
                        column![
                            text("SSH Terminal").size(12),
                            output_scroll,
                            input_row,
                        ]
                        .spacing(4)
                        .padding(6),
                    )
                    .width(Length::Fill)
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
                };

                // Status bar
                let local_count = self.sessions[self.active_tab].local_entries.len().saturating_sub(1);
                let remote_count = self.sessions[self.active_tab].remote_entries.len().saturating_sub(1);
                let local_sel = self.sessions[self.active_tab].local_selected.len();
                let remote_sel = self.sessions[self.active_tab].remote_selected.len();
                let local_sel_size: u64 = self.sessions[self.active_tab].local_selected.iter()
                    .filter_map(|&idx| self.sessions[self.active_tab].local_entries.get(idx))
                    .filter(|e| !e.is_dir && e.name != "..")
                    .map(|e| e.size)
                    .sum();
                let remote_sel_size: u64 = self.sessions[self.active_tab].remote_selected.iter()
                    .filter_map(|&idx| self.sessions[self.active_tab].remote_entries.get(idx))
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
                let active = &self.sessions[self.active_tab].active_panel;
                let make_fkey_bar = || -> Element<'_, Message> {
                    container(
                        row![
                            button(text("F2 Rename").size(11))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalRename,
                                    ActivePanel::Remote => Message::RemoteRename,
                                })
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                            button(text("F4 Term").size(11))
                                .on_press(Message::ToggleTerminal)
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                            button(text("F5 Copy").size(11))
                                .on_press(match active {
                                    ActivePanel::Local => Message::UploadSelected,
                                    ActivePanel::Remote => Message::DownloadSelected,
                                })
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                            button(text("F6 Move").size(11))
                                .on_press(match active {
                                    ActivePanel::Local => Message::MoveUpload,
                                    ActivePanel::Remote => Message::MoveDownload,
                                })
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                            button(text("F7 Mkdir").size(11))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalMkdir,
                                    ActivePanel::Remote => Message::RemoteMkdir,
                                })
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                            button(text("F8 Delete").size(11))
                                .on_press(match active {
                                    ActivePanel::Local => Message::LocalDelete,
                                    ActivePanel::Remote => Message::RemoteDelete,
                                })
                                .padding([4, 8])
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(2),
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

                // Tab bar
                let tab_bar: Element<'_, Message> = if self.sessions.len() > 1 {
                    let mut tabs: Vec<Element<'_, Message>> = self.sessions.iter().enumerate().map(|(i, session)| {
                        let is_active = i == self.active_tab;
                        let label = &session.label;
                        let has_transfer = session.transfer_active;

                        let tab_text = if has_transfer {
                            format!("{} \u{21BB}", label)
                        } else {
                            label.clone()
                        };

                        let close_btn: Element<'_, Message> = button(icons::x(10.0))
                            .on_press(Message::CloseTab(i))
                            .padding([2, 4])
                            .into();

                        let tab_content = row![
                            text(tab_text).size(12),
                            close_btn,
                        ].spacing(6).align_y(iced::Alignment::Center);

                        let tab_btn = button(tab_content)
                            .on_press(Message::SwitchTab(i))
                            .padding([5, 10])
                            .style(move |theme: &Theme, status| {
                                let mut style = button::Style::default();
                                if is_active {
                                    style.background = Some(iced::Background::Color(iced::Color {
                                        a: 0.4,
                                        ..theme.extended_palette().primary.strong.color
                                    }));
                                    style.border = iced::Border {
                                        color: theme.extended_palette().primary.strong.color,
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    };
                                } else {
                                    style.background = Some(iced::Background::Color(iced::Color {
                                        a: 0.15,
                                        ..theme.extended_palette().background.strong.color
                                    }));
                                    style.border = iced::Border {
                                        radius: 4.0.into(),
                                        ..Default::default()
                                    };
                                    if matches!(status, button::Status::Hovered) {
                                        style.background = Some(iced::Background::Color(iced::Color {
                                            a: 0.25,
                                            ..theme.extended_palette().background.strong.color
                                        }));
                                    }
                                }
                                style
                            });

                        tab_btn.into()
                    }).collect();

                    tabs.push(
                        button(text("+").size(14))
                            .on_press(Message::NewTab)
                            .padding([4, 10])
                            .into()
                    );

                    container(
                        scrollable(row(tabs).spacing(2).align_y(iced::Alignment::End))
                            .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new())),
                    )
                    .width(Length::Fill)
                    .padding([2, 6])
                    .style(|theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            a: 0.15,
                            ..theme.extended_palette().background.strong.color
                        })),
                        ..Default::default()
                    })
                    .into()
                } else {
                    container(text("")).height(Length::Shrink).into()
                };

                let base_layout = || -> Element<'_, Message> {
                    column![toolbar, tab_bar, bookmarks_bar, horizontal_rule(1), panels, transfer_bar, make_log_panel(), make_terminal_panel(), horizontal_rule(1), status_bar, make_fkey_bar()]
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

                // Global dialogs (work on any screen)
                let content: Element<'_, Message> = match &self.sessions[self.active_tab].dialog {
                    Dialog::FirstRun => {
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("Welcome to Open SFTP").size(20),
                                text("How would you like to run the app?").size(13),
                                container(
                                    column![
                                        button(
                                            column![
                                                text("Run Portable").size(14),
                                                text("Just run the binary directly. No system integration.").size(11),
                                            ].spacing(2)
                                        )
                                        .on_press(Message::FirstRunPortable)
                                        .padding([12, 16])
                                        .width(Length::Fill),
                                        button(
                                            column![
                                                text("Add to Applications").size(14),
                                                text("Create a desktop shortcut and menu entry for easy access.").size(11),
                                            ].spacing(2)
                                        )
                                        .on_press(Message::FirstRunInstall)
                                        .padding([12, 16])
                                        .width(Length::Fill),
                                    ].spacing(8)
                                ).width(Length::Fill),
                                text("You can change this later in Settings.").size(11),
                            ].spacing(16).max_width(400).into()),
                        ].into()
                    }
                    Dialog::Settings => {
                        let current = self.theme;
                        let is_installed = self.settings.install_mode == Some(InstallMode::Installed);

                        let dark_themes: Vec<Element<'_, Message>> = AppTheme::all()
                            .iter()
                            .filter(|t| t.is_dark())
                            .map(|&t| {
                                let label = if t == current {
                                    format!("> {}", t.name())
                                } else {
                                    format!("  {}", t.name())
                                };
                                button(text(label).size(12))
                                    .on_press(Message::ThemeSelected(t))
                                    .padding([4, 10])
                                    .width(Length::Fill)
                                    .into()
                            })
                            .collect();

                        let light_themes: Vec<Element<'_, Message>> = AppTheme::all()
                            .iter()
                            .filter(|t| !t.is_dark())
                            .map(|&t| {
                                let label = if t == current {
                                    format!("> {}", t.name())
                                } else {
                                    format!("  {}", t.name())
                                };
                                button(text(label).size(12))
                                    .on_press(Message::ThemeSelected(t))
                                    .padding([4, 10])
                                    .width(Length::Fill)
                                    .into()
                            })
                            .collect();

                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text("Settings").size(20),

                                text("Theme").size(14),
                                row![
                                    column![
                                        text("Dark").size(12),
                                        scrollable(column(dark_themes).spacing(2)).height(Length::Fixed(220.0)),
                                    ].spacing(4).width(Length::FillPortion(1)),
                                    column![
                                        text("Light").size(12),
                                        scrollable(column(light_themes).spacing(2)).height(Length::Fixed(220.0)),
                                    ].spacing(4).width(Length::FillPortion(1)),
                                ].spacing(12),

                                horizontal_rule(1),
                                text("Installation").size(14),
                                row![
                                    button(if !is_installed { row![icons::check(12.0), text(" Portable").size(12)].align_y(iced::Alignment::Center) } else { row![text("Portable").size(12)] })
                                        .on_press(Message::SettingsSetInstallMode(InstallMode::Portable))
                                        .padding([6, 12]),
                                    button(if is_installed { row![icons::check(12.0), text(" Installed").size(12)].align_y(iced::Alignment::Center) } else { row![text("Installed").size(12)] })
                                        .on_press(Message::SettingsSetInstallMode(InstallMode::Installed))
                                        .padding([6, 12]),
                                ].spacing(8),
                                text(if is_installed {
                                    "App is registered in your system application menu."
                                } else {
                                    "Running as portable binary. No system integration."
                                }).size(11),

                                horizontal_rule(1),
                                button("Close").on_press(Message::CloseSettings).padding([6, 16]),
                            ].spacing(10).max_width(440).into()),
                        ].into()
                    }

                    // Screen-specific dialogs (only in browser mode)
                    _ if !is_browser => base_layout(),
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
                    Dialog::Chmod(path, octal) => {
                        let preview = u32::from_str_radix(octal, 8)
                            .map(format_permissions)
                            .unwrap_or_else(|_| "invalid".to_string());
                        let filename = path.rsplit('/').next().unwrap_or(path);
                        iced::widget::stack![
                            base_layout(),
                            wrap_dialog(column![
                                text(format!("Permissions: {}", filename)).size(16),
                                text(format!("Preview: {}", preview)).size(13),
                                text_input("Octal (e.g. 755)", octal)
                                    .on_input(Message::ChmodValueChanged)
                                    .on_submit(Message::ChmodConfirm)
                                    .padding(8),
                                row![
                                    button("Cancel").on_press(Message::ChmodCancel).padding(6),
                                    button("Apply").on_press(Message::ChmodConfirm).padding(6),
                                ].spacing(12),
                            ].spacing(12).max_width(300).into()),
                        ].into()
                    }
                    Dialog::ThemePicker => {
                        // Legacy: now handled by Settings dialog
                        base_layout()
                    }
                };

                // Context menu overlay
                let final_content: Element<'_, Message> = if is_browser && self.sessions[self.active_tab].context_menu != ContextMenu::None {
                    let (entry_name, is_dir, is_local) = match &self.sessions[self.active_tab].context_menu {
                        ContextMenu::Local(i) => {
                            let e = self.sessions[self.active_tab].local_entries.get(*i);
                            (
                                e.map(|e| e.name.clone()).unwrap_or_default(),
                                e.map(|e| e.is_dir).unwrap_or(false),
                                true,
                            )
                        }
                        ContextMenu::Remote(i) => {
                            let e = self.sessions[self.active_tab].remote_entries.get(*i);
                            (
                                e.map(|e| e.name.clone()).unwrap_or_default(),
                                e.map(|e| e.is_dir).unwrap_or(false),
                                false,
                            )
                        }
                        ContextMenu::None => unreachable!(),
                    };

                    let is_dotdot = entry_name == "..";
                    let icon_widget: Element<'_, Message> = if is_dir { icons::folder(13.0).into() } else { icons::file(13.0).into() };

                    let mut menu_items: Vec<Element<'_, Message>> = vec![
                        row![icon_widget, text(format!(" {}", entry_name)).size(13)].spacing(4).align_y(iced::Alignment::Center).into(),
                        horizontal_rule(1).into(),
                    ];

                    if is_dir && !is_dotdot {
                        menu_items.push(
                            button(text("Open").size(12))
                                .on_press(Message::ShowContextMenu(ContextAction::Open))
                                .padding([4, 12])
                                .width(Length::Fill)
                                .into(),
                        );
                    }

                    if !is_dotdot {
                        if is_local {
                            menu_items.push(
                                button(row![icons::upload(12.0), text(" Upload").size(12)].align_y(iced::Alignment::Center))
                                    .on_press(Message::ShowContextMenu(ContextAction::Upload))
                                    .padding([4, 12])
                                    .width(Length::Fill)
                                    .into(),
                            );
                        } else {
                            menu_items.push(
                                button(row![icons::download(12.0), text(" Download").size(12)].align_y(iced::Alignment::Center))
                                    .on_press(Message::ShowContextMenu(ContextAction::Download))
                                    .padding([4, 12])
                                    .width(Length::Fill)
                                    .into(),
                            );
                        }
                    }

                    if !is_dotdot {
                        menu_items.push(
                            button(text("Rename        F2").size(12))
                                .on_press(Message::ShowContextMenu(ContextAction::Rename))
                                .padding([4, 12])
                                .width(Length::Fill)
                                .into(),
                        );
                        menu_items.push(
                            button(text("Delete       Del").size(12))
                                .on_press(Message::ShowContextMenu(ContextAction::Delete))
                                .padding([4, 12])
                                .width(Length::Fill)
                                .into(),
                        );
                    }

                    menu_items.push(
                        button(text("New Directory F7").size(12))
                            .on_press(Message::ShowContextMenu(ContextAction::NewDir))
                            .padding([4, 12])
                            .width(Length::Fill)
                            .into(),
                    );
                    menu_items.push(
                        button(text("Refresh       F5").size(12))
                            .on_press(Message::ShowContextMenu(ContextAction::Refresh))
                            .padding([4, 12])
                            .width(Length::Fill)
                            .into(),
                    );

                    if !is_dotdot {
                        menu_items.push(horizontal_rule(1).into());
                        menu_items.push(
                            button(text("Copy Path").size(12))
                                .on_press(Message::ShowContextMenu(ContextAction::CopyPath))
                                .padding([4, 12])
                                .width(Length::Fill)
                                .into(),
                        );
                        if !is_local {
                            menu_items.push(
                                button(text("Permissions").size(12))
                                    .on_press(Message::ShowContextMenu(ContextAction::Chmod))
                                    .padding([4, 12])
                                    .width(Length::Fill)
                                    .into(),
                            );
                        }
                    }

                    let menu_card = container(
                        column(menu_items).spacing(2).width(Length::Fixed(200.0)),
                    )
                    .padding(8)
                    .style(|theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(
                            theme.extended_palette().background.base.color,
                        )),
                        border: iced::Border {
                            color: theme.extended_palette().primary.strong.color,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    });

                    let backdrop = button(
                        container("")
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::DismissContextMenu)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme: &Theme, _status| {
                        button::Style {
                            background: Some(iced::Background::Color(iced::Color {
                                r: 0.0, g: 0.0, b: 0.0, a: 0.3,
                            })),
                            ..Default::default()
                        }
                    });

                    let menu_overlay = container(menu_card)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill);

                    iced::widget::stack![
                        content,
                        backdrop,
                        menu_overlay,
                    ].into()
                } else if self.sessions[self.active_tab].drop_hovering {
                    let (label, accent) = if !is_browser || self.sessions[self.active_tab].sftp.is_none() {
                        ("Connect to a server first", iced::Color::from_rgb(0.8, 0.3, 0.3))
                    } else if self.sessions[self.active_tab].transfer_active {
                        ("Drop to queue upload", iced::Color::from_rgb(1.0, 0.7, 0.0))
                    } else {
                        ("Drop files to upload", iced::Color::from_rgb(0.0, 0.5, 1.0))
                    };

                    let drop_overlay = container(
                        column![
                            icons::upload(32.0),
                            text(label).size(16),
                        ]
                        .spacing(8)
                        .align_x(iced::Alignment::Center),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            r: accent.r, g: accent.g, b: accent.b, a: 0.15,
                        })),
                        border: iced::Border {
                            color: accent,
                            width: 2.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    });

                    iced::widget::stack![
                        content,
                        drop_overlay,
                    ].into()
                } else {
                    content
                };

        container(final_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        self.theme.to_iced_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyPressed(key, modifiers))
        });

        let drops = iced::event::listen_with(|event, _status, _id| {
            match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::FilesDropped(vec![path]))
                }
                iced::Event::Window(iced::window::Event::FileHovered(_)) => {
                    Some(Message::DragHovering(true))
                }
                iced::Event::Window(iced::window::Event::FilesHoveredLeft)
                | iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::DragHovering(false))
                }
                _ => None,
            }
        });

        Subscription::batch([keys, drops])
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

// Utility functions are in helpers.rs

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
        assert_eq!(app.sessions[0].screen, Screen::Connect);
        assert!(app.sessions[0].sftp.is_none());
        assert!(!app.sessions[0].transfer_active);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Local);
    }

    #[test]
    fn smoke_app_default_local_path_exists() {
        let app = App::default();
        assert!(!app.sessions[0].local_path.is_empty());
        assert!(std::path::Path::new(&app.sessions[0].local_path).exists());
    }

    #[test]
    fn smoke_app_default_local_entries_populated() {
        let app = App::default();
        // Should at least have ".." entry
        assert!(!app.sessions[0].local_entries.is_empty());
        assert_eq!(app.sessions[0].local_entries[0].name, "..");
        assert!(app.sessions[0].local_entries[0].is_dir);
    }

    #[test]
    fn smoke_app_default_remote_empty() {
        let app = App::default();
        assert!(app.sessions[0].remote_entries.is_empty());
        assert!(app.sessions[0].remote_path.is_empty());
        assert!(app.sessions[0].remote_selected.is_empty());
    }

    #[test]
    fn smoke_app_default_theme_is_dark() {
        let app = App::default();
        assert_eq!(app.theme, AppTheme::Dark);
    }

    #[test]
    fn smoke_app_default_filters_empty() {
        let app = App::default();
        assert!(app.sessions[0].local_filter.is_empty());
        assert!(app.sessions[0].remote_filter.is_empty());
    }

    #[test]
    fn smoke_app_default_hidden_off() {
        let app = App::default();
        assert!(!app.sessions[0].show_hidden);
        assert!(!app.show_log);
    }

    #[test]
    fn smoke_app_default_sort_name_asc() {
        let app = App::default();
        assert_eq!(app.sessions[0].sort_column, SortColumn::Name);
        assert!(app.sessions[0].sort_ascending);
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
        assert!(!app.sessions[0].show_hidden);
        let entries_before = app.sessions[0].local_entries.len();
        app.sessions[0].show_hidden = true;
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, true);
        // With hidden shown, should have >= entries as before
        assert!(app.sessions[0].local_entries.len() >= entries_before);
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
        app.sessions[0].screen = Screen::Connecting;
        app.update(Message::ConnectionFailed("timeout".into()));
        assert_eq!(app.sessions[0].screen, Screen::Connect);
        assert!(app.sessions[0].status.contains("timeout"));
    }

    #[test]
    fn smoke_disconnect_clears_remote() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![FileEntry {
            name: "test.txt".into(), is_dir: false, size: 100,
            modified: String::new(), permissions: String::new(),
        }];
        app.update(Message::Disconnect);
        assert_eq!(app.sessions[0].screen, Screen::Connect);
        assert!(app.sessions[0].remote_entries.is_empty());
    }

    #[test]
    fn smoke_disconnected_status() {
        let mut app = App::default();
        app.update(Message::Disconnected);
        assert_eq!(app.sessions[0].status, "Disconnected");
    }

    // --- Dialog state machine ---
    #[test]
    fn smoke_remote_mkdir_dialog_not_on_connect() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Connect;
        app.update(Message::RemoteMkdir);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_mkdir_dialog_on_browser() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::RemoteMkdir);
        assert!(matches!(app.sessions[0].dialog, Dialog::Mkdir(_)));
    }

    #[test]
    fn smoke_mkdir_name_changes() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Mkdir(String::new());
        app.update(Message::MkdirNameChanged("new_dir".into()));
        assert!(matches!(app.sessions[0].dialog, Dialog::Mkdir(ref n) if n == "new_dir"));
    }

    #[test]
    fn smoke_mkdir_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Mkdir("something".into());
        app.update(Message::MkdirCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_delete_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::ConfirmDelete("file.txt".into());
        app.update(Message::DeleteCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_rename_name_changes() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Rename("old.txt".into(), "old.txt".into());
        app.update(Message::RenameNameChanged("new.txt".into()));
        assert!(matches!(app.sessions[0].dialog, Dialog::Rename(_, ref n) if n == "new.txt"));
    }

    #[test]
    fn smoke_rename_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Rename("old.txt".into(), "new.txt".into());
        app.update(Message::RenameCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    // --- Local dialogs ---
    #[test]
    fn smoke_local_mkdir_dialog() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::LocalMkdir);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalMkdir(_)));
    }

    #[test]
    fn smoke_local_mkdir_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalMkdir("test".into());
        app.update(Message::LocalMkdirCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_local_delete_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalConfirmDelete("file.txt".into());
        app.update(Message::LocalDeleteCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_local_rename_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalRename("old.txt".into(), "new.txt".into());
        app.update(Message::LocalRenameCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_save_connection_dialog() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].connect_state.username = "user".into();
        app.sessions[0].connect_state.host = "host.com".into();
        app.update(Message::SaveConnection);
        assert!(matches!(app.sessions[0].dialog, Dialog::SaveProfile(ref n) if n == "user@host.com"));
    }

    #[test]
    fn smoke_save_connection_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::SaveProfile("test".into());
        app.update(Message::SaveConnectionCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    // --- Selection and navigation ---
    #[test]
    fn smoke_local_click_selects() {
        let mut app = App::default();
        // Entry 1 should exist (first file/dir after "..")
        if app.sessions[0].local_entries.len() > 1 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            assert!(app.sessions[0].local_selected.contains(&1));
            assert_eq!(app.sessions[0].active_panel, ActivePanel::Local);
        }
    }

    #[test]
    fn smoke_local_click_replaces_selection() {
        let mut app = App::default();
        if app.sessions[0].local_entries.len() > 2 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(2, false, false)));
            assert!(!app.sessions[0].local_selected.contains(&1));
            assert!(app.sessions[0].local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_local_ctrl_click_adds() {
        let mut app = App::default();
        if app.sessions[0].local_entries.len() > 2 {
            app.modifiers = Modifiers::CTRL;
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(2, false, false)));
            assert!(app.sessions[0].local_selected.contains(&1));
            assert!(app.sessions[0].local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_local_shift_click_range() {
        let mut app = App::default();
        if app.sessions[0].local_entries.len() > 3 {
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(1, false, false)));
            app.modifiers = Modifiers::SHIFT;
            app.update(Message::LocalPanel(PanelMessage::EntryClicked(3, false, false)));
            assert!(app.sessions[0].local_selected.contains(&1));
            assert!(app.sessions[0].local_selected.contains(&2));
            assert!(app.sessions[0].local_selected.contains(&3));
        }
    }

    #[test]
    fn smoke_local_go_up() {
        let mut app = App::default();
        let original = app.sessions[0].local_path.clone();
        if original != "/" {
            app.update(Message::LocalPanel(PanelMessage::GoUp));
            assert_ne!(app.sessions[0].local_path, original);
            assert!(app.sessions[0].local_selected.is_empty());
        }
    }

    #[test]
    fn smoke_local_filter() {
        let mut app = App::default();
        app.update(Message::LocalPanel(PanelMessage::FilterChanged("test".into())));
        assert_eq!(app.sessions[0].local_filter, "test");
        assert!(app.sessions[0].local_selected.is_empty());
    }

    #[test]
    fn smoke_local_sort_toggles() {
        let mut app = App::default();
        assert_eq!(app.sessions[0].sort_column, SortColumn::Name);
        assert!(app.sessions[0].sort_ascending);
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Name)));
        assert!(!app.sessions[0].sort_ascending);
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert_eq!(app.sessions[0].sort_column, SortColumn::Size);
        assert!(app.sessions[0].sort_ascending);
    }

    #[test]
    fn smoke_local_path_segment_click() {
        let mut app = App::default();
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/tmp".into())));
        assert_eq!(app.sessions[0].local_path, "/tmp");
        assert!(app.sessions[0].local_selected.is_empty());
    }

    // --- Remote panel (without connection) ---
    #[test]
    fn smoke_remote_delete_no_selection() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::RemoteDelete);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_delete_skips_dotdot() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![FileEntry {
            name: "..".into(), is_dir: true, size: 0,
            modified: String::new(), permissions: String::new(),
        }];
        app.sessions[0].remote_selected.insert(0);
        app.update(Message::RemoteDelete);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_delete_single() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "file.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].remote_selected.insert(1);
        app.update(Message::RemoteDelete);
        assert!(matches!(app.sessions[0].dialog, Dialog::ConfirmDelete(ref n) if n == "file.txt"));
    }

    #[test]
    fn smoke_remote_delete_batch_label() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "a.txt".into(), is_dir: false, size: 10, modified: String::new(), permissions: String::new() },
            FileEntry { name: "b.txt".into(), is_dir: false, size: 20, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].remote_selected.insert(1);
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::RemoteDelete);
        assert!(matches!(app.sessions[0].dialog, Dialog::ConfirmDelete(ref n) if n == "2 items"));
    }

    #[test]
    fn smoke_remote_rename_no_selection() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::RemoteRename);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_rename_skips_dotdot() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![FileEntry {
            name: "..".into(), is_dir: true, size: 0,
            modified: String::new(), permissions: String::new(),
        }];
        app.sessions[0].remote_selected.insert(0);
        app.update(Message::RemoteRename);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn smoke_remote_rename_opens_dialog() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "data.csv".into(), is_dir: false, size: 50, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].remote_selected.insert(1);
        app.update(Message::RemoteRename);
        assert!(matches!(app.sessions[0].dialog, Dialog::Rename(ref old, ref new) if old == "data.csv" && new == "data.csv"));
    }

    // --- Transfer state ---
    #[test]
    fn smoke_transfer_progress_updates() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 500, total_size: 1000, done: false,
        }));
        assert!((app.sessions[0].transfer_progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn smoke_transfer_progress_zero_total() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.sessions[0].transfer_progress = 0.3;
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 0, total_size: 0, done: false,
        }));
        // Should not update (div by zero guard)
        assert!((app.sessions[0].transfer_progress - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn smoke_transfer_complete_logs() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.update(Message::TransferComplete("Downloaded 3 items".into()));
        assert!(!app.sessions[0].transfer_active);
        assert_eq!(app.sessions[0].transfer_progress, 1.0);
        assert_eq!(app.sessions[0].transfer_log.len(), 1);
        assert!(!app.sessions[0].transfer_log[0].is_error);
        assert!(app.sessions[0].transfer_log[0].message.contains("Downloaded"));
    }

    #[test]
    fn smoke_transfer_failed_logs_error() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.update(Message::TransferFailed("connection lost".into()));
        assert!(!app.sessions[0].transfer_active);
        assert_eq!(app.sessions[0].transfer_progress, 0.0);
        assert_eq!(app.sessions[0].transfer_log.len(), 1);
        assert!(app.sessions[0].transfer_log[0].is_error);
        assert!(app.sessions[0].transfer_log[0].message.contains("connection lost"));
    }

    #[test]
    fn smoke_multiple_transfers_accumulate_log() {
        let mut app = App::default();
        app.update(Message::TransferComplete("file1".into()));
        app.update(Message::TransferComplete("file2".into()));
        app.update(Message::TransferFailed("err".into()));
        assert_eq!(app.sessions[0].transfer_log.len(), 3);
        assert!(!app.sessions[0].transfer_log[0].is_error);
        assert!(!app.sessions[0].transfer_log[1].is_error);
        assert!(app.sessions[0].transfer_log[2].is_error);
    }

    // --- Remote op messages ---
    #[test]
    fn smoke_remote_op_done_sets_status() {
        let mut app = App::default();
        app.update(Message::RemoteOpDone("Created dir".into()));
        assert_eq!(app.sessions[0].status, "Created dir");
    }

    #[test]
    fn smoke_remote_op_failed_sets_status() {
        let mut app = App::default();
        app.update(Message::RemoteOpFailed("Permission denied".into()));
        assert!(app.sessions[0].status.contains("Permission denied"));
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        if app.sessions[0].local_entries.len() > 2 {
            app.sessions[0].local_selected.clear();
            app.sessions[0].local_selected.insert(0);
            app.update(Message::KeyPressed(
                Key::Named(keyboard::key::Named::ArrowDown),
                Modifiers::empty(),
            ));
            assert!(app.sessions[0].local_selected.contains(&1));
            assert!(!app.sessions[0].local_selected.contains(&0));
        }
    }

    #[test]
    fn smoke_key_up_selects_prev() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        if app.sessions[0].local_entries.len() > 2 {
            app.sessions[0].local_selected.clear();
            app.sessions[0].local_selected.insert(2);
            app.sessions[0].last_anchor_local = Some(2);
            app.update(Message::KeyPressed(
                Key::Named(keyboard::key::Named::ArrowUp),
                Modifiers::empty(),
            ));
            assert!(app.sessions[0].local_selected.contains(&1));
            assert!(!app.sessions[0].local_selected.contains(&2));
        }
    }

    #[test]
    fn smoke_key_down_clamps_at_end() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        let max = app.sessions[0].local_entries.len().saturating_sub(1);
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(max);
        app.sessions[0].last_anchor_local = Some(max);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        ));
        assert!(app.sessions[0].local_selected.contains(&max));
    }

    #[test]
    fn smoke_key_up_clamps_at_zero() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(0);
        app.sessions[0].last_anchor_local = Some(0);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowUp),
            Modifiers::empty(),
        ));
        assert!(app.sessions[0].local_selected.contains(&0));
    }

    // --- Connect state ---
    #[test]
    fn smoke_connect_state_fields_update() {
        let mut app = App::default();
        app.update(Message::Connect(ConnectMessage::HostChanged("server.com".into())));
        assert_eq!(app.sessions[0].connect_state.host, "server.com");
        app.update(Message::Connect(ConnectMessage::PortChanged("2222".into())));
        assert_eq!(app.sessions[0].connect_state.port, "2222");
        app.update(Message::Connect(ConnectMessage::UsernameChanged("admin".into())));
        assert_eq!(app.sessions[0].connect_state.username, "admin");
        app.update(Message::Connect(ConnectMessage::PasswordChanged("pass123".into())));
        assert_eq!(app.sessions[0].connect_state.password, "pass123");
    }

    #[test]
    fn smoke_connect_starts_connecting_screen() {
        let mut app = App::default();
        app.sessions[0].connect_state.host = "example.com".into();
        app.sessions[0].connect_state.port = "22".into();
        app.sessions[0].connect_state.username = "user".into();
        app.update(Message::Connect(ConnectMessage::Connect));
        assert_eq!(app.sessions[0].screen, Screen::Connecting);
        assert!(app.sessions[0].status.contains("example.com"));
    }

    #[test]
    fn smoke_connect_cancel_resets() {
        let mut app = App::default();
        app.sessions[0].connect_state.host = "old.com".into();
        app.update(Message::Connect(ConnectMessage::Cancel));
        // Cancel creates new state
        assert!(app.sessions[0].connect_state.host.is_empty());
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
        assert_eq!(app.sessions[0].remote_path, "/home/user");
        assert_eq!(app.sessions[0].remote_entries.len(), 3); // ".." + 2
        assert!(app.sessions[0].remote_selected.is_empty());
    }

    #[test]
    fn smoke_remote_listing_failed() {
        let mut app = App::default();
        app.update(Message::RemoteListingFailed("timeout".into()));
        assert!(app.sessions[0].status.contains("timeout"));
    }

    // --- Local mkdir confirm (actual filesystem) ---
    #[test]
    fn smoke_local_mkdir_confirm() {
        let tmp = std::env::temp_dir().join("eurosftp_smoke_mkdir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].dialog = Dialog::LocalMkdir("testdir".into());
        app.update(Message::LocalMkdirConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
        assert!(tmp.join("testdir").exists());
        assert!(app.sessions[0].status.contains("Created"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn smoke_local_mkdir_empty_name() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalMkdir(String::new());
        app.update(Message::LocalMkdirConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].dialog = Dialog::LocalRename("old.txt".into(), "new.txt".into());
        app.update(Message::LocalRenameConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
        assert!(!tmp.join("old.txt").exists());
        assert!(tmp.join("new.txt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn smoke_local_rename_same_name_noop() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalRename("same.txt".into(), "same.txt".into());
        app.update(Message::LocalRenameConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    // --- Local delete (actual filesystem) ---
    #[test]
    fn smoke_local_delete_confirm() {
        let tmp = std::env::temp_dir().join("eurosftp_smoke_delete");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("todelete.txt"), "bye").unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);
        // Find the index of "todelete.txt"
        let idx = app.sessions[0].local_entries.iter().position(|e| e.name == "todelete.txt").unwrap();
        app.sessions[0].local_selected.insert(idx);
        app.sessions[0].dialog = Dialog::LocalConfirmDelete("todelete.txt".into());
        app.update(Message::LocalDeleteConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();

        // 1. Create subdirectory via dialog
        app.update(Message::LocalMkdir);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalMkdir(_)));
        app.update(Message::LocalMkdirNameChanged("subdir".into()));
        app.update(Message::LocalMkdirConfirm);
        assert!(tmp.join("subdir").is_dir());
        assert!(app.sessions[0].status.contains("Created"));

        // 2. Verify entries refreshed and subdir is present
        let subdir_idx = app.sessions[0].local_entries.iter()
            .position(|e| e.name == "subdir")
            .expect("subdir should appear in entries after mkdir");
        assert!(app.sessions[0].local_entries[subdir_idx].is_dir);

        // 3. Create a file inside via filesystem, then refresh
        std::fs::write(tmp.join("data.txt"), "hello").unwrap();
        app.update(Message::LocalPanel(PanelMessage::Refresh));
        let file_idx = app.sessions[0].local_entries.iter()
            .position(|e| e.name == "data.txt")
            .expect("data.txt should appear after refresh");
        assert!(!app.sessions[0].local_entries[file_idx].is_dir);

        // 4. Rename file
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(file_idx);
        app.update(Message::LocalRename);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalRename(ref old, _) if old == "data.txt"));
        app.update(Message::LocalRenameNameChanged("renamed.txt".into()));
        app.update(Message::LocalRenameConfirm);
        assert!(!tmp.join("data.txt").exists());
        assert!(tmp.join("renamed.txt").exists());

        // 5. Delete the renamed file
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);
        let renamed_idx = app.sessions[0].local_entries.iter()
            .position(|e| e.name == "renamed.txt").unwrap();
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(renamed_idx);
        app.update(Message::LocalDelete);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalConfirmDelete(ref n) if n == "renamed.txt"));
        app.update(Message::LocalDeleteConfirm);
        assert!(!tmp.join("renamed.txt").exists());

        // 6. Delete the subdirectory
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);
        let subdir_idx = app.sessions[0].local_entries.iter()
            .position(|e| e.name == "subdir").unwrap();
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(subdir_idx);
        app.sessions[0].dialog = Dialog::LocalConfirmDelete("subdir".into());
        app.update(Message::LocalDeleteConfirm);
        assert!(!tmp.join("subdir").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Navigation: breadcrumb click → go up → path segment → verify entries ---
    #[test]
    fn integ_local_navigation_flow() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;

        // Navigate to /tmp
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/tmp".into())));
        assert_eq!(app.sessions[0].local_path, "/tmp");
        assert!(app.sessions[0].local_selected.is_empty());
        assert!(app.sessions[0].local_filter.is_empty());

        // Go up → should be /
        app.update(Message::LocalPanel(PanelMessage::GoUp));
        assert_eq!(app.sessions[0].local_path, "/");

        // Navigate to /usr/bin
        app.update(Message::LocalPanel(PanelMessage::PathSegmentClicked("/usr".into())));
        assert_eq!(app.sessions[0].local_path, "/usr");
        let has_bin = app.sessions[0].local_entries.iter().any(|e| e.name == "bin" && e.is_dir);
        assert!(has_bin, "/usr should contain bin/");

        // Enter bin via double-click simulation (entry click on dir)
        let bin_idx = app.sessions[0].local_entries.iter().position(|e| e.name == "bin").unwrap();
        // First click sets selection
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(bin_idx, false, false)));
        // Immediate second click within 400ms triggers navigation
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(bin_idx, false, false)));
        assert_eq!(app.sessions[0].local_path, "/usr/bin");
    }

    // --- Keyboard navigation flow: arrows + enter ---
    #[test]
    fn integ_keyboard_navigation_enter_dir() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_kbd_nav");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("inner")).unwrap();
        std::fs::write(tmp.join("file.txt"), "x").unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);
        app.sessions[0].active_panel = ActivePanel::Local;

        // Arrow down from top to select first real entry
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        ));
        let selected = *app.sessions[0].local_selected.iter().next().unwrap();
        assert!(selected >= 1);

        // Find the "inner" dir index
        let inner_idx = app.sessions[0].local_entries.iter()
            .position(|e| e.name == "inner").unwrap();

        // Navigate to inner via arrow keys
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(inner_idx);
        app.sessions[0].last_anchor_local = Some(inner_idx);

        // Press Enter to open directory
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        assert!(app.sessions[0].local_path.ends_with("inner"));
        assert!(app.sessions[0].local_selected.is_empty());

        // Press Enter on ".." to go back up
        app.sessions[0].local_selected.insert(0); // ".." is always index 0
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].local_path, tmp.to_string_lossy().to_string());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Keyboard shortcuts: F7 mkdir, F2 rename, Delete ---
    #[test]
    fn integ_keyboard_shortcuts_open_dialogs() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;

        // F7 → local mkdir dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalMkdir(_)));
        app.sessions[0].dialog = Dialog::None;

        // F2 with selection → local rename dialog
        app.sessions[0].local_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F2),
            Modifiers::empty(),
        ));
        if app.sessions[0].local_entries.len() > 1 && app.sessions[0].local_entries[1].name != ".." {
            assert!(matches!(app.sessions[0].dialog, Dialog::LocalRename(_, _)));
        }
        app.sessions[0].dialog = Dialog::None;

        // Delete with selection → local delete dialog
        app.sessions[0].local_selected.clear();
        app.sessions[0].local_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Delete),
            Modifiers::empty(),
        ));
        if app.sessions[0].local_entries.len() > 1 && app.sessions[0].local_entries[1].name != ".." {
            assert!(matches!(app.sessions[0].dialog, Dialog::LocalConfirmDelete(_)));
        }
    }

    // --- Keyboard shortcuts with remote active panel ---
    #[test]
    fn integ_keyboard_shortcuts_remote_panel() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Remote;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "remote.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];

        // F7 → remote mkdir dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert!(matches!(app.sessions[0].dialog, Dialog::Mkdir(_)));
        app.sessions[0].dialog = Dialog::None;

        // F2 with selection → remote rename dialog
        app.sessions[0].remote_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F2),
            Modifiers::empty(),
        ));
        assert!(matches!(app.sessions[0].dialog, Dialog::Rename(ref old, _) if old == "remote.txt"));
        app.sessions[0].dialog = Dialog::None;

        // Delete → remote delete dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Delete),
            Modifiers::empty(),
        ));
        assert!(matches!(app.sessions[0].dialog, Dialog::ConfirmDelete(ref n) if n == "remote.txt"));
    }

    // --- Tab switches active panel ---
    #[test]
    fn integ_tab_switches_panel() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Local);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Remote);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Local);
    }

    // --- Dialogs block keyboard shortcuts ---
    #[test]
    fn integ_dialog_blocks_shortcuts() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].dialog = Dialog::Mkdir("test".into());

        // F7 should be blocked by open dialog
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        // Dialog should still be Mkdir, not LocalMkdir
        assert!(matches!(app.sessions[0].dialog, Dialog::Mkdir(_)));
    }

    // --- Shortcuts blocked on Connect screen ---
    #[test]
    fn integ_shortcuts_blocked_on_connect_screen() {
        let mut app = App::default();
        assert_eq!(app.sessions[0].screen, Screen::Connect);

        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F7),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        // Select some items
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].local_selected.insert(2);
        assert_eq!(app.sessions[0].local_selected.len(), 2);

        // Sort by size → should clear selection
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert!(app.sessions[0].local_selected.is_empty());

        // Apply filter
        app.update(Message::LocalPanel(PanelMessage::FilterChanged(".txt".into())));
        assert_eq!(app.sessions[0].local_filter, ".txt");
        assert!(app.sessions[0].local_selected.is_empty());

        // Entries still exist (filter is applied in view, not data)
        assert!(app.sessions[0].local_entries.iter().any(|e| e.name == "beta.log"));

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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        // Ctrl-click to select a.txt and c.txt
        let a_idx = app.sessions[0].local_entries.iter().position(|e| e.name == "a.txt").unwrap();
        let c_idx = app.sessions[0].local_entries.iter().position(|e| e.name == "c.txt").unwrap();

        app.modifiers = Modifiers::CTRL;
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(a_idx, false, false)));
        app.update(Message::LocalPanel(PanelMessage::EntryClicked(c_idx, false, false)));
        assert!(app.sessions[0].local_selected.contains(&a_idx));
        assert!(app.sessions[0].local_selected.contains(&c_idx));

        // Trigger delete
        app.modifiers = Modifiers::empty();
        app.update(Message::LocalDelete);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalConfirmDelete(ref n) if n == "2 items"));

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
            favorite: false,
            jump_host: None,
        };
        let path = tmp.join("connections.json");
        let json = serde_json::to_string_pretty(&vec![profile]).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Load and apply
        let content = std::fs::read_to_string(&path).unwrap();
        let profiles: Vec<ConnectionProfile> = serde_json::from_str(&content).unwrap();

        let mut app = App::default();
        app.sessions[0].connect_state.profiles = profiles;
        app.sessions[0].connect_state.apply_profile("integration-test");

        assert_eq!(app.sessions[0].connect_state.host, "10.0.0.42");
        assert_eq!(app.sessions[0].connect_state.port, "2222");
        assert_eq!(app.sessions[0].connect_state.username, "deploy");
        assert_eq!(app.sessions[0].connect_state.password, "s3cret");
        assert_eq!(app.sessions[0].connect_state.selected_profile, Some("integration-test".into()));

        // Verify auth_method() returns correct type
        let auth = app.sessions[0].connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Password(ref p) if p == "s3cret"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Theme cycle through all + verify iced theme conversion ---
    #[test]
    fn integ_theme_full_cycle_with_conversion() {
        let mut app = App::default();
        app.theme = AppTheme::Dark; // Ensure deterministic start
        let start = app.theme;
        let mut seen_themes = Vec::new();

        for _ in 0..AppTheme::all().len() {
            seen_themes.push(app.theme);
            let _ = app.theme();
            app.update(Message::CycleTheme);
        }
        assert_eq!(seen_themes.len(), AppTheme::all().len());
        for t in AppTheme::all() {
            assert!(seen_themes.contains(t), "Missing theme: {:?}", t);
        }
        assert_eq!(app.theme, start);
    }

    // --- Transfer log + toggle log interaction ---
    #[test]
    fn integ_transfer_log_accumulation_and_toggle() {
        let mut app = App::default();

        // Accumulate mixed log entries
        app.update(Message::TransferComplete("Downloaded file1.txt".into()));
        app.update(Message::TransferFailed("Permission denied on file2".into()));
        app.update(Message::TransferComplete("Uploaded 3 items".into()));

        assert_eq!(app.sessions[0].transfer_log.len(), 3);
        assert!(!app.sessions[0].transfer_log[0].is_error);
        assert!(app.sessions[0].transfer_log[1].is_error);
        assert!(!app.sessions[0].transfer_log[2].is_error);

        // All entries should have timestamps
        for entry in &app.sessions[0].transfer_log {
            assert!(!entry.timestamp.is_empty());
        }

        // Toggle log visibility
        assert!(!app.show_log);
        app.update(Message::ToggleLog);
        assert!(app.show_log);

        // Log content persists after toggle
        assert_eq!(app.sessions[0].transfer_log.len(), 3);
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
        assert_eq!(app.sessions[0].screen, Screen::Connecting);
        assert!(app.sessions[0].status.contains("192.168.1.99"));

        // Simulate failure
        app.update(Message::ConnectionFailed("Connection refused".into()));
        assert_eq!(app.sessions[0].screen, Screen::Connect);
        assert!(app.sessions[0].status.contains("Connection refused"));

        // Form values should still be there for retry
        assert_eq!(app.sessions[0].connect_state.host, "192.168.1.99");
        assert_eq!(app.sessions[0].connect_state.username, "testuser");
    }

    // --- Remote listing + navigate + selection state ---
    #[test]
    fn integ_remote_listing_navigation_state() {
        use crate::core::sftp::RemoteEntry;
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;

        // Simulate initial listing
        let entries = vec![
            RemoteEntry { name: "docs".into(), is_dir: true, size: 0, mtime: 0, permissions: 0o755 },
            RemoteEntry { name: "src".into(), is_dir: true, size: 0, mtime: 0, permissions: 0o755 },
            RemoteEntry { name: "main.rs".into(), is_dir: false, size: 200, mtime: 1704067200, permissions: 0o644 },
        ];
        app.update(Message::RemoteListing(entries, "/home/user".into()));
        assert_eq!(app.sessions[0].remote_path, "/home/user");
        assert_eq!(app.sessions[0].remote_entries.len(), 4); // ".." + 3
        assert!(app.sessions[0].remote_selected.is_empty());

        // Select an entry
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(1, false, false)));
        assert!(app.sessions[0].remote_selected.contains(&1));
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Remote);

        // New listing clears selection
        let new_entries = vec![
            RemoteEntry { name: "README.md".into(), is_dir: false, size: 100, mtime: 0, permissions: 0o644 },
        ];
        app.update(Message::RemoteListing(new_entries, "/home/user/docs".into()));
        assert_eq!(app.sessions[0].remote_path, "/home/user/docs");
        assert!(app.sessions[0].remote_selected.is_empty());
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        let count_no_hidden = app.sessions[0].local_entries.len();
        assert!(!app.sessions[0].local_entries.iter().any(|e| e.name == ".hidden"));

        // Toggle hidden on (simulate ToggleHidden but without sftp)
        app.sessions[0].show_hidden = true;
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, true);
        assert!(app.sessions[0].local_entries.len() > count_no_hidden);
        assert!(app.sessions[0].local_entries.iter().any(|e| e.name == ".hidden"));

        // Toggle back
        app.sessions[0].show_hidden = false;
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);
        assert!(!app.sessions[0].local_entries.iter().any(|e| e.name == ".hidden"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- Ctrl+H shortcut toggles hidden ---
    #[test]
    fn integ_ctrl_h_toggles_hidden() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        assert!(!app.sessions[0].show_hidden);

        app.update(Message::KeyPressed(
            Key::Character("h".into()),
            Modifiers::CTRL,
        ));
        assert!(app.sessions[0].show_hidden);

        app.update(Message::KeyPressed(
            Key::Character("h".into()),
            Modifiers::CTRL,
        ));
        assert!(!app.sessions[0].show_hidden);
    }

    // --- Selection + sort interaction: sort clears both panels ---
    #[test]
    fn integ_sort_clears_both_panels_selection() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "remote.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];

        app.sessions[0].local_selected.insert(1);
        app.sessions[0].remote_selected.insert(1);

        // Sort from local panel
        app.update(Message::LocalPanel(PanelMessage::SortBy(SortColumn::Size)));
        assert!(app.sessions[0].local_selected.is_empty());
        assert!(app.sessions[0].remote_selected.is_empty());

        // Re-select
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].remote_selected.insert(1);

        // Sort from remote panel
        app.update(Message::RemotePanel(PanelMessage::SortBy(SortColumn::Name)));
        assert!(app.sessions[0].local_selected.is_empty());
        assert!(app.sessions[0].remote_selected.is_empty());
    }

    // --- Save profile replaces existing with same name ---
    #[test]
    fn integ_save_profile_upsert() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].connect_state.host = "old.com".into();
        app.sessions[0].connect_state.port = "22".into();
        app.sessions[0].connect_state.username = "user".into();

        // Manually add a profile
        use crate::core::connection::{ConnectionProfile, AuthMethod};
        app.sessions[0].connect_state.profiles = vec![ConnectionProfile {
            name: "myserver".into(),
            host: "old.com".into(),
            port: 22,
            username: "user".into(),
            auth: AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        }];

        // Open save dialog and update host
        app.sessions[0].connect_state.host = "new.com".into();
        app.update(Message::Connect(ConnectMessage::JumpHostToggled(true)));
        app.update(Message::Connect(ConnectMessage::JumpHostChanged("bastion.example.com".into())));
        app.update(Message::Connect(ConnectMessage::JumpPortChanged("2222".into())));
        app.update(Message::Connect(ConnectMessage::JumpUsernameChanged("jumper".into())));
        app.update(Message::Connect(ConnectMessage::JumpPasswordChanged("pw".into())));
        app.update(Message::Connect(ConnectMessage::JumpAuthTypeSelected(crate::ui::connect_dialog::AuthType::Password)));
        app.sessions[0].dialog = Dialog::SaveProfile("myserver".into());
        app.update(Message::SaveConnectionConfirm);

        // Should have updated, not duplicated
        assert_eq!(app.sessions[0].connect_state.profiles.len(), 1);
        assert_eq!(app.sessions[0].connect_state.profiles[0].host, "new.com");
        let jump = app.sessions[0].connect_state.profiles[0].jump_host.as_ref().expect("jump host should persist");
        assert_eq!(jump.host, "bastion.example.com");
        assert_eq!(jump.port, 2222);
        assert_eq!(jump.username, "jumper");
    }

    // --- Shift-select on remote panel ---
    #[test]
    fn integ_shift_select_remote() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "a".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "b".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "c".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "d".into(), is_dir: false, size: 0, modified: String::new(), permissions: String::new() },
        ];

        // Click entry 1 to set anchor
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(1, false, false)));
        assert_eq!(app.sessions[0].remote_selected.len(), 1);

        // Shift-click entry 3 to select range
        app.modifiers = Modifiers::SHIFT;
        app.update(Message::RemotePanel(PanelMessage::EntryClicked(3, false, false)));
        assert_eq!(app.sessions[0].remote_selected.len(), 3);
        assert!(app.sessions[0].remote_selected.contains(&1));
        assert!(app.sessions[0].remote_selected.contains(&2));
        assert!(app.sessions[0].remote_selected.contains(&3));
    }

    // --- Auth type switching ---
    #[test]
    fn integ_auth_type_switch() {
        use crate::ui::connect_dialog::AuthType;
        let mut app = App::default();

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::Agent)));
        assert_eq!(app.sessions[0].connect_state.auth_type, AuthType::Agent);
        let auth = app.sessions[0].connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Agent));

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::KeyFile)));
        assert_eq!(app.sessions[0].connect_state.auth_type, AuthType::KeyFile);
        app.update(Message::Connect(ConnectMessage::KeyPathChanged("/tmp/key".into())));
        let auth = app.sessions[0].connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::KeyFile(ref p) if p.to_string_lossy() == "/tmp/key"));

        app.update(Message::Connect(ConnectMessage::AuthTypeSelected(AuthType::Password)));
        app.update(Message::Connect(ConnectMessage::PasswordChanged("mypass".into())));
        let auth = app.sessions[0].connect_state.auth_method();
        assert!(matches!(auth, crate::core::connection::AuthMethod::Password(ref p) if p == "mypass"));
    }

    // --- Download/upload without connection is no-op ---
    #[test]
    fn integ_transfer_without_connection_noop() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "file.txt".into(), is_dir: false, size: 100, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].remote_selected.insert(1);
        app.sessions[0].local_selected.insert(1);

        // No sftp connection → should not panic or change state
        app.update(Message::DownloadSelected);
        assert!(!app.sessions[0].transfer_active);

        app.update(Message::UploadSelected);
        assert!(!app.sessions[0].transfer_active);
    }

    // --- Nested dir delete on filesystem ---
    #[test]
    fn integ_local_delete_nested_dir() {
        let tmp = std::env::temp_dir().join("eurosftp_integ_nested_del");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("parent/child/grandchild")).unwrap();
        std::fs::write(tmp.join("parent/child/file.txt"), "data").unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        let parent_idx = app.sessions[0].local_entries.iter().position(|e| e.name == "parent").unwrap();
        app.sessions[0].local_selected.insert(parent_idx);
        app.sessions[0].dialog = Dialog::LocalConfirmDelete("parent".into());
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
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].dialog = Dialog::Mkdir("test".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_confirm_delete() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].dialog = Dialog::ConfirmDelete("file.txt".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_rename() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].dialog = Dialog::Rename("old".into(), "new".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_mkdir() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalMkdir("dir".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_delete() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalConfirmDelete("x".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_local_rename() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::LocalRename("a".into(), "b".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_closes_save_profile() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::SaveProfile("name".into());
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b4_escape_noop_when_no_dialog() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].dialog = Dialog::None;
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    // --- Transfer speed tracking state ---
    #[test]
    fn b4_transfer_start_state() {
        let app = App::default();
        assert!(app.sessions[0].transfer_start.is_none());
        assert_eq!(app.sessions[0].transfer_bytes, 0);
        assert_eq!(app.sessions[0].transfer_total, 0);
    }

    #[test]
    fn b4_transfer_progress_tracks_bytes() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.sessions[0].transfer_start = Some(Instant::now());
        app.update(Message::TransferProgress(TransferProgress {
            transferred: 50000, total_size: 100000, done: false,
        }));
        assert_eq!(app.sessions[0].transfer_bytes, 50000);
        assert_eq!(app.sessions[0].transfer_total, 100000);
    }

    #[test]
    fn b4_transfer_complete_resets_start() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.sessions[0].transfer_start = Some(Instant::now());
        app.sessions[0].transfer_bytes = 1000;
        app.update(Message::TransferComplete("done".into()));
        assert!(app.sessions[0].transfer_start.is_none());
    }

    #[test]
    fn b4_transfer_failed_resets_start() {
        let mut app = App::default();
        app.sessions[0].transfer_active = true;
        app.sessions[0].transfer_start = Some(Instant::now());
        app.sessions[0].transfer_bytes = 500;
        app.update(Message::TransferFailed("err".into()));
        assert!(app.sessions[0].transfer_start.is_none());
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

    // ===== CONTEXT MENU TESTS =====

    fn make_browser_app_with_entries() -> App {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "docs".into(), is_dir: true, size: 4096, modified: String::new(), permissions: String::new() },
            FileEntry { name: "readme.md".into(), is_dir: false, size: 500, modified: String::new(), permissions: String::new() },
            FileEntry { name: "data.csv".into(), is_dir: false, size: 10000, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].remote_entries = vec![
            FileEntry { name: "..".into(), is_dir: true, size: 0, modified: String::new(), permissions: String::new() },
            FileEntry { name: "src".into(), is_dir: true, size: 4096, modified: String::new(), permissions: String::new() },
            FileEntry { name: "main.rs".into(), is_dir: false, size: 200, modified: String::new(), permissions: String::new() },
        ];
        app.sessions[0].local_path = "/tmp".into();
        app.sessions[0].remote_path = "/home/user".into();
        app
    }

    #[test]
    fn ctx_right_click_local_opens_menu() {
        let mut app = make_browser_app_with_entries();
        app.update(Message::LocalPanel(PanelMessage::EntryRightClicked(2)));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::Local(2));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Local);
    }

    #[test]
    fn ctx_right_click_remote_opens_menu() {
        let mut app = make_browser_app_with_entries();
        app.update(Message::RemotePanel(PanelMessage::EntryRightClicked(1)));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::Remote(1));
        assert!(app.sessions[0].remote_selected.contains(&1));
        assert_eq!(app.sessions[0].active_panel, ActivePanel::Remote);
    }

    #[test]
    fn ctx_right_click_preserves_multiselect() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].local_selected.insert(2);
        // Right-click on already selected entry
        app.update(Message::LocalPanel(PanelMessage::EntryRightClicked(2)));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::Local(2));
        // Multi-selection preserved
        assert!(app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&2));
    }

    #[test]
    fn ctx_right_click_unselected_replaces_selection() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].local_selected.insert(1);
        // Right-click on non-selected entry
        app.update(Message::LocalPanel(PanelMessage::EntryRightClicked(3)));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::Local(3));
        assert!(!app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&3));
    }

    #[test]
    fn ctx_dismiss_clears_menu() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(1);
        app.update(Message::DismissContextMenu);
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_escape_closes_menu() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_rename_action_opens_dialog() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(2);
        app.sessions[0].local_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Rename));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalRename(ref old, _) if old == "readme.md"));
    }

    #[test]
    fn ctx_delete_action_opens_dialog() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(2);
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Delete));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::ConfirmDelete(ref n) if n == "main.rs"));
    }

    #[test]
    fn ctx_newdir_action_opens_dialog_local() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(1);
        app.update(Message::ShowContextMenu(ContextAction::NewDir));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalMkdir(_)));
    }

    #[test]
    fn ctx_newdir_action_opens_dialog_remote() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(1);
        app.update(Message::ShowContextMenu(ContextAction::NewDir));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::Mkdir(_)));
    }

    #[test]
    fn ctx_open_local_dir_navigates() {
        let tmp = std::env::temp_dir().join("eurosftp_ctx_open");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("inner")).unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        let inner_idx = app.sessions[0].local_entries.iter().position(|e| e.name == "inner").unwrap();
        app.sessions[0].context_menu = ContextMenu::Local(inner_idx);
        app.update(Message::ShowContextMenu(ContextAction::Open));
        assert!(app.sessions[0].local_path.ends_with("inner"));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ctx_copy_path_local() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(2);
        app.update(Message::ShowContextMenu(ContextAction::CopyPath));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(app.sessions[0].status.contains("/tmp/readme.md"));
    }

    #[test]
    fn ctx_copy_path_remote() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(2);
        app.update(Message::ShowContextMenu(ContextAction::CopyPath));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(app.sessions[0].status.contains("/home/user/main.rs"));
    }

    #[test]
    fn ctx_refresh_local() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(1);
        app.update(Message::ShowContextMenu(ContextAction::Refresh));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_download_without_connection_noop() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(2);
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Download));
        assert!(!app.sessions[0].transfer_active);
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_upload_without_connection_noop() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(2);
        app.sessions[0].local_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Upload));
        assert!(!app.sessions[0].transfer_active);
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_default_is_none() {
        let app = App::default();
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn ctx_rename_remote_action_opens_dialog() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(2);
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Rename));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::Rename(ref old, _) if old == "main.rs"));
    }

    #[test]
    fn ctx_delete_local_action_opens_dialog() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Local(2);
        app.sessions[0].local_selected.insert(2);
        app.update(Message::ShowContextMenu(ContextAction::Delete));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalConfirmDelete(ref n) if n == "readme.md"));
    }

    // ===== BATCH 2: SELECTION & KEYBOARD TESTS =====

    #[test]
    fn b2_ctrl_a_selects_all_local() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.update(Message::KeyPressed(
            Key::Character("a".into()),
            Modifiers::CTRL,
        ));
        // Should select all except ".." (index 0)
        assert_eq!(app.sessions[0].local_selected.len(), app.sessions[0].local_entries.len() - 1);
        assert!(!app.sessions[0].local_selected.contains(&0)); // ".." not selected
        assert!(app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert!(app.sessions[0].local_selected.contains(&3));
    }

    #[test]
    fn b2_ctrl_a_selects_all_remote() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Remote;
        app.update(Message::KeyPressed(
            Key::Character("a".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(app.sessions[0].remote_selected.len(), app.sessions[0].remote_entries.len() - 1);
        assert!(!app.sessions[0].remote_selected.contains(&0));
    }

    #[test]
    fn b2_ctrl_i_inverts_selection() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].local_selected.insert(2);
        app.update(Message::KeyPressed(
            Key::Character("i".into()),
            Modifiers::CTRL,
        ));
        // 1 and 2 were selected, now 3 should be (entries: 0=.., 1=docs, 2=readme, 3=data)
        assert!(!app.sessions[0].local_selected.contains(&0)); // ".." never selected
        assert!(!app.sessions[0].local_selected.contains(&1));
        assert!(!app.sessions[0].local_selected.contains(&2));
        assert!(app.sessions[0].local_selected.contains(&3));
    }

    #[test]
    fn b2_ctrl_i_inverts_empty_to_all() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Remote;
        // Nothing selected
        app.update(Message::KeyPressed(
            Key::Character("i".into()),
            Modifiers::CTRL,
        ));
        // Should select all except ".."
        assert_eq!(app.sessions[0].remote_selected.len(), app.sessions[0].remote_entries.len() - 1);
    }

    #[test]
    fn b2_ctrl_c_copies_paths() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(2);
        app.sessions[0].local_selected.insert(3);
        app.update(Message::KeyPressed(
            Key::Character("c".into()),
            Modifiers::CTRL,
        ));
        assert!(app.sessions[0].status.contains("Copied 2 path(s)"));
    }

    #[test]
    fn b2_ctrl_c_skips_dotdot() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(0); // ".."
        app.sessions[0].local_selected.insert(1);
        app.update(Message::KeyPressed(
            Key::Character("c".into()),
            Modifiers::CTRL,
        ));
        assert!(app.sessions[0].status.contains("Copied 1 path(s)"));
    }

    #[test]
    fn b2_f6_triggers_upload_from_local() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(2);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F6),
            Modifiers::empty(),
        ));
        // Without connection, no transfer starts, but message was dispatched
        assert!(!app.sessions[0].transfer_active);
    }

    #[test]
    fn b2_f6_triggers_download_from_remote() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Remote;
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F6),
            Modifiers::empty(),
        ));
        assert!(!app.sessions[0].transfer_active);
    }

    #[test]
    fn b2_f8_opens_delete_dialog() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(2);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F8),
            Modifiers::empty(),
        ));
        assert!(matches!(app.sessions[0].dialog, Dialog::LocalConfirmDelete(_)));
    }

    #[test]
    fn b2_shift_arrow_down_extends_selection() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].last_anchor_local = Some(1);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::SHIFT,
        ));
        assert!(app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert_eq!(app.sessions[0].local_selected.len(), 2);
    }

    #[test]
    fn b2_shift_arrow_up_extends_selection() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(3);
        app.sessions[0].last_anchor_local = Some(3);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowUp),
            Modifiers::SHIFT,
        ));
        assert!(app.sessions[0].local_selected.contains(&3));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert_eq!(app.sessions[0].local_selected.len(), 2);
    }

    #[test]
    fn b2_home_jumps_to_first() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(3);
        app.sessions[0].last_anchor_local = Some(3);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Home),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].local_selected.len(), 1);
        assert!(app.sessions[0].local_selected.contains(&0));
        assert_eq!(app.sessions[0].last_anchor_local, Some(0));
    }

    #[test]
    fn b2_end_jumps_to_last() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::End),
            Modifiers::empty(),
        ));
        let last = app.sessions[0].local_entries.len() - 1;
        assert_eq!(app.sessions[0].local_selected.len(), 1);
        assert!(app.sessions[0].local_selected.contains(&last));
        assert_eq!(app.sessions[0].last_anchor_local, Some(last));
    }

    #[test]
    fn b2_backspace_goes_up_local() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        let original = app.sessions[0].local_path.clone();
        if original != "/" {
            app.update(Message::KeyPressed(
                Key::Named(keyboard::key::Named::Backspace),
                Modifiers::empty(),
            ));
            assert_ne!(app.sessions[0].local_path, original);
        }
    }

    #[test]
    fn b2_enter_on_file_triggers_upload() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(2); // readme.md (file, not dir)
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        // Without connection, no transfer, but state should be unchanged
        assert!(!app.sessions[0].transfer_active);
    }

    #[test]
    fn b2_enter_on_dir_navigates() {
        let tmp = std::env::temp_dir().join("eurosftp_b2_enter");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("subdir")).unwrap();

        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_path = tmp.to_string_lossy().to_string();
        app.sessions[0].local_entries = read_local_dir(&app.sessions[0].local_path, false);

        let idx = app.sessions[0].local_entries.iter().position(|e| e.name == "subdir").unwrap();
        app.sessions[0].local_selected.insert(idx);
        app.sessions[0].last_anchor_local = Some(idx);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
        ));
        assert!(app.sessions[0].local_path.ends_with("subdir"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn b2_space_toggles_and_moves() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].last_anchor_local = Some(1);
        // Space should toggle 1 (deselect) and move to 2
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Space),
            Modifiers::empty(),
        ));
        assert!(!app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert_eq!(app.sessions[0].last_anchor_local, Some(2));
    }

    #[test]
    fn b2_space_selects_and_moves() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.clear();
        app.sessions[0].last_anchor_local = Some(1);
        // Space should toggle 1 (select it since not selected) and move to 2
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Space),
            Modifiers::empty(),
        ));
        assert!(app.sessions[0].local_selected.contains(&1));
        assert!(app.sessions[0].local_selected.contains(&2));
        assert_eq!(app.sessions[0].last_anchor_local, Some(2));
    }

    #[test]
    fn b2_arrow_without_shift_clears_selection() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        app.sessions[0].local_selected.insert(1);
        app.sessions[0].local_selected.insert(2);
        app.sessions[0].last_anchor_local = Some(2);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].local_selected.len(), 1);
        assert!(app.sessions[0].local_selected.contains(&3));
    }

    #[test]
    fn b2_ctrl_a_then_invert_clears() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].active_panel = ActivePanel::Local;
        // Select all
        app.update(Message::KeyPressed(
            Key::Character("a".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(app.sessions[0].local_selected.len(), 3);
        // Invert: all selected -> none
        app.update(Message::KeyPressed(
            Key::Character("i".into()),
            Modifiers::CTRL,
        ));
        assert_eq!(app.sessions[0].local_selected.len(), 0);
    }

    // ===== BATCH 3: THEME TESTS =====

    #[test]
    fn b3_open_theme_picker_dialog() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::OpenThemePicker);
        assert_eq!(app.sessions[0].dialog, Dialog::Settings);
    }

    #[test]
    fn b3_theme_selected_changes_theme() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::ThemePicker;
        app.update(Message::ThemeSelected(AppTheme::TokyoNight));
        assert_eq!(app.theme, AppTheme::TokyoNight);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b3_theme_selected_closes_dialog() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::ThemePicker;
        app.update(Message::ThemeSelected(AppTheme::CatppuccinMocha));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b3_escape_closes_theme_picker() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::ThemePicker;
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
        ));
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b3_cycle_theme_still_works() {
        let mut app = App::default();
        let initial = app.theme;
        app.update(Message::CycleTheme);
        assert_ne!(app.theme, initial);
    }

    #[test]
    fn b3_all_22_themes_selectable() {
        let mut app = App::default();
        for &theme in AppTheme::all() {
            app.sessions[0].dialog = Dialog::ThemePicker;
            app.update(Message::ThemeSelected(theme));
            assert_eq!(app.theme, theme);
            assert_eq!(app.sessions[0].dialog, Dialog::None);
            // Verify iced conversion works
            let _ = app.theme();
        }
    }

    #[test]
    fn b3_theme_serialize_roundtrip() {
        // Test serialization without touching disk (avoids test race conditions)
        for &theme in AppTheme::all() {
            let json = serde_json::to_string(&theme).unwrap();
            let restored: AppTheme = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, theme);
        }
    }

    #[test]
    fn b3_theme_picker_from_connect_screen() {
        let mut app = App::default();
        assert_eq!(app.sessions[0].screen, Screen::Connect);
        // Theme picker should still work from any screen via CycleTheme
        app.update(Message::CycleTheme);
        assert_ne!(app.theme, AppTheme::Dark);
    }

    // ── Batch 4: Terminal ──────────────────────────────────────────

    #[test]
    fn b4_terminal_toggle() {
        let mut app = App::default();
        assert!(!app.sessions[0].show_terminal);
        app.update(Message::ToggleTerminal);
        assert!(app.sessions[0].show_terminal);
        app.update(Message::ToggleTerminal);
        assert!(!app.sessions[0].show_terminal);
    }

    #[test]
    fn b4_terminal_input_changed() {
        let mut app = App::default();
        app.update(Message::TerminalInputChanged("ls -la".into()));
        assert_eq!(app.sessions[0].terminal_input, "ls -la");
    }

    #[test]
    fn b4_terminal_submit_no_connection() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::TerminalInputChanged("whoami".into()));
        app.update(Message::TerminalSubmit);
        assert_eq!(app.sessions[0].terminal_history.len(), 1);
        assert_eq!(app.sessions[0].terminal_history[0].command, "whoami");
        assert!(app.sessions[0].terminal_history[0].is_error);
        assert_eq!(app.sessions[0].terminal_history[0].output, "Not connected");
        assert!(app.sessions[0].terminal_input.is_empty());
        assert!(!app.sessions[0].terminal_running);
    }

    #[test]
    fn b4_terminal_submit_empty_ignored() {
        let mut app = App::default();
        app.update(Message::TerminalInputChanged("".into()));
        app.update(Message::TerminalSubmit);
        assert!(app.sessions[0].terminal_history.is_empty());
    }

    #[test]
    fn b4_terminal_submit_whitespace_ignored() {
        let mut app = App::default();
        app.update(Message::TerminalInputChanged("   ".into()));
        app.update(Message::TerminalSubmit);
        assert!(app.sessions[0].terminal_history.is_empty());
    }

    #[test]
    fn b4_terminal_output_message() {
        let mut app = App::default();
        app.sessions[0].terminal_cmd_history.push("uname".into());
        app.sessions[0].terminal_running = true;
        app.update(Message::TerminalOutput("Linux\n".into()));
        assert_eq!(app.sessions[0].terminal_history.len(), 1);
        assert_eq!(app.sessions[0].terminal_history[0].command, "uname");
        assert_eq!(app.sessions[0].terminal_history[0].output, "Linux\n");
        assert!(!app.sessions[0].terminal_history[0].is_error);
        assert!(!app.sessions[0].terminal_running);
    }

    #[test]
    fn b4_terminal_error_message() {
        let mut app = App::default();
        app.sessions[0].terminal_cmd_history.push("bad_cmd".into());
        app.sessions[0].terminal_running = true;
        app.update(Message::TerminalError("command not found".into()));
        assert_eq!(app.sessions[0].terminal_history.len(), 1);
        assert!(app.sessions[0].terminal_history[0].is_error);
        assert!(!app.sessions[0].terminal_running);
    }

    #[test]
    fn b4_terminal_clear() {
        let mut app = App::default();
        app.sessions[0].terminal_history.push(TerminalEntry {
            command: "ls".into(),
            output: "file.txt".into(),
            is_error: false,
        });
        app.sessions[0].terminal_history.push(TerminalEntry {
            command: "pwd".into(),
            output: "/home".into(),
            is_error: false,
        });
        assert_eq!(app.sessions[0].terminal_history.len(), 2);
        app.update(Message::TerminalClear);
        assert!(app.sessions[0].terminal_history.is_empty());
    }

    #[test]
    fn b4_terminal_cmd_history_builds() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        // No sftp connection, so commands go to "not connected" path
        app.update(Message::TerminalInputChanged("cmd1".into()));
        app.update(Message::TerminalSubmit);
        app.update(Message::TerminalInputChanged("cmd2".into()));
        app.update(Message::TerminalSubmit);
        assert_eq!(app.sessions[0].terminal_cmd_history, vec!["cmd1", "cmd2"]);
    }

    #[test]
    fn b4_terminal_submit_clears_input() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.update(Message::TerminalInputChanged("df -h".into()));
        assert_eq!(app.sessions[0].terminal_input, "df -h");
        app.update(Message::TerminalSubmit);
        assert!(app.sessions[0].terminal_input.is_empty());
    }

    #[test]
    fn b4_terminal_running_blocks_submit() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        app.sessions[0].terminal_running = true;
        app.update(Message::TerminalInputChanged("ls".into()));
        app.update(Message::TerminalSubmit);
        // Should not have added to history because terminal_running is true
        assert!(app.sessions[0].terminal_history.is_empty());
        // Input should still be there (not cleared)
        assert_eq!(app.sessions[0].terminal_input, "ls");
    }

    #[test]
    fn b4_terminal_multiple_outputs() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        // Simulate multiple command executions without connection
        for cmd in &["ls", "pwd", "whoami"] {
            app.update(Message::TerminalInputChanged(cmd.to_string()));
            app.update(Message::TerminalSubmit);
        }
        assert_eq!(app.sessions[0].terminal_history.len(), 3);
        assert_eq!(app.sessions[0].terminal_history[0].command, "ls");
        assert_eq!(app.sessions[0].terminal_history[1].command, "pwd");
        assert_eq!(app.sessions[0].terminal_history[2].command, "whoami");
    }

    #[test]
    fn b4_f4_toggles_terminal() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Browser;
        assert!(!app.sessions[0].show_terminal);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F4),
            Modifiers::default(),
        ));
        assert!(app.sessions[0].show_terminal);
        app.update(Message::KeyPressed(
            Key::Named(keyboard::key::Named::F4),
            Modifiers::default(),
        ));
        assert!(!app.sessions[0].show_terminal);
    }

    #[test]
    fn b4_terminal_default_state() {
        let app = App::default();
        assert!(!app.sessions[0].show_terminal);
        assert!(app.sessions[0].terminal_input.is_empty());
        assert!(app.sessions[0].terminal_history.is_empty());
        assert!(!app.sessions[0].terminal_running);
        assert!(app.sessions[0].terminal_cmd_history.is_empty());
        assert!(app.sessions[0].terminal_history_idx.is_none());
    }

    #[test]
    fn b4_terminal_input_resets_history_idx() {
        let mut app = App::default();
        app.sessions[0].terminal_history_idx = Some(2);
        app.update(Message::TerminalInputChanged("new input".into()));
        assert!(app.sessions[0].terminal_history_idx.is_none());
    }

    // ── Batch 5: Quality of Life ──────────────────────────────────

    #[test]
    fn b5_quick_connect_applies_profile_and_connects() {
        let mut app = App::default();
        app.sessions[0].connect_state.profiles.push(crate::core::connection::ConnectionProfile {
            name: "test-srv".into(),
            host: "10.0.0.1".into(),
            port: 2222,
            username: "deploy".into(),
            auth: crate::core::connection::AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        });
        app.update(Message::Connect(ConnectMessage::QuickConnect("test-srv".into())));
        assert_eq!(app.sessions[0].connect_state.host, "10.0.0.1");
        assert_eq!(app.sessions[0].connect_state.port, "2222");
        assert_eq!(app.sessions[0].connect_state.username, "deploy");
        // Should be in connecting state (connection will fail without real server)
        assert_eq!(app.sessions[0].screen, Screen::Connecting);
    }

    #[test]
    fn b5_delete_profile() {
        let mut app = App::default();
        // Clear any loaded profiles first
        app.sessions[0].connect_state.profiles.clear();
        app.sessions[0].connect_state.profiles.push(crate::core::connection::ConnectionProfile {
            name: "to-delete".into(),
            host: "host".into(),
            port: 22,
            username: "user".into(),
            auth: crate::core::connection::AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        });
        app.sessions[0].connect_state.profiles.push(crate::core::connection::ConnectionProfile {
            name: "keep".into(),
            host: "other".into(),
            port: 22,
            username: "user".into(),
            auth: crate::core::connection::AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        });
        assert_eq!(app.sessions[0].connect_state.profiles.len(), 2);
        app.update(Message::Connect(ConnectMessage::DeleteProfile("to-delete".into())));
        assert_eq!(app.sessions[0].connect_state.profiles.len(), 1);
        assert_eq!(app.sessions[0].connect_state.profiles[0].name, "keep");
    }

    #[test]
    fn b5_delete_profile_clears_selection() {
        let mut app = App::default();
        app.sessions[0].connect_state.profiles.clear();
        app.sessions[0].connect_state.profiles.push(crate::core::connection::ConnectionProfile {
            name: "srv".into(),
            host: "host".into(),
            port: 22,
            username: "user".into(),
            auth: crate::core::connection::AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        });
        app.sessions[0].connect_state.selected_profile = Some("srv".into());
        app.update(Message::Connect(ConnectMessage::DeleteProfile("srv".into())));
        assert!(app.sessions[0].connect_state.profiles.is_empty());
        assert!(app.sessions[0].connect_state.selected_profile.is_none());
    }

    #[test]
    fn b5_chmod_dialog_opens() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].remote_selected.insert(2); // main.rs
        app.update(Message::RemoteChmod);
        assert!(matches!(app.sessions[0].dialog, Dialog::Chmod(_, _)));
        if let Dialog::Chmod(ref path, ref octal) = app.sessions[0].dialog {
            assert!(path.contains("main.rs"));
            // Should have parsed permissions from entry
            assert!(!octal.is_empty());
        }
    }

    #[test]
    fn b5_chmod_value_changes() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Chmod("/test".into(), "644".into());
        app.update(Message::ChmodValueChanged("755".into()));
        if let Dialog::Chmod(_, ref octal) = app.sessions[0].dialog {
            assert_eq!(octal, "755");
        } else {
            panic!("Dialog should still be Chmod");
        }
    }

    #[test]
    fn b5_chmod_cancel() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Chmod("/test".into(), "644".into());
        app.update(Message::ChmodCancel);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b5_chmod_confirm_no_connection() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Chmod("/test".into(), "755".into());
        app.update(Message::ChmodConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b5_chmod_invalid_octal() {
        let mut app = App::default();
        app.sessions[0].dialog = Dialog::Chmod("/test".into(), "xyz".into());
        app.update(Message::ChmodConfirm);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
        assert!(app.sessions[0].status.contains("Invalid octal"));
    }

    #[test]
    fn b5_chmod_dotdot_ignored() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].remote_selected.clear();
        app.sessions[0].remote_selected.insert(0); // ".."
        app.update(Message::RemoteChmod);
        assert_eq!(app.sessions[0].dialog, Dialog::None);
    }

    #[test]
    fn b5_chmod_context_menu() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].context_menu = ContextMenu::Remote(2);
        app.update(Message::ShowContextMenu(ContextAction::Chmod));
        assert!(matches!(app.sessions[0].dialog, Dialog::Chmod(_, _)));
        assert_eq!(app.sessions[0].context_menu, ContextMenu::None);
    }

    #[test]
    fn b5_home_button_local() {
        let mut app = make_browser_app_with_entries();
        let _original_path = app.sessions[0].local_path.clone();
        app.update(Message::LocalPanel(PanelMessage::GoHome));
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        assert_eq!(app.sessions[0].local_path, home);
        assert!(app.sessions[0].local_selected.is_empty());
    }

    #[test]
    fn b5_connection_timer_default_none() {
        let app = App::default();
        assert!(app.sessions[0].connected_at.is_none());
    }

    #[test]
    fn b5_disconnect_clears_terminal() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].show_terminal = true;
        app.sessions[0].terminal_history.push(TerminalEntry {
            command: "ls".into(),
            output: "files".into(),
            is_error: false,
        });
        app.sessions[0].terminal_input = "pwd".into();
        app.sessions[0].terminal_cmd_history.push("ls".into());
        app.sessions[0].connected_at = Some(Instant::now());
        app.update(Message::Disconnect);
        assert!(!app.sessions[0].show_terminal);
        assert!(app.sessions[0].terminal_history.is_empty());
        assert!(app.sessions[0].terminal_input.is_empty());
        assert!(app.sessions[0].terminal_cmd_history.is_empty());
        assert!(app.sessions[0].connected_at.is_none());
    }

    #[test]
    fn b5_format_permissions_roundtrip_in_chmod() {
        // Test that the octal parsing in RemoteChmod correctly converts rwx string
        let mut app = make_browser_app_with_entries();
        // Entry at index 2 is main.rs with permissions "rw-r--r--" (644)
        app.sessions[0].remote_selected.insert(2);
        app.update(Message::RemoteChmod);
        if let Dialog::Chmod(_, ref octal) = app.sessions[0].dialog {
            assert_eq!(octal, "644");
        }
    }

    // --- Drag & Drop Tests ---

    #[test]
    fn dnd_hover_sets_flag() {
        let mut app = App::default();
        assert!(!app.sessions[0].drop_hovering);
        app.update(Message::DragHovering(true));
        assert!(app.sessions[0].drop_hovering);
        app.update(Message::DragHovering(false));
        assert!(!app.sessions[0].drop_hovering);
    }

    #[test]
    fn dnd_drop_clears_hover() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].drop_hovering = true;
        app.update(Message::FilesDropped(vec![PathBuf::from("/tmp/test.txt")]));
        assert!(!app.sessions[0].drop_hovering);
    }

    #[test]
    fn dnd_drop_without_connection_shows_status() {
        let mut app = make_browser_app_with_entries();
        // No sftp connection set (default)
        app.update(Message::FilesDropped(vec![PathBuf::from("/tmp/test.txt")]));
        assert!(app.sessions[0].status.contains("Connect to a server"));
        assert!(!app.sessions[0].transfer_active);
    }

    #[test]
    fn dnd_drop_on_connect_screen_shows_status() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Connect;
        app.update(Message::FilesDropped(vec![PathBuf::from("/tmp/test.txt")]));
        assert!(app.sessions[0].status.contains("Connect to a server"));
        assert!(!app.sessions[0].transfer_active);
    }

    #[test]
    fn dnd_drop_during_transfer_queues_files() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].transfer_active = true;
        let paths = vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b.txt")];
        app.update(Message::FilesDropped(paths));
        assert_eq!(app.sessions[0].drop_pending.len(), 2);
        assert!(app.sessions[0].status.contains("queued"));
    }

    #[test]
    fn dnd_multiple_queued_batches_accumulate() {
        let mut app = make_browser_app_with_entries();
        app.sessions[0].transfer_active = true;
        app.update(Message::FilesDropped(vec![PathBuf::from("/tmp/a.txt")]));
        app.update(Message::FilesDropped(vec![PathBuf::from("/tmp/b.txt")]));
        assert_eq!(app.sessions[0].drop_pending.len(), 2);
    }

    #[test]
    fn dnd_hover_visible_on_any_screen() {
        let mut app = App::default();
        app.sessions[0].screen = Screen::Connect;
        app.update(Message::DragHovering(true));
        assert!(app.sessions[0].drop_hovering);
    }

    // ===== TAB MANAGEMENT TESTS =====

    #[test]
    fn tab_new_tab_adds_session() {
        let mut app = App::default();
        assert_eq!(app.sessions.len(), 1);
        app.update(Message::NewTab);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_tab, 1);
        // New session should be on connect screen
        assert_eq!(app.sessions[1].screen, Screen::Connect);
        assert_eq!(app.sessions[1].label, "New Connection");
    }

    #[test]
    fn tab_close_tab_removes_session() {
        let mut app = App::default();
        app.update(Message::NewTab);
        app.update(Message::NewTab);
        assert_eq!(app.sessions.len(), 3);
        app.update(Message::CloseTab(1));
        assert_eq!(app.sessions.len(), 2);
    }

    #[test]
    fn tab_close_last_tab_creates_fresh() {
        let mut app = App::default();
        app.update(Message::CloseTab(0));
        // Should still have one session (fresh one created)
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_tab, 0);
        assert_eq!(app.sessions[0].screen, Screen::Connect);
    }

    #[test]
    fn tab_switch_tab() {
        let mut app = App::default();
        app.update(Message::NewTab);
        app.update(Message::NewTab);
        assert_eq!(app.active_tab, 2);
        app.update(Message::SwitchTab(0));
        assert_eq!(app.active_tab, 0);
        app.update(Message::SwitchTab(1));
        assert_eq!(app.active_tab, 1);
    }

    #[test]
    fn tab_next_prev_wraps() {
        let mut app = App::default();
        app.update(Message::NewTab);
        app.update(Message::NewTab);
        app.update(Message::SwitchTab(0));
        assert_eq!(app.active_tab, 0);
        app.update(Message::PrevTab);
        assert_eq!(app.active_tab, 2);
        app.update(Message::NextTab);
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn tab_close_adjusts_active_tab() {
        let mut app = App::default();
        app.update(Message::NewTab);
        app.update(Message::NewTab);
        // Active is 2 (last)
        app.update(Message::CloseTab(2));
        // Should adjust to 1
        assert_eq!(app.active_tab, 1);
        assert_eq!(app.sessions.len(), 2);
    }

    #[test]
    fn tab_sessions_are_independent() {
        let mut app = App::default();
        app.update(Message::NewTab);
        // Modify session 0
        app.update(Message::SwitchTab(0));
        app.sessions[0].status = "Tab 0 status".to_string();
        // Switch to session 1
        app.update(Message::SwitchTab(1));
        // Session 1 should have its own status
        assert_ne!(app.sessions[1].status, "Tab 0 status");
    }

    #[test]
    fn tab_unique_ids() {
        let mut app = App::default();
        let id0 = app.sessions[0].id;
        app.update(Message::NewTab);
        let id1 = app.sessions[1].id;
        assert_ne!(id0, id1);
        app.update(Message::NewTab);
        let id2 = app.sessions[2].id;
        assert_ne!(id1, id2);
    }

    #[test]
    fn tab_switch_out_of_bounds_noop() {
        let mut app = App::default();
        app.update(Message::SwitchTab(99));
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn tab_close_out_of_bounds_noop() {
        let mut app = App::default();
        let count = app.sessions.len();
        app.update(Message::CloseTab(99));
        assert_eq!(app.sessions.len(), count);
    }
}
