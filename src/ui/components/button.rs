use crate::ui::primitives::activation::{ActivationHandler, activation_listener, with_activation};
use crate::ui::tokens::metrics::{radius, spacing};
use crate::ui::{ActivationExt, ControlSize, Label, TextStyle, Tooltip, UiColors, colors, icon};
use gpui::{
    App, BoxShadow, Context, Div, ElementId, FocusHandle, RenderOnce, SharedString, Stateful,
    StyleRefinement, Styled, Window, div, prelude::*, px, transparent_black,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    #[default]
    Secondary,
    Ghost,
    Destructive,
}

/// A stateless push button. Activation is wired at render time so disabled
/// controls stay inert regardless of builder-method order.
#[derive(IntoElement)]
pub struct Button {
    base: Stateful<Div>,
    label: SharedString,
    appearance: ButtonStyle,
    size: ControlSize,
    disabled: bool,
    activate: Option<ActivationHandler>,
    tooltip: Option<SharedString>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            base: div().id(id),
            label: label.into(),
            appearance: ButtonStyle::default(),
            size: ControlSize::default(),
            disabled: false,
            activate: None,
            tooltip: None,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.appearance = style;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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
}

impl ActivationExt for Button {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.activate = Some(activation_listener(cx, activate));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = colors(cx);
        let (background, foreground, border) = match self.appearance {
            ButtonStyle::Primary => (palette.inverse, palette.on_inverse, palette.inverse),
            ButtonStyle::Secondary => (palette.raised, palette.text, palette.border_strong),
            ButtonStyle::Ghost => (transparent_black(), palette.text, transparent_black()),
            ButtonStyle::Destructive => (palette.danger_soft, palette.danger, palette.danger_soft),
        };
        let mut button = self
            .base
            .tab_index(0)
            .focus_visible(|style| {
                style.shadow(vec![
                    BoxShadow::new(px(0.0), px(0.0), palette.accent)
                        .spread_radius(px(1.0))
                        .inset(),
                ])
            })
            .h(self.size.height())
            .px(px(spacing::MD))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(border)
            .bg(background)
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .text_color(foreground)
            .when(self.disabled, |button| button.opacity(0.55))
            .when(!self.disabled, |button| {
                button
                    .hover(|style| style.bg(background.blend(palette.overlay)))
                    .active(|style| style.bg(background.blend(palette.overlay_strong)))
            })
            .when_some(self.tooltip, |button, text| {
                button.tooltip(Tooltip::text(text))
            })
            .child(Label::new(self.label).text_style(TextStyle::Body));
        if let Some(activate) = self.activate.filter(|_| !self.disabled) {
            button = with_activation(button, activate);
        }
        button
    }
}

#[cfg(test)]
mod tests;

/// A compact ghost icon button: the only button shape outside the composer's
/// bespoke send control.
pub fn icon_button(id: impl Into<ElementId>, name: &'static str, theme: UiColors) -> Stateful<Div> {
    div()
        .id(id)
        .size(ControlSize::Mini.height())
        .rounded(px(radius::CONTROL))
        .flex()
        .items_center()
        .justify_center()
        .cursor_default()
        .hover(|element| element.bg(theme.overlay))
        .active(|element| element.bg(theme.overlay_strong))
        .child(icon(name, 13.0, theme.text_tertiary))
}
