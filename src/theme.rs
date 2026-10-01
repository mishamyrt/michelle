use gpui::{App, Global, Hsla, Rems, Window, WindowAppearance, hsla, rems, rgb, transparent_black};
use serde::Deserialize;
use std::{fs, io, path::Path};

pub use michelle_client::theme::ThemePreference;

/// Scaled pixels: a dimension authored at the default 14px UI font size,
/// expressed in rems so the UI font size setting scales it. The window's rem
/// size *is* the UI font size, so at the default setting this resolves to
/// exactly the authored pixel value.
///
/// Chrome text sizes and their line heights go through here. Content surfaces
/// that already derive from a font-size setting — markdown metrics, the file
/// editor, diff rows, tool-output mono — stay in `px` so they never scale
/// twice.
pub fn sp(value: f32) -> Rems {
    rems(value / michelle_client::persistence::DEFAULT_UI_FONT_SIZE)
}

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

/// Michelle's visual language, take two: neutral graphite surfaces in the spirit
/// of Cursor — color is reserved for meaning. On macOS the sidebar's semantic
/// tint is installed as a native layer above Sidebar vibrancy; keeping this
/// GPUI surface clear avoids incorrectly accumulating the alpha of nested Metal
/// backgrounds. Selected, hovered, and pressed rows remain a 6% neutral layer.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    #[serde(skip)]
    pub is_dark: bool,
    pub canvas: Hsla,
    pub sidebar: Hsla,
    pub sidebar_drag_background: Hsla,
    pub sidebar_item_background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub composer: Hsla,
    pub inset: Hsla,
    /// Terminal screen surface: paper-white in light mode, near-black in dark.
    pub terminal: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,

    pub border: Hsla,
    pub border_strong: Hsla,
    pub sidebar_border: Hsla,

    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,

    /// Brand coral. Logo, caret, live-activity pulses — nothing structural.
    pub accent: Hsla,
    pub resize_handle: Hsla,
    /// Meter fills in the usage panel. Quota-meter blue by convention;
    /// warning/danger take over as a lane fills.
    pub gauge: Hsla,

    /// Text-selection wash. Painted *under* the glyphs, so it stays
    /// translucent and deliberately reads as the familiar browser blue rather
    /// than as brand color.
    pub selection: Hsla,
    /// Inline `code` foreground and its rounded wash.
    pub code_text: Hsla,
    pub code_wash: Hsla,

    /// Light fill for primary buttons (send, allow), dark glyph on top.
    pub inverse: Hsla,
    pub on_inverse: Hsla,

    pub warning: Hsla,
    pub success: Hsla,
    pub favorite: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
}

impl Theme {
    /// The native sidebar layer owns its tint on macOS, so GPUI stays clear.
    pub fn sidebar_surface(self) -> Hsla {
        if cfg!(target_os = "macos") {
            transparent_black()
        } else {
            self.sidebar
        }
    }

    pub fn generation(cx: &App) -> u64 {
        cx.try_global::<ActiveMichelleTheme>()
            .map_or(0, |theme| theme.1)
    }

    pub fn current(cx: &App) -> Self {
        if cx.has_global::<ActiveMichelleTheme>() {
            cx.global::<ActiveMichelleTheme>().0
        } else {
            Self::dark()
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            canvas: rgb(0x1A1A1A).into(),
            sidebar: Hsla::from(rgb(0x181818)).opacity(0.92),
            sidebar_drag_background: rgb(0x181818).into(),
            sidebar_item_background: hsla(0.0, 0.0, 0.941, 0.06),
            surface: rgb(0x1A1A1A).into(),
            raised: rgb(0x232323).into(),
            composer: rgb(0x212121).into(),
            inset: rgb(0x151515).into(),
            terminal: rgb(0x151515).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.90, 0.07),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.14),
            sidebar_border: hsla(126.93 / 360.0, 0.000_000_1, 0.16077, 1.0),

            text: rgb(0xE2E2E2).into(),
            text_secondary: rgb(0xA3A3A3).into(),
            text_tertiary: rgb(0x7D7D7D).into(),
            text_ghost: rgb(0x575757).into(),

            accent: rgb(0x0091FF).into(),
            resize_handle: rgb(0x3B82F6).into(),
            gauge: rgb(0x3B82F6).into(),

            selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.55),
            code_text: rgb(0x82E087).into(),
            code_wash: hsla(220.0 / 360.0, 0.10, 0.90, 0.08),

            inverse: rgb(0xE7E9EC).into(),
            on_inverse: rgb(0x17181C).into(),

            warning: rgb(0xE0B36A).into(),
            success: rgb(0x62C987).into(),
            favorite: rgb(0xEAB308).into(),
            danger: rgb(0xE2726A).into(),
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.63, 0.10),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            canvas: rgb(0xF6F5F6).into(),
            sidebar: Hsla::from(rgb(0xF3F3F3)).opacity(0.92),
            sidebar_drag_background: rgb(0xF3F3F3).into(),
            sidebar_item_background: hsla(0.0, 0.0, 0.078, 0.06),
            surface: rgb(0xF6F5F6).into(),
            raised: rgb(0xECECEC).into(),
            composer: rgb(0xFFFFFF).into(),
            inset: rgb(0xE6E6E6).into(),
            terminal: rgb(0xFFFFFF).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.12, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.12, 0.08),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.15),
            sidebar_border: hsla(0.0, 0.0, 0.078, 0.12),

            text: rgb(0x242424).into(),
            text_secondary: rgb(0x666666).into(),
            text_tertiary: rgb(0x858585).into(),
            text_ghost: rgb(0xA4A4A4).into(),

            accent: rgb(0x0088FF).into(),
            resize_handle: rgb(0x2563EB).into(),
            gauge: rgb(0x2563EB).into(),

            selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.35),
            code_text: rgb(0x289A2E).into(),
            code_wash: hsla(220.0 / 360.0, 0.10, 0.12, 0.07),

            inverse: rgb(0x202227).into(),
            on_inverse: rgb(0xF8F8F9).into(),

            warning: rgb(0xA66B20).into(),
            success: rgb(0x2F8F52).into(),
            favorite: rgb(0xCA8A04).into(),
            danger: rgb(0xC64A42).into(),
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.52, 0.10),
        }
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
struct ActiveMichelleTheme(Theme, u64);

impl Global for ActiveMichelleTheme {}

/// Publish the resolved palette. [`Theme::current`] reads it back from the
/// global, which is how every view gets its colors.
fn set_active_theme(theme: Theme, cx: &mut App) {
    let generation = Theme::generation(cx).wrapping_add(1);
    cx.set_global(ActiveMichelleTheme(theme, generation));
}

/// Resolve and publish the startup palette, before any window exists.
pub fn init(cx: &mut App) {
    let system_appearance = cx.window_appearance();
    let theme = if resolves_to_dark(ThemePreference::System, system_appearance) {
        Theme::dark()
    } else {
        Theme::light()
    };
    set_active_theme(theme, cx);
}

pub fn apply_theme_preference(
    preference: ThemePreference,
    definition: &ThemeDefinition,
    window: &mut Window,
    cx: &mut App,
) {
    crate::platform::set_window_appearance(window, native_override(preference));
    let theme = definition.resolve(preference, cx.window_appearance());
    set_active_theme(theme, cx);
    crate::platform::configure_sidebar_material(window, theme.is_dark, theme.sidebar);
    window.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../docs/themes/example.toml");

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
