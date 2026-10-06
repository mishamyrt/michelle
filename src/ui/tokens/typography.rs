//! Semantic chrome text styles. Content/editor fonts retain their own scale.

use gpui::{FontWeight, Pixels, Styled, px};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    Caption,
    CaptionEmphasized,
    Body,
    BodyEmphasized,
    Headline,
    HeadlineEmphasized,
    Title3,
    Title3Emphasized,
    Title2,
    Title2Emphasized,
    Title1,
    Title1Emphasized,
}

impl TextStyle {
    pub fn size(self) -> Pixels {
        px(match self {
            Self::Caption | Self::CaptionEmphasized => 11.0,
            Self::Body | Self::BodyEmphasized => 13.0,
            Self::Headline | Self::HeadlineEmphasized => 13.0,
            Self::Title3 | Self::Title3Emphasized => 15.0,
            Self::Title2 | Self::Title2Emphasized => 17.0,
            Self::Title1 | Self::Title1Emphasized => 22.0,
        })
    }

    pub fn line_height(self) -> Pixels {
        px(match self {
            Self::Caption | Self::CaptionEmphasized => 14.0,
            Self::Body | Self::BodyEmphasized => 16.0,
            Self::Headline | Self::HeadlineEmphasized => 16.0,
            Self::Title3 | Self::Title3Emphasized => 20.0,
            Self::Title2 | Self::Title2Emphasized => 22.0,
            Self::Title1 | Self::Title1Emphasized => 26.0,
        })
    }

    pub fn weight(self) -> FontWeight {
        match self {
            Self::Caption => FontWeight::NORMAL,
            Self::CaptionEmphasized => FontWeight::SEMIBOLD,
            Self::Body => FontWeight::NORMAL,
            Self::BodyEmphasized => FontWeight::SEMIBOLD,
            Self::Headline => FontWeight::SEMIBOLD,
            Self::HeadlineEmphasized => FontWeight::BOLD,
            Self::Title3 => FontWeight::NORMAL,
            Self::Title3Emphasized => FontWeight::SEMIBOLD,
            Self::Title2 => FontWeight::NORMAL,
            Self::Title2Emphasized => FontWeight::SEMIBOLD,
            Self::Title1 => FontWeight::NORMAL,
            Self::Title1Emphasized => FontWeight::SEMIBOLD,
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
