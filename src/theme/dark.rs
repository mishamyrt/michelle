use gpui::{Hsla, hsla, rgb};

use super::Theme;
use crate::ui::Palette;

pub(super) fn theme() -> Theme {
    let palette = Palette::dark();
    Theme {
        is_dark: true,
        canvas: rgb(0x1A1A1A).into(),
        sidebar: Hsla::from(rgb(0x181818)).opacity(0.92),
        sidebar_drag_background: rgb(0x181818).into(),
        sidebar_item_background: palette.fills.tertiary,
        surface: rgb(0x1A1A1A).into(),
        raised: rgb(0x232323).into(),
        composer: rgb(0x212121).into(),
        inset: rgb(0x151515).into(),
        terminal: rgb(0x151515).into(),
        overlay: palette.fills.quaternary,
        overlay_strong: palette.fills.primary,

        border: palette.separators.standard,
        border_strong: palette.separators.strong,
        sidebar_border: hsla(126.93 / 360.0, 0.000_000_1, 0.16077, 1.0),

        toolbar_button_bg: hsla(0.0, 0.0, 1.0, 0.06),
        toolbar_button_border: hsla(0.0, 0.0, 0.72, 0.21),

        text: palette.labels.primary,
        text_secondary: palette.labels.secondary,
        text_tertiary: palette.labels.tertiary,
        text_ghost: palette.labels.quaternary,

        accent: palette.colors.blue_deep,
        resize_handle: palette.colors.blue_deep,
        gauge: palette.colors.blue_deep,

        selection: hsla(211.0 / 360.0, 1.0, 0.50, 0.55),
        code_text: palette.colors.green_bright,
        code_wash: palette.fills.secondary,

        inverse: rgb(0xE7E9EC).into(),
        on_inverse: rgb(0x17181C).into(),

        warning: palette.colors.orange,
        success: palette.colors.green,
        favorite: palette.colors.yellow,
        danger: palette.colors.red,
        danger_soft: hsla(4.0 / 360.0, 0.55, 0.63, 0.10),
    }
}
