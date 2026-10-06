//! Shared chrome dimensions. Component-specific geometry stays in its module.

use gpui::{Pixels, Rems, px, rems};

/// Height of the window's top toolbar and titlebar strips.
pub const TOOLBAR_HEIGHT: f32 = 52.0;

/// Fixed rem base for legacy UI dimensions, independent of content settings.
pub const UI_REM_SIZE: f32 = 14.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    Mini,
    Small,
    #[default]
    Regular,
    Large,
}

impl ControlSize {
    pub fn height(self) -> Pixels {
        px(match self {
            Self::Mini => 22.0,
            Self::Small => 26.0,
            Self::Regular => 28.0,
            Self::Large => 36.0,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconSize {
    Tiny,
    Small,
    #[default]
    Regular,
    Large,
}

impl IconSize {
    pub const fn font_size(self) -> f32 {
        match self {
            Self::Tiny => 11.0,
            Self::Small => 13.0,
            Self::Regular => 15.0,
            Self::Large => 17.0,
        }
    }

    pub const fn max_width(self) -> f32 {
        match self {
            Self::Tiny => 16.0,
            Self::Small => 18.0,
            Self::Regular => 22.0,
            Self::Large => 26.0,
        }
    }
}

pub mod spacing {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
    pub const XL: f32 = 16.0;
}

pub mod radius {
    pub const CONTROL: f32 = 6.0;
    pub const ROW: f32 = 7.0;
    pub const POPOVER: f32 = 9.0;
}

/// Legacy UI dimensions in rems. Windows use the fixed [`UI_REM_SIZE`] base,
/// so the resolved logical size equals `value`. New UI uses `TextStyle`/`px`;
/// content surfaces receive explicit pixel metrics from their owner.
pub fn sp(value: f32) -> Rems {
    rems(value / UI_REM_SIZE)
}
