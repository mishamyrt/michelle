use gpui::{
    AnyElement, App, Div, RenderOnce, SharedString, StyleRefinement, Window, prelude::*, px,
};

use super::{Label, control::with_defaults};
use crate::ui::{TextStyle, UiColors, h_stack, v_stack};

/// A wrapping label column and an independent control. The row has no action.
#[derive(IntoElement)]
pub struct FormRow {
    base: Div,
    label: SharedString,
    description: Option<SharedString>,
    label_content: Option<AnyElement>,
    description_content: Option<AnyElement>,
    leading: Option<AnyElement>,
    control: Option<AnyElement>,
}

impl FormRow {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            base: h_stack(),
            label: label.into(),
            description: None,
            label_content: None,
            description_content: None,
            leading: None,
            control: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self.description_content = None;
        self
    }

    pub fn label_content(mut self, label: impl IntoElement) -> Self {
        self.label_content = Some(label.into_any_element());
        self
    }

    pub fn description_content(mut self, description: impl IntoElement) -> Self {
        self.description = None;
        self.description_content = Some(description.into_any_element());
        self
    }

    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }
}

impl Styled for FormRow {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for FormRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let label = self.label_content.unwrap_or_else(|| {
            Label::new(self.label)
                .text_style(TextStyle::Headline)
                .text_color(colors.text)
                .into_any_element()
        });
        let description = self.description_content.or_else(|| {
            self.description.map(|description| {
                Label::new(description)
                    .text_color(colors.text_secondary)
                    .into_any_element()
            })
        });
        with_defaults(self.base, |base| {
            base.w_full()
                .min_h(px(60.0))
                .px(px(20.0))
                .py(px(12.0))
                .flex()
                .items_center()
                .gap(px(24.0))
        })
        .children(self.leading)
        .child(
            v_stack()
                .flex_1()
                .min_w_0()
                .gap(px(5.0))
                .child(label)
                .children(description),
        )
        .children(self.control)
    }
}
