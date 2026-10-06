use gpui::{BoxShadow, Div, Hsla, Stateful, StyleRefinement, Styled, prelude::*, px};

use crate::ui::UiColors;
use crate::ui::primitives::activation::{ActivationHandler, with_activation};

/// Move only styles; the element keeps its identity, children and handlers.
pub(super) fn with_defaults<E: Styled>(mut element: E, defaults: impl FnOnce(E) -> E) -> E {
    let explicit = std::mem::take(element.style());
    let mut element = defaults(element);
    element.style().refine(&explicit);
    element
}

pub(super) fn background_color(element: &mut impl Styled, fallback: Hsla) -> Hsla {
    element
        .style()
        .background
        .as_ref()
        .and_then(|fill| fill.color())
        .and_then(|color| color.as_solid())
        .unwrap_or(fallback)
}

#[derive(Default)]
pub(super) struct ControlState {
    pub disabled: bool,
    pub activate: Option<ActivationHandler>,
    pub hover: Option<Box<StyleRefinement>>,
    pub active: Option<Box<StyleRefinement>>,
    pub focus_visible: Option<Box<StyleRefinement>>,
}

impl ControlState {
    pub fn apply(
        self,
        mut element: Stateful<Div>,
        theme: UiColors,
        hover: StyleRefinement,
        active: StyleRefinement,
    ) -> Stateful<Div> {
        let mut focus = self.focus_visible.map(|style| *style).unwrap_or_default();
        // The inset ring is mandatory, even with a caller's focus shadow.
        // shortcut: GPUI applies hover/active shadows after focus; preserve the ring there until state priority is controllable.
        focus.box_shadow.get_or_insert_default().push(
            BoxShadow::new(px(0.0), px(0.0), theme.accent)
                .spread_radius(px(1.0))
                .inset(),
        );
        element = element.focus_visible(|_| focus);
        if self.disabled {
            element.opacity(0.55)
        } else {
            element = element
                .hover(|_| merge_state(hover, self.hover))
                .active(|_| merge_state(active, self.active));
            if let Some(activate) = self.activate {
                element = with_activation(element, activate);
            }
            element
        }
    }
}

fn merge_state(
    mut defaults: StyleRefinement,
    explicit: Option<Box<StyleRefinement>>,
) -> StyleRefinement {
    if let Some(explicit) = explicit {
        defaults.refine(&explicit);
    }
    defaults
}

// GPUI installs hover only once. Store refinements and install each state once
// after the component's defaults have been resolved.
macro_rules! control_style_methods {
    () => {
        pub fn hover(
            mut self,
            style: impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement,
        ) -> Self {
            let previous = self
                .state
                .hover
                .take()
                .map(|style| *style)
                .unwrap_or_default();
            self.state.hover = Some(Box::new(style(previous)));
            self
        }

        pub fn active(
            mut self,
            style: impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement,
        ) -> Self {
            let previous = self
                .state
                .active
                .take()
                .map(|style| *style)
                .unwrap_or_default();
            self.state.active = Some(Box::new(style(previous)));
            self
        }

        pub fn focus_visible(
            mut self,
            style: impl FnOnce(gpui::StyleRefinement) -> gpui::StyleRefinement,
        ) -> Self {
            let previous = self
                .state
                .focus_visible
                .take()
                .map(|style| *style)
                .unwrap_or_default();
            self.state.focus_visible = Some(Box::new(style(previous)));
            self
        }
    };
}

pub(super) use control_style_methods;

#[cfg(test)]
mod tests;
