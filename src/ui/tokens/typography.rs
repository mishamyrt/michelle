//! Fixed macOS text styles in logical pixels, matching AppKit preferred fonts.

use gpui::{FontWeight, Pixels, Styled, px};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextVariant {
    LargeTitle,
    Title1,
    Title2,
    Title3,
    Headline,
    Body,
    Callout,
    Subheadline,
    Footnote,
    Caption1,
    Caption2,
}

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    variant: TextVariant,
    emphasized: bool,
}

impl TextStyle {
    pub const LARGE_TITLE: Self = Self::new(TextVariant::LargeTitle);
    pub const TITLE_1: Self = Self::new(TextVariant::Title1);
    pub const TITLE_2: Self = Self::new(TextVariant::Title2);
    pub const TITLE_3: Self = Self::new(TextVariant::Title3);
    pub const HEADLINE: Self = Self::new(TextVariant::Headline);
    pub const BODY: Self = Self::new(TextVariant::Body);
    pub const CALL_OUT: Self = Self::new(TextVariant::Callout);
    pub const SUBHEADLINE: Self = Self::new(TextVariant::Subheadline);
    pub const FOOTNOTE: Self = Self::new(TextVariant::Footnote);
    pub const CAPTION_1: Self = Self::new(TextVariant::Caption1);
    pub const CAPTION_2: Self = Self::new(TextVariant::Caption2);

    const fn new(variant: TextVariant) -> Self {
        Self {
            variant,
            emphasized: false,
        }
    }

    pub const fn bold(self) -> Self {
        Self {
            emphasized: true,
            ..self
        }
    }
}

impl TextStyle {
    pub const fn size(self) -> Pixels {
        px(match self.variant {
            TextVariant::LargeTitle => 26.0,
            TextVariant::Title1 => 22.0,
            TextVariant::Title2 => 17.0,
            TextVariant::Title3 => 15.0,
            TextVariant::Headline => 13.0,
            TextVariant::Body => 13.0,
            TextVariant::Callout => 12.0,
            TextVariant::Subheadline => 11.0,
            TextVariant::Footnote => 10.0,
            TextVariant::Caption1 => 10.0,
            TextVariant::Caption2 => 10.0,
        })
    }

    pub const fn line_height(self) -> Pixels {
        px(match self.variant {
            TextVariant::LargeTitle => 32.0,
            TextVariant::Title1 => 26.0,
            TextVariant::Title2 => 22.0,
            TextVariant::Title3 => 20.0,
            TextVariant::Headline => 16.0,
            TextVariant::Body => 16.0,
            TextVariant::Callout => 15.0,
            TextVariant::Subheadline => 14.0,
            TextVariant::Footnote => 13.0,
            TextVariant::Caption1 => 13.0,
            TextVariant::Caption2 => 13.0,
        })
    }

    pub const fn weight(self) -> FontWeight {
        match (self.variant, self.emphasized) {
            (TextVariant::Headline, false) => FontWeight::BOLD,
            (TextVariant::Caption2, false) => FontWeight::MEDIUM,
            (_, false) => FontWeight::NORMAL,
            (TextVariant::LargeTitle, true) => FontWeight::BOLD,
            (TextVariant::Title1, true) => FontWeight::BOLD,
            (TextVariant::Title2, true) => FontWeight::BOLD,
            (TextVariant::Headline, true) => FontWeight(860.0),
            (TextVariant::Caption1, true) => FontWeight::MEDIUM,
            (_, true) => FontWeight::SEMIBOLD,
        }
    }
}

pub trait StyledTypography: Styled + Sized {
    fn text_style(self, style: TextStyle) -> Self {
        self.text_size(style.size())
            .line_height(style.line_height())
            .font_weight(style.weight())
    }
}

impl<T: Styled> StyledTypography for T {}
