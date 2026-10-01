//! Shared locale selection and translation access.

use serde::{Deserialize, Serialize};

/// The language preference Michelle persists. `System` resolves to one of the
/// locales Michelle deliberately ships today.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppLanguage {
    // Removed locales fall back without invalidating the rest of the settings.
    #[serde(alias = "simplified-chinese", alias = "japanese")]
    System,
    English,
    Russian,
}

impl AppLanguage {
    pub const ALL: [Self; 3] = [Self::System, Self::English, Self::Russian];

    pub fn locale(self) -> &'static str {
        match self.resolved() {
            Self::System => unreachable!("system language always resolves to a shipped locale"),
            Self::English => "en",
            Self::Russian => "ru",
        }
    }

    /// Explicit language names are autonyms so the selector remains
    /// understandable even when the current locale is unfamiliar.
    pub fn label(self) -> String {
        match self {
            Self::System => translate("language.system"),
            Self::English => "English".to_owned(),
            Self::Russian => "Русский".to_owned(),
        }
    }

    pub fn resolved(self) -> Self {
        match self {
            Self::System => Self::from_system(),
            explicit => explicit,
        }
    }

    fn from_system() -> Self {
        Self::from_locale_id(&system_locale())
    }

    fn from_locale_id(locale: &str) -> Self {
        let locale = locale.replace('_', "-").to_ascii_lowercase();
        if locale == "ru" || locale.starts_with("ru-") {
            Self::Russian
        } else {
            Self::English
        }
    }
}

impl Default for AppLanguage {
    fn default() -> Self {
        Self::System
    }
}

pub fn set_language(language: AppLanguage) {
    rust_i18n::set_locale(language.locale());
}

pub fn translate(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

pub fn is_russian() -> bool {
    &*rust_i18n::locale() == "ru"
}

#[cfg(target_os = "macos")]
fn system_locale() -> String {
    use objc2_foundation::NSLocale;

    NSLocale::preferredLanguages()
        .firstObject()
        .map(|locale| locale.to_string())
        .unwrap_or_else(|| "en".to_owned())
}

#[cfg(not(target_os = "macos"))]
fn system_locale() -> String {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| "en".to_owned())
        .split('.')
        .next()
        .unwrap_or("en")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_locale_ids_are_supported() {
        assert_eq!(AppLanguage::English.locale(), "en");
        assert_eq!(AppLanguage::Russian.locale(), "ru");
        let locales = rust_i18n::available_locales!();
        assert_eq!(locales.len(), 2);
        assert!(locales.iter().any(|locale| locale.as_ref() == "en"));
        assert!(locales.iter().any(|locale| locale.as_ref() == "ru"));
    }

    #[test]
    fn language_names_are_autonyms() {
        assert_eq!(AppLanguage::English.label(), "English");
        assert_eq!(AppLanguage::Russian.label(), "Русский");
    }

    #[test]
    fn system_is_the_default_persisted_preference_and_resolves_to_a_shipped_locale() {
        assert_eq!(AppLanguage::default(), AppLanguage::System);
        assert_eq!(
            serde_json::to_string(&AppLanguage::System).unwrap(),
            r#""system""#
        );
        assert!(matches!(AppLanguage::System.locale(), "en" | "ru"));
    }

    #[test]
    fn russian_system_locales_are_detected() {
        for locale in ["ru", "ru_RU", "ru-RU", "RU-by"] {
            assert_eq!(AppLanguage::from_locale_id(locale), AppLanguage::Russian);
        }
        for locale in [
            "en-US",
            "zh-CN",
            "zh-Hans-CN",
            "zh_SG",
            "zh-Hant-TW",
            "ja",
            "ja_JP",
        ] {
            assert_eq!(AppLanguage::from_locale_id(locale), AppLanguage::English);
        }
    }

    #[test]
    fn language_preferences_round_trip_and_removed_locales_fall_back_to_system() {
        for language in AppLanguage::ALL {
            let json = serde_json::to_string(&language).unwrap();
            assert_eq!(
                serde_json::from_str::<AppLanguage>(&json).unwrap(),
                language
            );
        }
        assert_eq!(
            serde_json::to_string(&AppLanguage::Russian).unwrap(),
            r#""russian""#
        );
        for removed in ["simplified-chinese", "japanese"] {
            assert_eq!(
                serde_json::from_value::<AppLanguage>(serde_json::json!(removed)).unwrap(),
                AppLanguage::System
            );
        }
    }

    #[test]
    fn translations_are_complete_and_interpolate_naturally() {
        assert_eq!(&*rust_i18n::t!("settings.daemon", locale = "en"), "Daemon");
        assert_eq!(
            &*rust_i18n::t!("daemon.expose_title", locale = "en"),
            "Expose managed daemon"
        );
        assert_eq!(
            &*rust_i18n::t!("settings.general", locale = "ru"),
            "Основные"
        );
        assert_eq!(
            &*rust_i18n::t!("computer_use.allow_control", locale = "ru", app = "Finder"),
            "Разрешить Michelle управлять Finder?"
        );
        assert_eq!(
            &*rust_i18n::t!("session.rewound", locale = "ru", turn = 3),
            "Выполнен откат к началу шага 3"
        );
    }

    #[test]
    fn task_creation_copy_uses_task_terminology() {
        assert_eq!(&*rust_i18n::t!("menu.new_task", locale = "en"), "New Task");
        assert_eq!(
            &*rust_i18n::t!("command_palette.new_task", locale = "en"),
            "New task"
        );
        assert_eq!(
            &*rust_i18n::t!("providers.disabled_for_new_tasks", locale = "en"),
            "Disabled for new tasks"
        );
    }
}
