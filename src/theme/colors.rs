use super::{dark, light};
use crate::ui::UiColors;
use gpui::Hsla;
use serde::Deserialize;

/// Michelle's application color roles. Built-in themes compose UI palettes;
/// user theme files retain this complete, flat schema.
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

    pub toolbar_button_bg: Hsla,
    pub toolbar_button_border: Hsla,

    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,

    /// Accent for the logo, caret and live activity.
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
    pub fn light() -> Self {
        light::theme()
    }

    pub fn dark() -> Self {
        dark::theme()
    }

    /// Project application styling into the colors shared by reusable controls.
    pub fn ui_colors(self) -> UiColors {
        UiColors {
            raised: self.raised,
            inset: self.inset,
            overlay: self.overlay,
            overlay_strong: self.overlay_strong,
            border: self.border,
            border_strong: self.border_strong,
            text: self.text,
            text_secondary: self.text_secondary,
            text_tertiary: self.text_tertiary,
            text_ghost: self.text_ghost,
            accent: self.accent,
            inverse: self.inverse,
            on_inverse: self.on_inverse,
            danger: self.danger,
            danger_soft: self.danger_soft,
            row_selection: self.sidebar_item_background,
            control_fill: self.toolbar_button_bg,
            control_border: self.toolbar_button_border,
        }
    }
}
