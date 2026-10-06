use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, Hsla, InteractiveElement, Interactivity,
    MouseButton, RenderOnce, Stateful, StyleRefinement, Window, deferred, div, hsla, prelude::*,
    px,
};

use super::{Divider, control::with_defaults, surface::surface_defaults};
use crate::ui::UiColors;
use crate::ui::primitives::activation::{ActivationHandler, activation_listener};

/// A deferred modal surface. Drafts, operations, actions and focus ownership remain outside it.
#[derive(IntoElement)]
pub struct Dialog {
    card: Stateful<Div>,
    layer_id: ElementId,
    header: Option<AnyElement>,
    children: Vec<AnyElement>,
    actions: Option<AnyElement>,
    scrim: Hsla,
    dismiss: Option<ActivationHandler>,
}

impl Dialog {
    pub fn new(layer_id: impl Into<ElementId>, card_id: impl Into<ElementId>) -> Self {
        Self {
            card: div().id(card_id),
            layer_id: layer_id.into(),
            header: None,
            children: Vec::new(),
            actions: None,
            scrim: hsla(0.0, 0.0, 0.0, 0.34),
            dismiss: None,
        }
    }
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }
    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }
    pub fn scrim(mut self, color: Hsla) -> Self {
        self.scrim = color;
        self
    }
    pub fn on_dismiss<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.dismiss = Some(activation_listener(cx, handler));
        self
    }
}

impl Styled for Dialog {
    fn style(&mut self) -> &mut StyleRefinement {
        self.card.style()
    }
}
impl InteractiveElement for Dialog {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.card.interactivity()
    }
}
impl ParentElement for Dialog {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}

impl RenderOnce for Dialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let card = with_defaults(self.card, |base| {
            surface_defaults(base, colors)
                .max_w(px(420.0))
                .overflow_hidden()
                .rounded(px(18.0))
                .shadow_xl()
        })
        .tab_group()
        .tab_stop(false)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(self.header)
        .children(self.children)
        .when_some(self.actions, |card, actions| {
            card.child(Divider::horizontal().inset(px(8.0)))
                .child(actions)
        });
        let card = card.squircle();
        let layer = div()
            .id(self.layer_id)
            .absolute()
            .inset_0()
            .occlude()
            .bg(self.scrim)
            .p(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .when_some(self.dismiss, |layer, dismiss| {
                layer.on_mouse_down(MouseButton::Left, move |_, window, cx| dismiss(window, cx))
            })
            .child(card);
        deferred(layer).with_priority(4)
    }
}

/// Deferred content joins the dispatch tree after draw. Check the owner's
/// current identity before focusing two frames later, including after close/replacement.
pub fn focus_dialog_input<E: 'static>(
    window: &mut Window,
    cx: &mut Context<E>,
    focus: FocusHandle,
    is_current: impl Fn(&E, &App) -> bool + 'static,
) {
    let owner = cx.entity().downgrade();
    window.on_next_frame(move |window, _| {
        window.on_next_frame(move |window, cx| {
            if owner
                .read_with(cx, |owner, cx| is_current(owner, cx))
                .unwrap_or(false)
            {
                window.focus(&focus, cx);
            }
        });
    });
}

#[cfg(test)]
mod tests;
