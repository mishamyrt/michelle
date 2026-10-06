use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, BoxShadow, Context, Div, ElementId, InteractiveElement, Interactivity,
    ParentElement, RenderOnce, SharedString, Stateful, StyleRefinement, Styled, Window, div, hsla,
    point, prelude::*, px,
};

use super::control::{ControlState, background_color, control_style_methods, with_defaults};
use super::tooltip::Tooltip;
use crate::ui::UiColors;
use crate::ui::primitives::activation::activation_listener;
use crate::ui::primitives::timing::TOOLTIP_SHOW_DELAY_MS;
use crate::ui::{ActivationExt, ControlSize, IconSize, motion, sf_icon};

#[derive(IntoElement)]
pub struct ToolbarButton {
    base: Stateful<Div>,
    icon: &'static str,
    tooltip: Option<SharedString>,
    selected: bool,
    state: ControlState,
    loading: bool,
    size: ControlSize,
}

impl ToolbarButton {
    pub fn new(id: impl Into<ElementId>, icon: &'static str) -> Self {
        Self {
            base: div().id(id).tab_index(0),
            icon,
            tooltip: None,
            selected: false,
            state: ControlState::default(),
            loading: false,
            size: ControlSize::Large,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    control_style_methods!();
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
        self.state.activate = Some(activation_listener(cx, activate));
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
        let mut button = with_defaults(self.base, |base| {
            toolbar_surface(base, theme)
                .size(self.size.height())
                .rounded(self.size.height() / 2.0)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text)
                .cursor_default()
                .when(self.selected, |button| {
                    button.bg(theme.control_fill.blend(theme.overlay_strong))
                })
        });
        let background = background_color(&mut button, theme.control_fill);
        let foreground = button.style().text.color.unwrap_or(theme.text);
        let button = self
            .state
            .apply(
                button,
                theme,
                StyleRefinement::default().bg(background.blend(theme.overlay)),
                StyleRefinement::default().bg(background.blend(theme.overlay_strong)),
            )
            .child(if self.loading {
                motion::spinner(IconSize::Regular.font_size(), foreground)
            } else {
                sf_icon(self.icon, IconSize::Regular.font_size(), foreground).into_any_element()
            });

        let button = if let Some(tooltip) = self.tooltip {
            button
                .tooltip(Tooltip::text(tooltip))
                .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
        } else {
            button
        };

        button.squircle()
    }
}

/// Two independent actions sharing the circular toolbar button's surface.
/// Build each half with `toolbar_button_segment` before attaching its handlers
/// or dropdown, so menus remain free to paint outside the capsule.
pub fn toolbar_split_button(
    primary: impl IntoElement,
    secondary: impl IntoElement,
    theme: UiColors,
) -> crate::ui::squircle::SquircleElement<Div> {
    toolbar_surface(div(), theme)
        .h(ControlSize::Large.height())
        .rounded(ControlSize::Large.height() / 2.0)
        .flex_none()
        .flex()
        .items_center()
        .child(primary)
        .child(secondary)
        .squircle()
}

pub fn toolbar_button_segment(
    id: impl Into<ElementId>,
    theme: UiColors,
) -> crate::ui::squircle::SquircleElement<Stateful<Div>> {
    toolbar_control(div().id(id), theme)
        .w(ControlSize::Large.height())
        .h_full()
        // Round each half's own paint inside the capsule's border.
        .rounded((ControlSize::Large.height() - px(1.0)) / 2.0)
        .squircle()
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
