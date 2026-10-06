use super::{dark, light};
use gpui::Hsla;

/// Color families shared by themes. These tokens have no application or
/// component roles; a theme decides where each color is used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub labels: LabelColors,
    pub fills: FillColors,
    pub separators: SeparatorColors,
    pub colors: SystemColors,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelColors {
    pub primary: Hsla,
    pub secondary: Hsla,
    pub tertiary: Hsla,
    pub quaternary: Hsla,
}

/// Translucent neutral layers, ordered from strongest to weakest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillColors {
    pub primary: Hsla,
    pub secondary: Hsla,
    pub tertiary: Hsla,
    pub quaternary: Hsla,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeparatorColors {
    pub standard: Hsla,
    pub strong: Hsla,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemColors {
    pub blue: Hsla,
    pub blue_deep: Hsla,
    pub green: Hsla,
    pub green_bright: Hsla,
    pub orange: Hsla,
    pub yellow: Hsla,
    pub red: Hsla,
}

impl Palette {
    pub fn dark() -> Self {
        dark::palette()
    }

    pub fn light() -> Self {
        light::palette()
    }
}
