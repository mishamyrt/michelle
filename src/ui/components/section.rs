use gpui::{
    AnyElement, App, Div, RenderOnce, SharedString, StyleRefinement, Window, prelude::*, px,
};

use super::{Label, control::with_defaults};
use crate::ui::{TextStyle, UiColors};

/// A heading and description above the section's content, without a card.
#[derive(IntoElement)]
pub struct Section {
    base: Div,
    title: SharedString,
    description: Option<SharedString>,
    children: Vec<AnyElement>,
}

impl Section {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            base: gpui::div(),
            title: title.into(),
            description: None,
            children: Vec::new(),
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl Styled for Section {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Section {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Section {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| base.flex().flex_col().gap(px(5.0)))
            .child(
                Label::new(self.title)
                    .text_style(TextStyle::Headline)
                    .text_color(colors.text),
            )
            .when_some(self.description, |base, description| {
                base.child(Label::new(description).text_color(colors.text_secondary))
            })
            .children(self.children)
    }
}
