use gpui::{App, Div, Pixels, RenderOnce, StyleRefinement, Window, div, prelude::*, px};

use super::control::with_defaults;
use crate::ui::UiColors;

#[derive(IntoElement)]
pub struct Divider {
    base: Div,
}

impl Divider {
    pub fn horizontal() -> Self {
        Self { base: div() }
    }

    pub fn inset(mut self, inset: Pixels) -> Self {
        self.base = self.base.mx(inset);
        self
    }
}

impl Styled for Divider {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Divider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        with_defaults(self.base, |base| {
            base.h(px(1.0)).flex_none().bg(UiColors::current(cx).border)
        })
    }
}
