use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Element, Length, Theme};
use super::icons;

use crate::core::connection::{AuthMethod, ConnectionProfile, JumpHostConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    Password,
    KeyFile,
    Agent,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password => write!(f, "Password"),
            Self::KeyFile => write!(f, "Key File"),
            Self::Agent => write!(f, "SSH Agent"),
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
    JumpHostToggled(bool),
    JumpHostChanged(String),
    JumpPortChanged(String),
    JumpUsernameChanged(String),
    JumpPasswordChanged(String),
    JumpAuthTypeSelected(AuthType),
    JumpKeyPathChanged(String),
    Connect,
    Cancel,
    ProfileSelected(String),
    QuickConnect(String),
    DeleteProfile(String),
    ToggleFavorite(String),
}

#[derive(Debug, Clone)]
pub struct ConnectState {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub auth_type: AuthType,
    pub key_path: String,
    pub use_jump_host: bool,
    pub jump_host: String,
    pub jump_port: String,
    pub jump_username: String,
    pub jump_password: String,
    pub jump_auth_type: AuthType,
    pub jump_key_path: String,
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
            use_jump_host: false,
            jump_host: String::new(),
            jump_port: "22".to_string(),
            jump_username: String::new(),
            jump_password: String::new(),
            jump_auth_type: AuthType::KeyFile,
            jump_key_path: String::new(),
            profiles: Vec::new(),
            selected_profile: None,
        }
    }
}

impl ConnectState {
    pub fn new() -> Self {
        let profiles = ConnectionProfile::load_all();
        let default_key = dirs::home_dir()
            .map(|h| h.join(".ssh").join("id_ed25519"))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            profiles,
            key_path: default_key.clone(),
            jump_key_path: default_key,
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
                AuthMethod::Password(pw) => { self.auth_type = AuthType::Password; self.password = pw.clone(); }
                AuthMethod::KeyFile(path) => { self.auth_type = AuthType::KeyFile; self.key_path = path.to_string_lossy().to_string(); }
                AuthMethod::Agent => { self.auth_type = AuthType::Agent; }
            }

