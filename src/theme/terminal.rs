use alacritty_terminal::vte::ansi::{NamedColor, Rgb};
use gpui::{Hsla, rgb};

use super::Theme;

#[derive(Clone, Copy)]
pub(crate) struct TerminalColors {
    pub is_dark: bool,
    pub background: Hsla,
    pub foreground: Hsla,
    pub cursor: Hsla,
    pub selection: Hsla,
}

impl TerminalColors {
    pub fn new(theme: Theme) -> Self {
        Self {
            is_dark: theme.is_dark,
            background: theme.terminal,
            foreground: rgb_to_hsla(terminal_rgb(NamedColor::Foreground as usize, theme.is_dark)),
            cursor: rgb_to_hsla(terminal_rgb(NamedColor::Cursor as usize, theme.is_dark)),
            selection: rgb(if theme.is_dark { 0x18304b } else { 0xd2e3f8 }).into(),
        }
    }
}

pub(crate) fn rgb_to_hsla(color: Rgb) -> Hsla {
    rgb((u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)).into()
}

pub(crate) fn terminal_rgb(index: usize, is_dark: bool) -> Rgb {
    // Myrt's dark hues are softened for Michelle's neutral surface.
    const ANSI_DARK: [u32; 16] = [
        0x626262, 0xdd6975, 0x33ba51, 0xe6d375, 0x6da5e3, 0x9f83d5, 0x38b1b9, 0xc0c0c0, 0x8b8b8b,
        0xe29da4, 0x7ad18e, 0xe6d375, 0xb4cbe5, 0xbaa5e4, 0x51bec6, 0xe3e3e3,
    ];
    const ANSI_LIGHT: [u32; 16] = [
        0x24292e, 0xcb2431, 0x28a745, 0xdbab09, 0x005cc5, 0x5a32a3, 0x1b7c83, 0x6a737d, 0x959da5,
        0xb31d28, 0x22863a, 0xb08800, 0x044289, 0x4c2889, 0x3192aa, 0xd1d5da,
    ];
    let ansi = if is_dark { &ANSI_DARK } else { &ANSI_LIGHT };
    let value = match index {
        0..=15 => ansi[index],
        16..=231 => {
            let index = index - 16;
            let channel = |value: usize| {
                if value == 0 {
                    0
                } else {
                    55 + value as u32 * 40
                }
            };
            let red = channel(index / 36);
            let green = channel((index / 6) % 6);
            let blue = channel(index % 6);
            (red << 16) | (green << 8) | blue
        }
        232..=255 => {
            let value = 8 + (index as u32 - 232) * 10;
            (value << 16) | (value << 8) | value
        }
        value if value == NamedColor::Background as usize => {
            if is_dark {
                0x181818
            } else {
                0xffffff
            }
        }
        value if value == NamedColor::Cursor as usize => {
            if is_dark {
                0x6da5e3
            } else {
                0x005cc5
            }
        }
        value
            if value >= NamedColor::DimBlack as usize && value <= NamedColor::DimWhite as usize =>
        {
            let base = ansi[value - NamedColor::DimBlack as usize];
            // Faint fades toward the surface: black on dark, white on light.
            let dim = |channel: u32| {
                if is_dark {
                    channel * 2 / 3
                } else {
                    channel + (255 - channel) / 3
                }
            };
            (dim((base >> 16) & 0xff) << 16) | (dim((base >> 8) & 0xff) << 8) | dim(base & 0xff)
        }
        _ => {
            if is_dark {
                0xc0c0c0
            } else {
                0x444d56
            }
        }
    };
    Rgb {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}
