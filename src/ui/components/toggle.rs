use super::control::{ControlState, control_style_methods, with_defaults};
use crate::ui::primitives::activation::activation_listener;
use crate::ui::primitives::timing::TOOLTIP_SHOW_DELAY_MS;
use crate::ui::squircle::SquircleExt;
use crate::ui::{ActivationExt, Tooltip, UiColors};
use gpui::{
    App, Context, Div, ElementId, FocusHandle, InteractiveElement, Interactivity, RenderOnce,
    SharedString, Stateful, StyleRefinement, Styled, Window, div, prelude::*, px,
};

/// A controlled switch: the owner accepts a proposed value and updates state.
#[derive(IntoElement)]
pub struct Toggle {
    base: Stateful<Div>,
    on: bool,
    state: ControlState,
    tooltip: Option<SharedString>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            base: div().id(id).tab_index(0),
            on,
            state: ControlState::default(),
            tooltip: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus);
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_change<E: 'static>(
        self,
        cx: &mut Context<E>,
        change: impl Fn(&mut E, bool, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        let proposed = !self.on;
        self.on_activation(cx, move |owner, window, cx| {
            change(owner, proposed, window, cx)
        })
    }

    control_style_methods!();
}

impl Styled for Toggle {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Toggle {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ActivationExt for Toggle {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.state.activate = Some(activation_listener(cx, activate));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = UiColors::current(cx);
        let base = toggle_control(self.base, self.on, theme);
        self.state
            .apply(
                base,
                theme,
                StyleRefinement::default(),
                StyleRefinement::default(),
            )
            .when_some(self.tooltip, |base, text| {
                base.tooltip(Tooltip::text(text))
                    .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
            })
            .squircle()
    }
}

fn toggle_control(base: Stateful<Div>, on: bool, theme: UiColors) -> Stateful<Div> {
    with_defaults(base, |base| {
        base.w(px(36.0))
            .h(px(20.0))
            .p(px(2.0))
            .flex_none()
            .rounded_full()
            .cursor_default()
            .bg(if on { theme.inverse } else { theme.inset })
            .border_1()
            .border_color(if on {
                theme.inverse
            } else {
                theme.border_strong
            })
            .flex()
            .items_center()
            .when(on, |element| element.justify_end())
            .child(div().w(px(14.0)).h(px(14.0)).rounded_full().bg(if on {
                theme.on_inverse
            } else {
                theme.text_tertiary
            }))
    })
}
