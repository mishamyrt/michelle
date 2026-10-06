use gpui::{
    AnyElement, App, Div, RenderOnce, SharedString, StyleRefinement, Window, div, prelude::*, px,
};

use super::{Label, control::with_defaults};
use crate::ui::{TextStyle, UiColors};

#[derive(IntoElement)]
pub struct EmptyState {
    base: Div,
    title: SharedString,
    title_style: TextStyle,
    description: Option<SharedString>,
    icon: Option<AnyElement>,
    action: Option<AnyElement>,
}
impl EmptyState {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            title: title.into(),
            title_style: TextStyle::Headline,
            description: None,
            icon: None,
            action: None,
        }
    }
    pub fn title_style(mut self, style: TextStyle) -> Self {
        self.title_style = style;
        self
    }
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}
impl Styled for EmptyState {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            base.size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .p(px(40.0))
                .text_color(colors.text)
        })
        .children(self.icon)
        .child(Label::new(self.title).text_style(self.title_style))
        .when_some(self.description, |base, description| {
            base.child(
                Label::new(description)
                    .max_w(px(420.0))
                    .text_center()
                    .text_color(colors.text_secondary),
            )
        })
        .children(self.action)
    }
}