            if let Some(jump) = &p.jump_host {
                self.use_jump_host = true;
                self.jump_host = jump.host.clone();
                self.jump_port = jump.port.to_string();
                self.jump_username = jump.username.clone();
                match &jump.auth {
                    AuthMethod::Password(pw) => { self.jump_auth_type = AuthType::Password; self.jump_password = pw.clone(); }
                    AuthMethod::KeyFile(path) => { self.jump_auth_type = AuthType::KeyFile; self.jump_key_path = path.to_string_lossy().to_string(); }
                    AuthMethod::Agent => { self.jump_auth_type = AuthType::Agent; }
                }
            } else {
                self.use_jump_host = false;
                self.jump_host.clear();
                self.jump_port = "22".to_string();
                self.jump_username.clear();
                self.jump_password.clear();
                self.jump_auth_type = AuthType::KeyFile;
                self.jump_key_path.clear();
            }
        }
    }

    pub fn auth_method(&self) -> AuthMethod {
        match self.auth_type {
            AuthType::Password => AuthMethod::Password(self.password.clone()),
            AuthType::KeyFile => AuthMethod::KeyFile(self.key_path.clone().into()),
            AuthType::Agent => AuthMethod::Agent,
        }
    }

    pub fn jump_auth_method(&self) -> AuthMethod {
        match self.jump_auth_type {
            AuthType::Password => AuthMethod::Password(self.jump_password.clone()),
            AuthType::KeyFile => AuthMethod::KeyFile(self.jump_key_path.clone().into()),
            AuthType::Agent => AuthMethod::Agent,
        }
    }

    pub fn jump_host_config(&self) -> Option<JumpHostConfig> {
        if !self.use_jump_host || self.jump_host.trim().is_empty() {
            return None;
        }
        Some(JumpHostConfig {
            host: self.jump_host.clone(),
            port: self.jump_port.parse().unwrap_or(22),
            username: self.jump_username.clone(),
            auth: self.jump_auth_method(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_host_config_builds_when_enabled() {
        let mut state = ConnectState::default();
        state.use_jump_host = true;
        state.jump_host = "bastion.example.com".into();
        state.jump_port = "2200".into();
        state.jump_username = "jumpuser".into();
        state.jump_auth_type = AuthType::Password;
        state.jump_password = "secret".into();

        let jump = state.jump_host_config().unwrap();
        assert_eq!(jump.host, "bastion.example.com");
        assert_eq!(jump.port, 2200);
        assert_eq!(jump.username, "jumpuser");
        assert!(matches!(jump.auth, AuthMethod::Password(ref pw) if pw == "secret"));
    }

    #[test]
    fn jump_host_config_none_when_disabled() {
        let state = ConnectState::default();
        assert!(state.jump_host_config().is_none());
    }

    #[test]
    fn apply_profile_clears_jump_host_when_absent() {
        let mut state = ConnectState::default();
        state.use_jump_host = true;
        state.jump_host = "old-bastion".into();
        state.jump_port = "2200".into();
        state.jump_username = "olduser".into();
        state.jump_password = "oldpw".into();
        state.profiles.push(ConnectionProfile {
            name: "plain".into(),
            host: "target.example.com".into(),
            port: 22,
            username: "deploy".into(),
            auth: AuthMethod::Agent,
            favorite: false,
            jump_host: None,
        });

        state.apply_profile("plain");

        assert!(!state.use_jump_host);
        assert!(state.jump_host.is_empty());
        assert_eq!(state.jump_port, "22");
        assert!(state.jump_username.is_empty());
        assert!(state.jump_password.is_empty());
    }
}

pub fn view_connect_dialog(state: &ConnectState) -> Element<'_, ConnectMessage> {
    let profile_names: Vec<String> = state.profiles.iter().map(|p| p.name.clone()).collect();

    let quick_cards: Vec<Element<'_, ConnectMessage>> = state.profiles.iter().map(|p| {
        let auth_icon: Element<'_, ConnectMessage> = match &p.auth {
            AuthMethod::Password(_) => icons::lock(11.0).into(),
            AuthMethod::KeyFile(_) => icons::key(11.0).into(),
            AuthMethod::Agent => icons::user(11.0).into(),
        };
        let name = p.name.clone();
        let delete_name = p.name.clone();
        let favorite_name = p.name.clone();
        let fav_icon: Element<'_, ConnectMessage> = if p.favorite { icons::star(11.0).into() } else { icons::star_empty(11.0).into() };
        container(row![
            button(fav_icon).on_press(ConnectMessage::ToggleFavorite(favorite_name)).padding([8, 6]),
            button(column![
                text(&p.name).size(13),
                row![auth_icon, text(format!(" {}@{}:{}", p.username, p.host, p.port)).size(11)].spacing(2).align_y(iced::Alignment::Center),
            ]).on_press(ConnectMessage::QuickConnect(name)).padding([8, 12]).width(Length::Fill),
            button(icons::x(11.0)).on_press(ConnectMessage::DeleteProfile(delete_name)).padding([8, 6]),
        ].spacing(2).align_y(iced::Alignment::Center)).width(Length::Fill).style(|theme: &Theme| container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.2, ..theme.extended_palette().background.strong.color })), border: iced::Border { color: theme.extended_palette().primary.weak.color, width: 1.0, radius: 6.0.into(), }, ..Default::default() }).into()
    }).collect();

    let quick_connect_section: Element<'_, ConnectMessage> = if quick_cards.is_empty() { container(text("")).height(Length::Shrink).into() } else { column![text("Quick Connect").size(13), iced::widget::scrollable(column(quick_cards).spacing(4),).height(Length::Shrink),].spacing(6).into() };

    let auth_selector = row![text("Auth:").size(13), pick_list(vec![AuthType::Password, AuthType::KeyFile, AuthType::Agent], Some(state.auth_type), ConnectMessage::AuthTypeSelected),].spacing(8).align_y(iced::Alignment::Center);

    let auth_fields: Element<'_, ConnectMessage> = match state.auth_type { AuthType::Password => text_input("Password", &state.password).on_input(ConnectMessage::PasswordChanged).padding(8).secure(true).into(), AuthType::KeyFile => row![text_input("Key file path", &state.key_path).on_input(ConnectMessage::KeyPathChanged).padding(8).width(Length::Fill), button("Browse").on_press(ConnectMessage::BrowseKeyFile).padding(8),].spacing(8).into(), AuthType::Agent => text("Uses SSH_AUTH_SOCK agent").size(13).into(), };

    let jump_auth_selector = row![text("Jump auth:").size(13), pick_list(vec![AuthType::Password, AuthType::KeyFile, AuthType::Agent], Some(state.jump_auth_type), ConnectMessage::JumpAuthTypeSelected),].spacing(8).align_y(iced::Alignment::Center);

    let jump_fields: Element<'_, ConnectMessage> = if state.use_jump_host {
        column![
            text_input("Jump host", &state.jump_host).on_input(ConnectMessage::JumpHostChanged).padding(8),
            row![
                text_input("Jump port", &state.jump_port).on_input(ConnectMessage::JumpPortChanged).padding(8).width(Length::Fixed(100.0)),
                text_input("Jump username", &state.jump_username).on_input(ConnectMessage::JumpUsernameChanged).padding(8).width(Length::Fill),
            ].spacing(8),
            jump_auth_selector,
            {let jump_auth_field: Element<'_, ConnectMessage> = match state.jump_auth_type { AuthType::Password => text_input("Jump password", &state.jump_password).on_input(ConnectMessage::JumpPasswordChanged).padding(8).secure(true).into(), AuthType::KeyFile => row![text_input("Jump key file path", &state.jump_key_path).on_input(ConnectMessage::JumpKeyPathChanged).padding(8).width(Length::Fill), button("Browse").on_press(ConnectMessage::BrowseKeyFile).padding(8),].spacing(8).into(), AuthType::Agent => text("Jump uses SSH_AUTH_SOCK agent").size(13).into(), }; jump_auth_field},
        ].spacing(8).into()
    } else { container(text("")).height(Length::Shrink).into() };

    let form = column![
        text("Open SFTP").size(22),
        text("Connect to Server").size(14),
        quick_connect_section,
        pick_list(profile_names, state.selected_profile.clone(), ConnectMessage::ProfileSelected).placeholder("Saved connections..."),
        text_input("Host", &state.host).on_input(ConnectMessage::HostChanged).padding(8),
        row![text_input("Port", &state.port).on_input(ConnectMessage::PortChanged).padding(8).width(Length::Fixed(100.0)), text_input("Username", &state.username).on_input(ConnectMessage::UsernameChanged).padding(8).width(Length::Fill),].spacing(8),
        auth_selector,
        auth_fields,
        row![button(text(if state.use_jump_host { "Jump Host ✓" } else { "Jump Host" }).size(13)).on_press(ConnectMessage::JumpHostToggled(!state.use_jump_host)).padding([8, 16]), button(text("Cancel").size(13)).on_press(ConnectMessage::Cancel).padding([8, 16]), button(text("Connect").size(13)).on_press(ConnectMessage::Connect).padding([8, 16]),].spacing(12),
        jump_fields,
    ]
    .spacing(14)
    .max_width(420);

    let card = container(form).padding(28).style(|theme: &Theme| container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.4, ..theme.extended_palette().background.strong.color })), border: iced::Border { color: theme.extended_palette().primary.weak.color, width: 1.0, radius: 8.0.into(), }, ..Default::default() });

    container(card).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).padding(40).into()
}
