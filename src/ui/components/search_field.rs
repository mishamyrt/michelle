use gpui::{
    AnyElement, App, ElementId, Entity, Interactivity, RenderOnce, StyleRefinement, Window,
    prelude::*,
};

use super::{TextField, TextInput};
use crate::ui::ControlSize;

/// Search chrome around the owner's existing single-line input.
#[derive(IntoElement)]
pub struct SearchField {
    field: TextField,
}

impl SearchField {
    pub fn new(id: impl Into<ElementId>, input: Entity<TextInput>) -> Self {
        Self {
            field: TextField::new(id, input).icon("magnifyingglass", 13.0),
        }
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.field = self.field.control_size(size);
        self
    }
}

impl Styled for SearchField {
    fn style(&mut self) -> &mut StyleRefinement {
        self.field.style()
    }
}

impl InteractiveElement for SearchField {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.field.interactivity()
    }
}

impl ParentElement for SearchField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.field.extend(elements);
    }
}

impl RenderOnce for SearchField {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.field
    }
}
