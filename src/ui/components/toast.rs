use super::{control::with_defaults, surface::surface_defaults};
use crate::ui::squircle::SquircleExt;
use crate::ui::{StyledTypography, TextStyle, UiColors};
use gpui::{
    AnyElement, App, Div, ElementId, Interactivity, RenderOnce, Stateful, StyleRefinement, Window,
    div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct Toast {
    base: Stateful<Div>,
    content: AnyElement,
    icon: Option<AnyElement>,
    dismiss: Option<AnyElement>,
    overlay: Option<AnyElement>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            base: div().id(id),
            content: content.into_any_element(),
            icon: None,
            dismiss: None,
            overlay: None,
        }
    }
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }
    pub fn dismiss_button(mut self, button: impl IntoElement) -> Self {
        self.dismiss = Some(button.into_any_element());
        self
    }
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlay = Some(overlay.into_any_element());
        self
    }
}

impl Styled for Toast {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Toast {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}
impl StatefulInteractiveElement for Toast {}

impl ParentElement for Toast {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for Toast {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        with_defaults(self.base, |base| {
            surface_defaults(base, colors)
                .w_auto()
                .occlude()
                .max_w(px(560.0))
                .min_w_0()
                .px(px(10.0))
                .py(px(7.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(colors.border_strong)
                .shadow_lg()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .text_style(TextStyle::Body)
                .text_color(colors.text)
        })
        .on_click(|_, _, cx| cx.stop_propagation())
        .children(self.icon)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .whitespace_normal()
                .child(self.content),
        )
        .children(self.dismiss)
        .children(self.overlay)
        .squircle()
    }
}
