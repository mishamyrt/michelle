use crate::ui::UiColors;
use gpui::Hsla;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatusTone {
    #[default]
    Neutral,
    Success,
    Warning,
    Danger,
}

impl StatusTone {
    pub(super) fn color(self, colors: UiColors) -> Hsla {
        match self {
            Self::Neutral => colors.text_tertiary,
            Self::Success => colors.success,
            Self::Warning => colors.warning,
            Self::Danger => colors.danger,
        }
    }
}
