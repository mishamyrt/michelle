use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, RenderOnce, SharedString, Stateful,
    StyleRefinement, Window, div, prelude::*, px,
};

use super::{ButtonRole, control::with_defaults};
use crate::ui::primitives::activation::{ActivationHandler, activation_listener, with_activation};
use crate::ui::{ActivationExt, StyledTypography, TextStyle, UiColors, icon, motion};

#[derive(IntoElement)]
pub struct ActionRow {
    base: Stateful<Div>,
    label: SharedString,
    icon: Option<&'static str>,
    trailing: Option<AnyElement>,
    role: ButtonRole,
    disabled: bool,
    loading: bool,
    activate: Option<ActivationHandler>,
}

impl ActionRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            base: div().id(id),
            label: label.into(),
            icon: None,
            trailing: None,
            role: ButtonRole::Normal,
            disabled: false,
            loading: false,
            activate: None,
        }
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }
    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus);
        self
    }
}

impl ActivationExt for ActionRow {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.activate = Some(activation_listener(cx, handler));
        self
    }
}

impl Styled for ActionRow {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for ActionRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let foreground = if self.disabled {
            if self.loading {
                colors.text_secondary
            } else {
                colors.text_ghost
            }
        } else if self.role == ButtonRole::Destructive {
            colors.danger
        } else {
            colors.text
        };
        let base = with_defaults(self.base, |base| {
            base.h(px(38.0))
                .w_full()
                .px(px(10.0))
                .rounded(px(9.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .cursor_default()
                .text_style(TextStyle::Body)
                .text_color(foreground)
        })
        .focus_visible(|style| style.border_1().border_color(colors.accent))
        .when(!self.disabled, |base| {
            base.tab_index(0)
                .hover(|style| style.bg(colors.overlay_strong))
        })
        .when(self.loading, |base| {
            base.child(motion::spinner(15.0, colors.text_secondary))
        })
        .when_some(self.icon.filter(|_| !self.loading), |base, name| {
            base.child(icon(name, 15.0, foreground))
        })
        .child(div().min_w_0().flex_1().truncate().child(self.label))
        .children(self.trailing);
        (match self.activate.filter(|_| !self.disabled) {
            Some(handler) => with_activation(base, handler),
            None => base,
        })
        .squircle()
    }
}
