use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, Hsla, RenderOnce, SharedString,
    Stateful, StyleRefinement, Window, div, prelude::*, px, transparent_black,
};

use super::control::with_defaults;
use crate::ui::primitives::activation::{ActivationHandler, activation_listener, with_activation};
use crate::ui::{ActivationExt, StyledTypography, TextStyle, UiColors, icon};

#[derive(IntoElement)]
pub struct Checkbox {
    base: Stateful<Div>,
    checked: bool,
    label: SharedString,
    disabled: bool,
    checked_background: Option<Hsla>,
    trailing: Option<AnyElement>,
    activate: Option<ActivationHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool, label: impl Into<SharedString>) -> Self {
        Self {
            base: div().id(id),
            checked,
            label: label.into(),
            disabled: false,
            checked_background: None,
            trailing: None,
            activate: None,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn checked_background(mut self, color: Hsla) -> Self {
        self.checked_background = Some(color);
        self
    }
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus);
        self
    }
    pub fn on_change<E: 'static>(
        self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, bool, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        let proposed = !self.checked;
        self.on_activation(cx, move |this, window, cx| {
            handler(this, proposed, window, cx)
        })
    }
}

impl ActivationExt for Checkbox {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.activate = Some(activation_listener(cx, handler));
        self
    }
}
impl Styled for Checkbox {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let base = with_defaults(self.base, |base| {
            base.h(px(44.0))
                .w_full()
                .px(px(12.0))
                .rounded(px(9.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .cursor_default()
                .text_style(TextStyle::Body)
                .text_color(if self.disabled {
                    colors.text_ghost
                } else {
                    colors.text
                })
        })
        .focus_visible(|style| style.border_1().border_color(colors.accent))
        .when(!self.disabled, |base| {
            base.tab_index(0).hover(|style| style.bg(colors.overlay))
        })
        .child(
            div()
                .size(px(15.0))
                .flex_none()
                .rounded(px(4.0))
                .border_1()
                .border_color(if self.checked {
                    colors.border_strong
                } else {
                    colors.text_ghost
                })
                .bg(if self.checked {
                    self.checked_background.unwrap_or(colors.inset)
                } else {
                    transparent_black()
                })
                .flex()
                .items_center()
                .justify_center()
                .when(self.checked, |base| {
                    base.child(icon("checkmark", 12.0, colors.text))
                })
                .squircle(),
        )
        .child(div().min_w_0().flex_1().child(self.label))
        .children(self.trailing);
        (match self.activate.filter(|_| !self.disabled) {
            Some(handler) => with_activation(base, handler),
            None => base,
        })
        .squircle()
    }
}
