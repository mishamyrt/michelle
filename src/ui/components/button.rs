use super::control::{
    ControlState, background_color, control_style_methods, overlay_color, with_defaults,
};
use crate::ui::primitives::activation::activation_listener;
use crate::ui::primitives::timing::TOOLTIP_SHOW_DELAY_MS;
use crate::ui::squircle::SquircleExt;
use crate::ui::tokens::metrics::{radius, spacing};
use crate::ui::{
    ActivationExt, ControlSize, StyledTypography, TextStyle, Tooltip, UiColors, colors, icon,
    motion,
};
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, Hsla, InteractiveElement, Interactivity,
    RenderOnce, SharedString, Stateful, StyleRefinement, Styled, Window, div, prelude::*, px, rgb,
    transparent_black,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    #[default]
    Secondary,
    Ghost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonRole {
    #[default]
    Normal,
    Destructive,
}

fn button_colors(style: ButtonStyle, role: ButtonRole, palette: UiColors) -> (Hsla, Hsla, Hsla) {
    match (style, role) {
        (ButtonStyle::Secondary, ButtonRole::Destructive) => {
            (palette.danger_soft, palette.danger, palette.danger_soft)
        }
        (ButtonStyle::Primary, ButtonRole::Destructive) => {
            (palette.danger, rgb(0xFFFFFF).into(), palette.danger)
        }
        (ButtonStyle::Primary, ButtonRole::Normal) => {
            (palette.inverse, palette.on_inverse, palette.inverse)
        }
        (ButtonStyle::Secondary, ButtonRole::Normal) => {
            (palette.raised, palette.text, palette.border_strong)
        }
        (ButtonStyle::Ghost, role) => (
            transparent_black(),
            if role == ButtonRole::Destructive {
                palette.danger
            } else {
                palette.text
            },
            transparent_black(),
        ),
    }
}

/// A stateless push button. Activation is wired at render time so disabled
/// controls stay inert regardless of builder-method order.
#[derive(IntoElement)]
pub struct Button {
    base: Stateful<Div>,
    label: SharedString,
    appearance: ButtonStyle,
    role: ButtonRole,
    size: ControlSize,
    state: ControlState,
    icon: Option<AnyElement>,
    loading: bool,
    tooltip: Option<SharedString>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            base: div().id(id).tab_index(0),
            label: label.into(),
            appearance: ButtonStyle::default(),
            role: ButtonRole::default(),
            size: ControlSize::default(),
            state: ControlState::default(),
            icon: None,
            loading: false,
            tooltip: None,
        }
    }

    pub fn button_style(mut self, style: ButtonStyle) -> Self {
        self.appearance = style;
        self
    }
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    control_style_methods!();

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
        self.state.activate = Some(activation_listener(cx, activate));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = colors(cx);
        let (background, foreground, border) = button_colors(self.appearance, self.role, palette);
        let mut button = with_defaults(self.base, |base| {
            base.h(self.size.height())
                .px(px(spacing::MD))
                .rounded(px(radius::CONTROL))
                .border_1()
                .border_color(border)
                .bg(background)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(spacing::SM))
                .cursor_default()
                .text_color(foreground)
                .text_style(TextStyle::BODY)
        });
        let background = background_color(&mut button, background);
        let foreground = button.style().text.color.unwrap_or(foreground);
        let button = self.state.apply(
            button,
            palette,
            StyleRefinement::default().bg(overlay_color(background, palette.overlay)),
            StyleRefinement::default().bg(overlay_color(background, palette.overlay_strong)),
        );
        button
            .when_some(self.tooltip, |button, text| {
                button
                    .tooltip(Tooltip::text(text))
                    .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
            })
            .when(self.loading, |button| {
                button.child(motion::spinner(13.0, foreground))
            })
            .when_some(self.icon.filter(|_| !self.loading), |button, icon| {
                button.child(icon)
            })
            .child(self.label)
            .squircle()
    }
}

/// A square button accepting any icon element. Its owner supplies the label
/// through `tooltip` and retains the focus handle.
#[derive(IntoElement)]
pub struct IconButton {
    base: Stateful<Div>,
    icon: AnyElement,
    size: ControlSize,
    state: ControlState,
    loading: bool,
    tooltip: Option<SharedString>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: impl IntoElement) -> Self {
        Self {
            base: div().id(id).tab_index(0),
            icon: icon.into_any_element(),
            size: ControlSize::Mini,
            state: ControlState::default(),
            loading: false,
            tooltip: None,
        }
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
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

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus);
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    control_style_methods!();
}

impl Styled for IconButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for IconButton {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ActivationExt for IconButton {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.state.activate = Some(activation_listener(cx, activate));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = colors(cx);
        let mut button = with_defaults(self.base, |base| {
            base.size(self.size.height())
                .rounded(px(radius::CONTROL))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_tertiary)
                .cursor_default()
        });
        let background = background_color(&mut button, transparent_black());
        let foreground = button.style().text.color.unwrap_or(theme.text_tertiary);
        self.state
            .apply(
                button,
                theme,
                StyleRefinement::default().bg(overlay_color(background, theme.overlay)),
                StyleRefinement::default().bg(overlay_color(background, theme.overlay_strong)),
            )
            .when_some(self.tooltip, |button, text| {
                button
                    .tooltip(Tooltip::text(text))
                    .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
            })
            .child(if self.loading {
                motion::spinner(13.0, foreground)
            } else {
                self.icon
            })
            .squircle()
    }
}

#[cfg(test)]
mod tests;

/// A raw ghost menu trigger for callers attaching their own GPUI handlers.
pub fn icon_button(
    id: impl Into<ElementId>,
    name: &'static str,
    theme: UiColors,
) -> crate::ui::squircle::SquircleElement<Stateful<Div>> {
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
        .squircle()
}
