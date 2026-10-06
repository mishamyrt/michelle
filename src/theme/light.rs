use gpui::{Hsla, hsla, rgb};

use super::Theme;
use crate::ui::Palette;

pub(super) fn theme() -> Theme {
    let palette = Palette::light();
    Theme {
        is_dark: false,
        canvas: rgb(0xF6F5F6).into(),
        sidebar: Hsla::from(rgb(0xF4F4F4)).opacity(0.85),
        sidebar_drag_background: rgb(0xFAFAFA).into(),
        sidebar_item_background: palette.fills.tertiary,
        surface: rgb(0xFFFFFF).into(),
        raised: rgb(0xFFFFFF).into(),
        composer: rgb(0xF8F8F8).into(),
        inset: rgb(0xE6E6E6).into(),
        terminal: rgb(0xFFFFFF).into(),
        overlay: palette.fills.quaternary,
        overlay_strong: palette.fills.primary,

        border: palette.separators.standard,
        border_strong: palette.separators.strong,
        sidebar_border: hsla(0.0, 0.0, 0.86, 1.0),

        toolbar_button_bg: hsla(0.0, 0.0, 0.97, 0.7),
        toolbar_button_border: hsla(0.0, 0.0, 0.1, 0.12),

        text: palette.labels.primary,
        text_secondary: palette.labels.secondary,
        text_tertiary: palette.labels.tertiary,
        text_ghost: palette.labels.quaternary,

        accent: palette.colors.blue,
        resize_handle: palette.colors.blue_deep,
        gauge: palette.colors.blue_deep,

        selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.35),
        code_text: palette.colors.green_bright,
        code_wash: palette.fills.secondary,

        inverse: rgb(0x202227).into(),
        on_inverse: rgb(0xF8F8F9).into(),

        warning: palette.colors.orange,
        success: palette.colors.green,
        favorite: palette.colors.yellow,
        danger: palette.colors.red,
        danger_soft: hsla(4.0 / 360.0, 0.55, 0.52, 0.10),
    }
}
