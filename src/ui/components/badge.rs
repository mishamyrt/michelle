use super::{StatusTone, control::with_defaults};
use crate::ui::squircle::SquircleExt;
use crate::ui::{StyledTypography, TextStyle, UiColors};
use gpui::{
    AnyElement, App, Div, RenderOnce, SharedString, StyleRefinement, Window, div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct Badge {
    base: Div,
    label: SharedString,
    icon: Option<AnyElement>,
    tone: StatusTone,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            label: label.into(),
            icon: None,
            tone: StatusTone::Neutral,
        }
    }
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }
    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Styled for Badge {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            base.flex_none()
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded_full()
                .bg(colors.overlay)
                .text_style(TextStyle::BODY)
                .text_color(self.tone.color(colors))
        })
        .children(self.icon)
        .child(self.label)
        .squircle()
    }
}
