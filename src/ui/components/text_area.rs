use gpui::{
    App, Div, ElementId, Entity, InteractiveElement, Interactivity, RenderOnce, Stateful,
    StyleRefinement, Window, div, prelude::*, px,
};

use super::{TextInput, control::with_defaults};
use crate::ui::{StyledTypography, TextStyle, UiColors};

/// Multi-line chrome; input mode, content, editing and scroll state stay with the input.
#[derive(IntoElement)]
pub struct TextArea {
    base: Stateful<Div>,
    input: Entity<TextInput>,
}

impl TextArea {
    pub fn new(id: impl Into<ElementId>, input: Entity<TextInput>) -> Self {
        Self {
            base: div().id(id),
            input,
        }
    }
}
impl Styled for TextArea {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl InteractiveElement for TextArea {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}
impl RenderOnce for TextArea {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        with_defaults(self.base, |base| {
            base.h(px(112.0))
                .px(px(16.0))
                .py(px(10.0))
                .text_style(TextStyle::BODY)
                .text_color(UiColors::current(cx).text)
        })
        .child(self.input)
    }
}
