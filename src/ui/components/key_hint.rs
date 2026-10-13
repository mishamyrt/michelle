use crate::ui::squircle::SquircleExt;
use gpui::{App, Div, RenderOnce, SharedString, StyleRefinement, Window, div, prelude::*, px};

use super::control::with_defaults;
use crate::ui::{StyledTypography, TextStyle, UiColors};

#[derive(IntoElement)]
pub struct KeyHint {
    base: Div,
    label: SharedString,
}

impl KeyHint {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            label: label.into(),
        }
    }
}

impl Styled for KeyHint {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for KeyHint {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            base.h(px(22.0))
                .min_w(px(34.0))
                .px(px(7.0))
                .rounded(px(11.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.overlay_strong)
                .text_style(TextStyle::BODY)
                .text_color(colors.text_secondary)
        })
        .child(self.label)
        .squircle()
    }
}
