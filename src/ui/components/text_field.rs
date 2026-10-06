use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement, Interactivity, ParentElement, RenderOnce,
    Stateful, StyleRefinement, Styled, Window, div, prelude::*, px,
};

pub use crate::input::{FieldMode, InputEvent, MediaPaste, TextInput};
use crate::ui::{TextStyle, UiColors};

use super::control::with_defaults;
use crate::ui::tokens::metrics::{radius, spacing};
use crate::ui::{ControlSize, icon};

/// The one-line text box shell: a fixed-height bordered field around a
/// [`TextInput`], with an optional leading icon and an accent border
/// while focused.
///
/// The embedded input must stay single-line (the default mode) — that is
/// what keeps the text from wrapping and slides overlong content under the
/// clipped viewport instead of growing out of the fixed-height shell.
/// Construction stays at the call site because the entity needs the window;
/// this component owns everything visual.
#[derive(IntoElement)]
pub struct TextField {
    base: Stateful<Div>,
    input: gpui::Entity<TextInput>,
    icon: Option<(&'static str, f32)>,
    size: ControlSize,
}

impl TextField {
    pub fn new(id: impl Into<ElementId>, input: gpui::Entity<TextInput>) -> Self {
        Self {
            base: div().id(id),
            input,
            icon: None,
            size: ControlSize::Regular,
        }
    }

    /// Leading icon, tinted tertiary like the address bar's lock.
    pub fn icon(mut self, path: &'static str, size: f32) -> Self {
        self.icon = Some((path, size));
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Styled for TextField {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for TextField {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for TextField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = UiColors::current(cx);
        let focused = self.input.read(cx).is_visually_focused(window);
        with_defaults(self.base, |base| {
            base.h(self.size.height())
                .px(px(spacing::MD))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(theme.border_strong)
                .bg(theme.inset)
                .flex()
                .items_center()
                .gap(px(spacing::SM))
                .text_size(TextStyle::Caption.size())
                .line_height(TextStyle::Caption.line_height())
        })
        // Focus remains visible even with an explicit border color.
        .when(focused, |element| element.border_color(theme.accent))
        .when_some(self.icon, |element, (path, size)| {
            element.child(icon(path, size, theme.text_tertiary))
        })
        .child(div().min_w_0().flex_1().child(self.input))
        .squircle()
    }
}
