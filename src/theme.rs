use gpui::{App, Global, Window, WindowAppearance};

mod colors;
mod dark;
mod light;

pub use crate::ui::sp;
pub use colors::Theme;
use serde::Deserialize;
use std::{fs, io, path::Path};

pub use michelle_client::theme::ThemePreference;

fn resolves_to_dark(preference: ThemePreference, system_appearance: WindowAppearance) -> bool {
    match preference {
        ThemePreference::System => matches!(
            system_appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
        ThemePreference::Light => false,
        ThemePreference::Dark => true,
    }
}

fn native_override(preference: ThemePreference) -> Option<bool> {
    match preference {
        ThemePreference::System => None,
        ThemePreference::Light => Some(false),
        ThemePreference::Dark => Some(true),
    }
}

/// One file supplies both complete color schemes. The filename is the stable
/// settings key; changing the display name does not lose the selection.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeDefinition {
    #[serde(skip)]
    pub file: Option<String>,
    pub name: String,
    pub light: Theme,
    pub dark: Theme,
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self {
            file: None,
            name: "Michelle".into(),
            light: Theme::light(),
            dark: Theme::dark(),
        }
    }
}

impl ThemeDefinition {
    fn parse(source: &str) -> anyhow::Result<Self> {
        let mut theme: Self = toml::from_str(source)?;
        theme.name = theme.name.trim().to_owned();
        anyhow::ensure!(!theme.name.is_empty(), "theme name must not be empty");
        theme.dark.is_dark = true;
        Ok(theme)
    }

    fn resolve(&self, preference: ThemePreference, appearance: WindowAppearance) -> Theme {
        if resolves_to_dark(preference, appearance) {
            self.dark
        } else {
            self.light
        }
    }
}

/// Called once on a background worker at startup; rendering reads the result.
pub fn load_themes() -> Vec<ThemeDefinition> {
    let loaded = dirs::data_dir()
        .ok_or_else(|| io::Error::other("Application Support directory is unavailable"))
        .and_then(|directory| load_themes_from_directory(&directory.join("Michelle/themes")));
    match loaded {
        Ok(themes) => themes,
        Err(error) => {
            eprintln!("could not load themes: {error}");
            vec![ThemeDefinition::default()]
        }
    }
}

fn load_themes_from_directory(directory: &Path) -> io::Result<Vec<ThemeDefinition>> {
    fs::create_dir_all(directory)?;
    let mut themes = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml")
            || !entry.file_type()?.is_file()
        {
            continue;
        }
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        match fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|source| ThemeDefinition::parse(&source))
        {
            Ok(mut theme) => {
                theme.file = Some(file.to_owned());
                themes.push(theme);
            }
            Err(error) => eprintln!("could not load theme {}: {error}", path.display()),
        }
    }
    themes.sort_by(|left, right| left.name.cmp(&right.name).then(left.file.cmp(&right.file)));
    themes.insert(0, ThemeDefinition::default());
    Ok(themes)
}

#[derive(Clone, Copy)]
struct ActiveTheme(Theme);
impl Global for ActiveTheme {}

impl Theme {
    pub fn current(cx: &App) -> Self {
        cx.try_global::<ActiveTheme>()
            .map_or_else(Self::dark, |active| active.0)
    }

    pub fn generation(cx: &App) -> u64 {
        crate::ui::appearance::generation(cx)
    }
}

fn set_theme(theme: Theme, cx: &mut App) {
    crate::ui::appearance::set_colors(theme.ui_colors(), cx);
    cx.set_global(ActiveTheme(theme));
}

/// Resolve and publish the startup theme, before any window exists.
pub fn init(cx: &mut App) {
    let system_appearance = cx.window_appearance();
    let theme = if resolves_to_dark(ThemePreference::System, system_appearance) {
        Theme::dark()
    } else {
        Theme::light()
    };
    set_theme(theme, cx);
}

