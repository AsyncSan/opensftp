use iced::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    Dark,
    Light,
    Nord,
    Dracula,
    SolarizedDark,
    GruvboxDark,
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
            Self::SolarizedDark => Theme::SolarizedDark,
            Self::GruvboxDark => Theme::GruvboxDark,
        }
    }

    pub fn all() -> &'static [AppTheme] {
        &[Self::Dark, Self::Light, Self::Nord, Self::Dracula, Self::SolarizedDark, Self::GruvboxDark]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Nord => "Nord",
            Self::Dracula => "Dracula",
            Self::SolarizedDark => "Solarized",
            Self::GruvboxDark => "Gruvbox",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dark() {
        assert_eq!(AppTheme::default(), AppTheme::Dark);
    }

    #[test]
    fn all_returns_six_themes() {
        let themes = AppTheme::all();
        assert_eq!(themes.len(), 6);
        assert!(themes.contains(&AppTheme::Dark));
        assert!(themes.contains(&AppTheme::Light));
        assert!(themes.contains(&AppTheme::Nord));
        assert!(themes.contains(&AppTheme::Dracula));
        assert!(themes.contains(&AppTheme::SolarizedDark));
        assert!(themes.contains(&AppTheme::GruvboxDark));
    }

    #[test]
    fn theme_names() {
        assert_eq!(AppTheme::Dark.name(), "Dark");
        assert_eq!(AppTheme::Light.name(), "Light");
        assert_eq!(AppTheme::Nord.name(), "Nord");
        assert_eq!(AppTheme::Dracula.name(), "Dracula");
        assert_eq!(AppTheme::SolarizedDark.name(), "Solarized");
        assert_eq!(AppTheme::GruvboxDark.name(), "Gruvbox");
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
}
