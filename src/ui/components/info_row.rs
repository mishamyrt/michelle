use gpui::{
    AnyElement, App, Div, Pixels, RenderOnce, SharedString, StyleRefinement, Window, div,
    prelude::*, px,
};

use super::control::with_defaults;
use crate::ui::{StyledTypography, TextStyle, UiColors};

#[derive(IntoElement)]
pub struct InfoRow {
    base: Div,
    label: SharedString,
    value: AnyElement,
    label_width: Pixels,
    trailing: Vec<AnyElement>,
}
impl InfoRow {
    pub fn new(label: impl Into<SharedString>, value: impl IntoElement) -> Self {
        Self {
            base: div(),
            label: label.into(),
            value: value.into_any_element(),
            label_width: px(84.0),
            trailing: Vec::new(),
        }
    }
    pub fn label_width(mut self, width: Pixels) -> Self {
        self.label_width = width;
        self
    }
}
impl Styled for InfoRow {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl ParentElement for InfoRow {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.trailing.extend(children);
    }
}
impl RenderOnce for InfoRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            base.py(px(8.0)).flex().items_baseline().gap(px(12.0))
        })
        .child(
            div()
                .w(self.label_width)
                .flex_none()
                .text_style(TextStyle::BODY)
                .text_color(colors.text_tertiary)
                .child(self.label),
        )
        .child(div().flex_1().min_w_0().flex().child(self.value))
        .children(self.trailing)
    }
}
