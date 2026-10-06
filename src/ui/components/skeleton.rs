use super::control::with_defaults;
use crate::ui::squircle::SquircleExt;
use crate::ui::{UiColors, motion};
use gpui::{App, Div, RenderOnce, StyleRefinement, Window, div, prelude::*, pulsating_between, px};
use std::time::Duration;

#[derive(IntoElement)]
pub struct Skeleton {
    base: Div,
}

impl Skeleton {
    pub fn new() -> Self {
        Self { base: div() }
    }

    /// Pulse the prepared group, keeping one clock lease for all its blocks.
    pub fn pulse<E: Styled + IntoElement + 'static>(content: E) -> motion::Pulse {
        motion::pulse(Duration::from_millis(1400), move |phase| {
            content
                .opacity(pulsating_between(0.45, 0.9)(phase))
                .into_any_element()
        })
        .every(2)
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        with_defaults(self.base, |base| {
            base.h(px(9.0))
                .w_full()
                .flex_none()
                .rounded(px(4.5))
                .bg(UiColors::current(cx).overlay_strong)
        })
        .squircle()
    }
}
