use gpui::{
    AnyElement, App, BoxShadow, Context, Div, ElementId, InteractiveElement, Interactivity,
    ParentElement, RenderOnce, SharedString, Stateful, StyleRefinement, Styled, Window, div, hsla,
    point, prelude::*, px,
};

use super::tooltip::Tooltip;
use crate::ui::UiColors;
use crate::ui::primitives::timing::TOOLTIP_SHOW_DELAY_MS;
use crate::ui::{ActivationExt, ControlSize, IconSize, sf_icon};

#[derive(IntoElement)]
pub struct ToolbarButton {
    base: Stateful<Div>,
    icon: &'static str,
    tooltip: Option<SharedString>,
    selected: bool,
}

impl ToolbarButton {
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

pub fn toolbar_button(id: impl Into<ElementId>, icon: &'static str) -> ToolbarButton {
    ToolbarButton {
        base: div().id(id),
        icon,
        tooltip: None,
        selected: false,
    }
}

impl Styled for ToolbarButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for ToolbarButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl ActivationExt for ToolbarButton {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.base = self.base.on_activation(cx, activate);
        self
    }
}

impl InteractiveElement for ToolbarButton {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for ToolbarButton {}

impl RenderOnce for ToolbarButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = UiColors::current(cx);
        let button = toolbar_control(self.base, theme)
            .size_full()
            .rounded((ControlSize::Large.height() - px(1.0)) / 2.0)
            .when(self.selected, |button| {
                button.bg(theme.control_fill.blend(theme.overlay_strong))
            })
            .child(sf_icon(
                self.icon,
                IconSize::Regular.font_size(),
                theme.text,
            ));

        let button = if let Some(tooltip) = self.tooltip {
            button
                .tooltip(Tooltip::text(tooltip))
                .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
        } else {
            button
        };

        toolbar_surface(div(), theme)
            .size(ControlSize::Large.height())
            .rounded(ControlSize::Large.height() / 2.0)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(button)
    }
}

/// Two independent actions sharing the circular toolbar button's surface.
/// Build each half with `toolbar_button_segment` before attaching its handlers
/// or dropdown, so menus remain free to paint outside the capsule.
pub fn toolbar_split_button(
    primary: impl IntoElement,
    secondary: impl IntoElement,
    theme: UiColors,
) -> Div {
    toolbar_surface(div(), theme)
        .h(ControlSize::Large.height())
        .rounded(ControlSize::Large.height() / 2.0)
        .flex_none()
        .flex()
        .items_center()
        .child(primary)
        .child(secondary)
}

pub fn toolbar_button_segment(id: impl Into<ElementId>, theme: UiColors) -> Stateful<Div> {
    toolbar_control(div().id(id), theme)
        .w(ControlSize::Large.height())
        .h_full()
        // Round each half's own paint inside the capsule's border.
        .rounded((ControlSize::Large.height() - px(1.0)) / 2.0)
}

fn toolbar_control(button: Stateful<Div>, theme: UiColors) -> Stateful<Div> {
    button
        .tab_index(0)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_default()
        .focus_visible(|style| {
            style.shadow(vec![
                BoxShadow::new(px(0.0), px(0.0), theme.accent)
                    .spread_radius(px(1.0))
                    .inset(),
            ])
        })
        .hover(|style| style.bg(theme.control_fill.blend(theme.overlay)))
        .active(|style| style.bg(theme.control_fill.blend(theme.overlay_strong)))
}

fn toolbar_surface<E: Styled>(element: E, theme: UiColors) -> E {
    element
        .bg(theme.control_fill)
        .border(px(0.5))
        .border_color(theme.control_border)
        .shadow(vec![BoxShadow {
            color: hsla(0.0, 0.0, 0.0, 0.05),
            offset: point(px(0.0), px(8.0)),
            blur_radius: px(15.0),
            spread_radius: px(0.0),
            inset: false,
        }])
}
