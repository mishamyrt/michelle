//! Resolved colors for reusable controls, independent of application surfaces.
use super::Palette;
use gpui::{Hsla, hsla, rgb};

/// A theme maps its colors into this small snapshot when it is applied.
/// Components never need to know about canvas, sidebar, composer or terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiColors {
    pub raised: Hsla,
    pub inset: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,
    pub accent: Hsla,
    pub inverse: Hsla,
    pub on_inverse: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
    pub row_selection: Hsla,
    pub control_fill: Hsla,
    pub control_border: Hsla,
}

impl UiColors {
    pub fn light() -> Self {
        let palette = Palette::light();
        Self {
            raised: rgb(0xFFFFFF).into(),
            inset: rgb(0xE6E6E6).into(),
            overlay: palette.fills.quaternary,
            overlay_strong: palette.fills.primary,
            border: palette.separators.standard,
            border_strong: palette.separators.strong,
            text: palette.labels.primary,
            text_secondary: palette.labels.secondary,
            text_tertiary: palette.labels.tertiary,
            text_ghost: palette.labels.quaternary,
            accent: palette.colors.blue,
            inverse: rgb(0x202227).into(),
            on_inverse: rgb(0xF8F8F9).into(),
            danger: palette.colors.red,
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.52, 0.10),
            row_selection: palette.fills.tertiary,
            control_fill: hsla(0.0, 0.0, 0.97, 0.7),
            control_border: hsla(0.0, 0.0, 0.1, 0.12),
        }
    }

    pub fn dark() -> Self {
        let palette = Palette::dark();
        Self {
            raised: rgb(0x232323).into(),
            inset: rgb(0x151515).into(),
            overlay: palette.fills.quaternary,
            overlay_strong: palette.fills.primary,
            border: palette.separators.standard,
            border_strong: palette.separators.strong,
            text: palette.labels.primary,
            text_secondary: palette.labels.secondary,
            text_tertiary: palette.labels.tertiary,
            text_ghost: palette.labels.quaternary,
            accent: palette.colors.blue,
            inverse: rgb(0xE7E9EC).into(),
            on_inverse: rgb(0x17181C).into(),
            danger: palette.colors.red,
            danger_soft: hsla(4.0 / 360.0, 0.55, 0.63, 0.10),
            row_selection: palette.fills.tertiary,
            control_fill: hsla(0.0, 0.0, 1.0, 0.06),
            control_border: hsla(0.0, 0.0, 0.72, 0.21),
        }
    }
}
