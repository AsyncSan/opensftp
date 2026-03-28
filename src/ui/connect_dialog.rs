use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Element, Length};

use crate::core::connection::{AuthMethod, ConnectionProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    Password,
    KeyFile,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password => write!(f, "Password"),
            Self::KeyFile => write!(f, "Key File"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectMessage {
    HostChanged(String),
    PortChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    AuthTypeSelected(AuthType),
    KeyPathChanged(String),
    BrowseKeyFile,
    KeyFileChosen(Option<String>),
    Connect,
    Cancel,
    ProfileSelected(String),
}

#[derive(Debug, Clone)]
pub struct ConnectState {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub auth_type: AuthType,
    pub key_path: String,
    pub profiles: Vec<ConnectionProfile>,
    pub selected_profile: Option<String>,
}

impl Default for ConnectState {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: "22".to_string(),
            username: String::new(),
            password: String::new(),
            auth_type: AuthType::Password,
            key_path: String::new(),
            profiles: Vec::new(),
            selected_profile: None,
        }
    }
}

impl ConnectState {
    pub fn new() -> Self {
        let profiles = ConnectionProfile::load_all();

        // Default key path if ~/.ssh/id_ed25519 exists
        let default_key = dirs::home_dir()
            .map(|h| h.join(".ssh").join("id_ed25519"))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            profiles,
            key_path: default_key,
            ..Default::default()
        }
    }

    pub fn apply_profile(&mut self, name: &str) {
        if let Some(p) = self.profiles.iter().find(|p| p.name == name) {
            self.host = p.host.clone();
            self.port = p.port.to_string();
            self.username = p.username.clone();
            self.selected_profile = Some(name.to_string());

            match &p.auth {
                AuthMethod::Password(pw) => {
                    self.auth_type = AuthType::Password;
                    self.password = pw.clone();
                }
                AuthMethod::KeyFile(path) => {
                    self.auth_type = AuthType::KeyFile;
                    self.key_path = path.to_string_lossy().to_string();
                }
                AuthMethod::Agent => {
                    self.auth_type = AuthType::Password;
                }
            }
        }
    }

    /// Build the AuthMethod from the current UI state.
    pub fn auth_method(&self) -> AuthMethod {
        match self.auth_type {
            AuthType::Password => AuthMethod::Password(self.password.clone()),
            AuthType::KeyFile => AuthMethod::KeyFile(self.key_path.clone().into()),
        }
    }
}

pub fn view_connect_dialog(state: &ConnectState) -> Element<'_, ConnectMessage> {
    let profile_names: Vec<String> = state.profiles.iter().map(|p| p.name.clone()).collect();

    let auth_selector = row![
        text("Auth:").size(13),
        pick_list(
            vec![AuthType::Password, AuthType::KeyFile],
            Some(state.auth_type),
            ConnectMessage::AuthTypeSelected,
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let auth_fields: Element<'_, ConnectMessage> = match state.auth_type {
        AuthType::Password => {
            text_input("Password", &state.password)
                .on_input(ConnectMessage::PasswordChanged)
                .padding(8)
                .secure(true)
                .into()
        }
        AuthType::KeyFile => {
            row![
                text_input("Key file path", &state.key_path)
                    .on_input(ConnectMessage::KeyPathChanged)
                    .padding(8)
                    .width(Length::Fill),
                button("Browse")
                    .on_press(ConnectMessage::BrowseKeyFile)
                    .padding(8),
            ]
            .spacing(8)
            .into()
        }
    };

    let form = column![
        text("Connect to Server").size(20),
        pick_list(
            profile_names,
            state.selected_profile.clone(),
            ConnectMessage::ProfileSelected,
        )
        .placeholder("Saved connections..."),
        text_input("Host", &state.host)
            .on_input(ConnectMessage::HostChanged)
            .padding(8),
        text_input("Port", &state.port)
            .on_input(ConnectMessage::PortChanged)
            .padding(8),
        text_input("Username", &state.username)
            .on_input(ConnectMessage::UsernameChanged)
            .padding(8),
        auth_selector,
        auth_fields,
        row![
            button("Cancel").on_press(ConnectMessage::Cancel).padding(8),
            button("Connect").on_press(ConnectMessage::Connect).padding(8),
        ]
        .spacing(12),
    ]
    .spacing(12)
    .max_width(400);

    container(form)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(40)
        .into()
}