pub fn apply_theme_preference(
    preference: ThemePreference,
    definition: &ThemeDefinition,
    window: &mut Window,
    cx: &mut App,
) {
    crate::platform::set_window_appearance(window, native_override(preference));
    let theme = definition.resolve(preference, cx.window_appearance());
    set_theme(theme, cx);
    crate::platform::configure_sidebar_material(window, theme.is_dark, theme.sidebar);
    window.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    const EXAMPLE: &str = include_str!("../docs/themes/example.toml");

    #[gpui::test]
    fn custom_themes_publish_app_and_control_colors_together(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert_eq!(Theme::current(cx), Theme::dark());
            let definition = ThemeDefinition::parse(EXAMPLE).unwrap();
            let mut theme = definition.light;
            theme.text_secondary = rgb(0x112233).into();
            theme.sidebar_item_background = rgb(0x223344).into();
            theme.toolbar_button_bg = rgb(0x334455).into();
            theme.toolbar_button_border = rgb(0x445566).into();
            theme.composer = rgb(0x556677).into();

            set_theme(theme, cx);
            let generation = Theme::generation(cx);
            let colors = crate::ui::colors(cx);
            assert_eq!(Theme::current(cx), theme);
            assert_eq!(colors, theme.ui_colors());
            assert_eq!(colors.text_secondary, theme.text_secondary);
            assert_eq!(colors.row_selection, theme.sidebar_item_background);
            assert_eq!(colors.control_fill, theme.toolbar_button_bg);
            assert_eq!(colors.control_border, theme.toolbar_button_border);

            set_theme(definition.dark, cx);
            assert_eq!(Theme::current(cx), definition.dark);
            assert_eq!(crate::ui::colors(cx), definition.dark.ui_colors());
            assert_ne!(Theme::generation(cx), generation);
        });
    }

    #[test]
    fn complete_palettes_resolve_independently_of_system_appearance() {
        let theme = ThemeDefinition::parse(EXAMPLE).unwrap();
        assert_eq!(theme.name, "Example");
        assert!(!theme.light.is_dark);
        assert!(theme.dark.is_dark);
        assert_eq!(theme.light.canvas, rgb(0xF6F5F6).into());
        assert_eq!(theme.dark.canvas, rgb(0x1A1A1A).into());
        assert!((theme.dark.sidebar.a - 235.0 / 255.0).abs() < f32::EPSILON);
        for appearance in [
            WindowAppearance::Light,
            WindowAppearance::Dark,
            WindowAppearance::VibrantDark,
        ] {
            assert_eq!(
                theme.resolve(ThemePreference::Light, appearance),
                theme.light
            );
            assert_eq!(theme.resolve(ThemePreference::Dark, appearance), theme.dark);
            assert_eq!(
                theme.resolve(ThemePreference::System, appearance).is_dark,
                appearance != WindowAppearance::Light,
            );
        }
    }

    #[test]
    fn incomplete_or_invalid_themes_are_rejected() {
        assert!(ThemeDefinition::parse(EXAMPLE.split("[dark]").next().unwrap()).is_err());
        for source in [
            EXAMPLE.replacen("text = \"#242424\"\n", "", 1),
            EXAMPLE.replacen("text = \"#E2E2E2\"\n", "", 1),
            EXAMPLE.replace("#0091FF", "#GGGGGG"),
            EXAMPLE.replace("name = \"Example\"", "name = \" \""),
            EXAMPLE.replace("[dark]", "[dark]\nunknown_color = \"#FFFFFF\""),
            EXAMPLE.replace("[light]", "[light]\nis_dark = true"),
        ] {
            assert!(ThemeDefinition::parse(&source).is_err());
        }
    }

    #[test]
    fn startup_catalog_keeps_michelle_and_only_loads_valid_toml_files() {
        let directory =
            std::env::temp_dir().join(format!("michelle-themes-{}", uuid::Uuid::new_v4()));
        let themes = load_themes_from_directory(&directory).unwrap();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "Michelle");
        assert_eq!(themes[0].file, None);

        fs::write(directory.join("incomplete.toml"), "name = 'Incomplete'").unwrap();
        fs::write(directory.join("ignored.txt"), EXAMPLE).unwrap();
        fs::create_dir(directory.join("directory.toml")).unwrap();
        assert_eq!(load_themes_from_directory(&directory).unwrap().len(), 1);

        fs::write(directory.join("custom.toml"), EXAMPLE).unwrap();
        fs::write(
            directory.join("alpha.toml"),
            EXAMPLE.replace("Example", "Alpha"),
        )
        .unwrap();
        let themes = load_themes_from_directory(&directory).unwrap();
        assert_eq!(themes.len(), 3);
        assert_eq!(themes[0].name, "Michelle");
        assert_eq!(themes[1].name, "Alpha");
        assert_eq!(themes[2].file.as_deref(), Some("custom.toml"));
        fs::remove_dir_all(directory).unwrap();
    }
}
