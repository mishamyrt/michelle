use crate::ui::squircle::SquircleExt;
use gpui::{AnyElement, App, Div, RenderOnce, StyleRefinement, Window, prelude::*, px};

use super::control::with_defaults;
use crate::ui::UiColors;

/// A raised card. Grouped rows own their padding; other content can set it here.
/// Rounded corners do not clip child paint: edge children need their own radii.
#[derive(IntoElement)]
pub struct Surface {
    base: Div,
}

impl Surface {
    pub fn new() -> Self {
        Self { base: gpui::div() }
    }
}

impl Styled for Surface {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Surface {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for Surface {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        surface_defaults(self.base, UiColors::current(cx)).squircle()
    }
}

pub(super) fn surface_defaults<E: Styled>(base: E, colors: UiColors) -> E {
    with_defaults(base, |base| {
        base.w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .bg(colors.raised)
    })
}
