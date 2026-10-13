use super::{StatusTone, control::with_defaults};
use crate::ui::squircle::SquircleExt;
use crate::ui::{StyledTypography, TextStyle, UiColors};
use gpui::{AnyElement, App, Div, RenderOnce, StyleRefinement, Window, div, prelude::*, px};

#[derive(IntoElement)]
pub struct InlineNotice {
    base: Div,
    content: AnyElement,
    icon: Option<AnyElement>,
    action: Option<AnyElement>,
    tone: StatusTone,
}

impl InlineNotice {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            base: div(),
            content: content.into_any_element(),
            icon: None,
            action: None,
            tone: StatusTone::Neutral,
        }
    }
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Styled for InlineNotice {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for InlineNotice {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            base.px(px(12.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(colors.border)
                .flex()
                .items_start()
                .gap(px(8.0))
                .text_style(TextStyle::BODY)
                .text_color(self.tone.color(colors))
        })
        .children(self.icon)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .whitespace_normal()
                .child(self.content),
        )
        .children(self.action)
        .squircle()
    }
}
