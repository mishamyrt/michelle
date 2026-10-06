use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, Interactivity, RenderOnce, Stateful,
    StyleRefinement, Window, div, prelude::*, px,
};

use super::control::{ControlState, control_style_methods, with_defaults};
use crate::ui::primitives::activation::{activation_listener, with_activation};
use crate::ui::{
    ActivationExt, IconSize, SfSymbol, StyledTypography, SymbolWeight, TextStyle, UiColors,
};

#[derive(IntoElement)]
pub struct SelectableRow {
    base: Stateful<Div>,
    content: AnyElement,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    selected: bool,
    highlighted: bool,
    checkmark_at_start: bool,
    state: ControlState,
}

impl SelectableRow {
    pub fn new(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            base: div().id(id),
            content: content.into_any_element(),
            leading: None,
            trailing: None,
            selected: false,
            highlighted: false,
            checkmark_at_start: false,
            state: ControlState::default(),
        }
    }

    pub fn leading(mut self, content: impl IntoElement) -> Self {
        self.leading = Some(content.into_any_element());
        self
    }

    pub fn trailing(mut self, content: impl IntoElement) -> Self {
        self.trailing = Some(content.into_any_element());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// Model menus reserve the checkmark column before the label.
    pub fn checkmark_at_start(mut self) -> Self {
        self.checkmark_at_start = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus).tab_index(0).tab_stop(true);
        self
    }

    control_style_methods!();
}

impl Styled for SelectableRow {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for SelectableRow {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for SelectableRow {}

impl ParentElement for SelectableRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl ActivationExt for SelectableRow {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.state.activate = Some(activation_listener(cx, handler));
        self
    }
}

impl RenderOnce for SelectableRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let base = with_defaults(self.base, |base| {
            base.w_full()
                .h(px(26.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .relative()
                .flex()
                .items_center()
                .gap(px(8.0))
                .cursor_default()
                .text_style(TextStyle::Body)
                .text_color(if self.state.disabled {
                    colors.text_ghost
                } else {
                    colors.text
                })
                .when(self.highlighted, |base| base.bg(colors.overlay_strong))
        });
        let mut focus = self
            .state
            .focus_visible
            .map(|style| *style)
            .unwrap_or_default();
        focus.box_shadow.get_or_insert_default().push(
            gpui::BoxShadow::new(px(0.0), px(0.0), colors.accent)
                .spread_radius(px(1.0))
                .inset(),
        );
        let base = base
            .focus_visible(|_| focus)
            .when(!self.state.disabled, |base| {
                let mut hover = StyleRefinement::default().bg(colors.overlay);
                if let Some(explicit) = self.state.hover {
                    hover.refine(&explicit);
                }
                let mut active = StyleRefinement::default().opacity(0.85);
                if let Some(explicit) = self.state.active {
                    active.refine(&explicit);
                }
                base.hover(|_| hover).active(|_| active)
            });
        let base = base
            .when(self.checkmark_at_start, |base| {
                base.child(
                    div()
                        .w(px(12.0))
                        .h(px(16.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(self.selected, |slot| {
                            slot.child(
                                SfSymbol::new("checkmark")
                                    .size(IconSize::Tiny)
                                    .weight(SymbolWeight::Bold)
                                    .w(px(10.0))
                                    .h(px(10.0)),
                            )
                        }),
                )
            })
            .children(self.leading)
            .child(div().min_w_0().flex_1().truncate().child(self.content))
            .when(self.selected && !self.checkmark_at_start, |base| {
                base.child(
                    SfSymbol::new("checkmark")
                        .size(IconSize::Tiny)
                        .color(colors.text_secondary),
                )
            })
            .children(self.trailing);
        (match self.state.activate.filter(|_| !self.state.disabled) {
            Some(handler) => with_activation(base, handler),
            None => base,
        })
        .squircle()
    }
}
