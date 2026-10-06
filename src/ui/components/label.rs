use gpui::{App, Div, RenderOnce, SharedString, StyleRefinement, Styled, Window, div, prelude::*};

use crate::ui::{StyledTypography, TextStyle};

/// Chrome text that inherits its container's font family and foreground color.
#[derive(IntoElement)]
pub struct Label {
    base: Div,
    text: SharedString,
}

impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            base: div().text_style(TextStyle::Body),
            text: text.into(),
        }
    }

    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.base = self.base.text_style(style);
        self
    }

    pub fn truncate(mut self) -> Self {
        self.base = self.base.min_w_0().truncate();
        self
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.base.child(self.text)
    }
}
