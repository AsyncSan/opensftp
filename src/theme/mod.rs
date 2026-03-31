use iced::Theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    Dark,
    Light,
    Nord,
    Dracula,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon,
    Ferra,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::Dark
    }
}

impl AppTheme {
    pub fn to_iced_theme(self) -> Theme {
        match self {
            Self::Dark => Theme::Dark,
            Self::Light => Theme::Light,
            Self::Nord => Theme::Nord,
            Self::Dracula => Theme::Dracula,
            Self::SolarizedLight => Theme::SolarizedLight,
            Self::SolarizedDark => Theme::SolarizedDark,
            Self::GruvboxLight => Theme::GruvboxLight,
            Self::GruvboxDark => Theme::GruvboxDark,
            Self::CatppuccinLatte => Theme::CatppuccinLatte,
            Self::CatppuccinFrappe => Theme::CatppuccinFrappe,
            Self::CatppuccinMacchiato => Theme::CatppuccinMacchiato,
            Self::CatppuccinMocha => Theme::CatppuccinMocha,
            Self::TokyoNight => Theme::TokyoNight,
            Self::TokyoNightStorm => Theme::TokyoNightStorm,
            Self::TokyoNightLight => Theme::TokyoNightLight,
            Self::KanagawaWave => Theme::KanagawaWave,
            Self::KanagawaDragon => Theme::KanagawaDragon,
            Self::KanagawaLotus => Theme::KanagawaLotus,
            Self::Moonfly => Theme::Moonfly,
            Self::Nightfly => Theme::Nightfly,
            Self::Oxocarbon => Theme::Oxocarbon,
            Self::Ferra => Theme::Ferra,
        }
    }

    pub fn all() -> &'static [AppTheme] {
        &[
            Self::Dark, Self::Light,
            Self::Nord, Self::Dracula,
            Self::SolarizedLight, Self::SolarizedDark,
            Self::GruvboxLight, Self::GruvboxDark,
            Self::CatppuccinLatte, Self::CatppuccinFrappe,
            Self::CatppuccinMacchiato, Self::CatppuccinMocha,
            Self::TokyoNight, Self::TokyoNightStorm, Self::TokyoNightLight,
            Self::KanagawaWave, Self::KanagawaDragon, Self::KanagawaLotus,
            Self::Moonfly, Self::Nightfly,
            Self::Oxocarbon, Self::Ferra,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Nord => "Nord",
            Self::Dracula => "Dracula",
            Self::SolarizedLight => "Solarized Light",
            Self::SolarizedDark => "Solarized Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::CatppuccinFrappe => "Catppuccin Frappe",
            Self::CatppuccinMacchiato => "Catppuccin Macchiato",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::TokyoNight => "Tokyo Night",
            Self::TokyoNightStorm => "Tokyo Night Storm",
            Self::TokyoNightLight => "Tokyo Night Light",
            Self::KanagawaWave => "Kanagawa Wave",
            Self::KanagawaDragon => "Kanagawa Dragon",
            Self::KanagawaLotus => "Kanagawa Lotus",
            Self::Moonfly => "Moonfly",
            Self::Nightfly => "Nightfly",
            Self::Oxocarbon => "Oxocarbon",
            Self::Ferra => "Ferra",
        }
    }

    pub fn is_dark(self) -> bool {
        !matches!(
            self,
            Self::Light | Self::SolarizedLight | Self::GruvboxLight
                | Self::CatppuccinLatte | Self::TokyoNightLight | Self::KanagawaLotus
        )
    }

    fn settings_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("eurosftp");
        p.push("settings.json");
        p
    }

    pub fn load_saved() -> Self {
        let path = Self::settings_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .map(|s| s.theme)
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let settings = Settings { theme: *self };
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&path, json);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Settings {
    theme: AppTheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dark() {
        assert_eq!(AppTheme::default(), AppTheme::Dark);
    }

    #[test]
    fn all_returns_22_themes() {
        let themes = AppTheme::all();
        assert_eq!(themes.len(), 22);
        assert!(themes.contains(&AppTheme::Dark));
        assert!(themes.contains(&AppTheme::Light));
        assert!(themes.contains(&AppTheme::CatppuccinMocha));
        assert!(themes.contains(&AppTheme::TokyoNight));
        assert!(themes.contains(&AppTheme::KanagawaWave));
        assert!(themes.contains(&AppTheme::Ferra));
    }

    #[test]
    fn theme_names() {
        assert_eq!(AppTheme::Dark.name(), "Dark");
        assert_eq!(AppTheme::CatppuccinMocha.name(), "Catppuccin Mocha");
        assert_eq!(AppTheme::TokyoNight.name(), "Tokyo Night");
        assert_eq!(AppTheme::KanagawaWave.name(), "Kanagawa Wave");
        assert_eq!(AppTheme::Ferra.name(), "Ferra");
    }

    #[test]
    fn to_iced_theme_mapping() {
        for theme in AppTheme::all() {
            let _ = theme.to_iced_theme();
        }
    }

    #[test]
    fn theme_equality() {
        assert_eq!(AppTheme::Dark, AppTheme::Dark);
        assert_ne!(AppTheme::Dark, AppTheme::Light);
    }

    #[test]
    fn dark_light_classification() {
        assert!(AppTheme::Dark.is_dark());
        assert!(AppTheme::Dracula.is_dark());
        assert!(AppTheme::TokyoNight.is_dark());
        assert!(!AppTheme::Light.is_dark());
        assert!(!AppTheme::SolarizedLight.is_dark());
        assert!(!AppTheme::CatppuccinLatte.is_dark());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let theme = AppTheme::CatppuccinMocha;
        let json = serde_json::to_string(&theme).unwrap();
        let restored: AppTheme = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, theme);
    }

    #[test]
    fn settings_roundtrip() {
        let settings = Settings { theme: AppTheme::TokyoNight };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.theme, AppTheme::TokyoNight);
    }

    #[test]
    fn all_themes_have_names() {
        for theme in AppTheme::all() {
            assert!(!theme.name().is_empty());
        }
    }

    #[test]
    fn settings_path_correct() {
        let path = AppTheme::settings_path();
        assert!(path.to_string_lossy().contains("eurosftp"));
        assert!(path.to_string_lossy().ends_with("settings.json"));
    }
}
