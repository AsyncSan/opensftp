use iced::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    Dark,
    Light,
    Nord,
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
        }
    }

    pub fn all() -> &'static [AppTheme] {
        &[Self::Dark, Self::Light, Self::Nord]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Nord => "Nord",
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
    fn all_returns_three_themes() {
        let themes = AppTheme::all();
        assert_eq!(themes.len(), 3);
        assert!(themes.contains(&AppTheme::Dark));
        assert!(themes.contains(&AppTheme::Light));
        assert!(themes.contains(&AppTheme::Nord));
    }

    #[test]
    fn theme_names() {
        assert_eq!(AppTheme::Dark.name(), "Dark");
        assert_eq!(AppTheme::Light.name(), "Light");
        assert_eq!(AppTheme::Nord.name(), "Nord");
    }

    #[test]
    fn to_iced_theme_mapping() {
        // Just verify it doesn't panic
        let _ = AppTheme::Dark.to_iced_theme();
        let _ = AppTheme::Light.to_iced_theme();
        let _ = AppTheme::Nord.to_iced_theme();
    }

    #[test]
    fn theme_equality() {
        assert_eq!(AppTheme::Dark, AppTheme::Dark);
        assert_ne!(AppTheme::Dark, AppTheme::Light);
    }
}
