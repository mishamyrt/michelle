use gpui::{
    Context, FocusHandle, IntoElement, Modifiers, MouseButton, Render, TestAppContext, Window, div,
    point, prelude::*, px, rgb,
};

use crate::ui::{
    ActionRow, ActivationExt, Button, Checkbox, IconButton, MenuChip, SelectableRow, TextField,
    TextInput, Toggle, ToolbarButton,
};

#[test]
fn hover_and_active_overlays_stay_visible_on_transparent_controls() {
    for theme in [crate::ui::UiColors::light(), crate::ui::UiColors::dark()] {
        for overlay in [theme.overlay, theme.overlay_strong] {
            let color = super::overlay_color(gpui::transparent_black(), overlay);
            assert!(!color.is_transparent());
            assert_eq!(color, overlay);
            for background in [theme.control_fill, rgb(0x123456).into()] {
                assert_eq!(
                    super::overlay_color(background, overlay),
                    background.blend(overlay)
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Button,
    Icon,
    Toolbar,
    Chip,
    Toggle,
    Action,
    Checkbox,
    Selectable,
}

struct Harness {
    kind: Kind,
    focus: FocusHandle,
    disabled: bool,
    disabled_first: bool,
    loading: bool,
    activations: usize,
    parent_activations: usize,
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        macro_rules! action {
            ($control:expr) => {
                $control
                    .track_focus(&self.focus)
                    .disabled(self.disabled && self.disabled_first)
                    .on_activation(cx, |this, _, _| this.activations += 1)
                    .when(!self.disabled_first, |control| {
                        control.disabled(self.disabled)
                    })
                    .w(px(80.0))
                    .h(px(44.0))
                    .into_any_element()
            };
        }
        let control = match self.kind {
            Kind::Button => action!(Button::new("control", "Save").loading(self.loading)),
            Kind::Icon => action!(IconButton::new("control", div()).loading(self.loading)),
            Kind::Toolbar => {
                action!(ToolbarButton::new("control", "arrow.clockwise").loading(self.loading))
            }
            Kind::Chip => action!(MenuChip::new("control").label("Menu")),
            Kind::Toggle => action!(Toggle::new("control", false)),
            Kind::Action => action!(ActionRow::new("control", "Commit").loading(self.loading)),
            Kind::Checkbox => action!(Checkbox::new("control", false, "Include")),
            Kind::Selectable => action!(SelectableRow::new("control", "Model")),
        };
        div()
            .id("parent")
            .flex()
            .p(px(8.0))
            .on_activation(cx, |this, _, _| this.parent_activations += 1)
            .child(control)
    }
}

#[gpui::test]
fn controls_activate_once_and_resolve_disabled_after_builders(cx: &mut TestAppContext) {
    for kind in [
        Kind::Button,
        Kind::Icon,
        Kind::Toolbar,
        Kind::Chip,
        Kind::Toggle,
        Kind::Action,
        Kind::Checkbox,
        Kind::Selectable,
    ] {
        for disabled in [false, true] {
            for disabled_first in [false, true] {
                let focus = cx.update(|cx| cx.focus_handle());
                let (view, cx) = cx.add_window_view(|_, _| Harness {
                    kind,
                    focus: focus.clone(),
                    disabled,
                    disabled_first,
                    // Loading alone must leave activation enabled.
                    loading: true,
                    activations: 0,
                    parent_activations: 0,
                });
                cx.update(|window, cx| window.focus(&focus, cx));
                cx.simulate_keystrokes("enter space cmd-enter shift-space alt-enter ctrl-space");
                cx.simulate_click(point(px(20.0), px(20.0)), Modifiers::none());
                view.read_with(cx, |view, _| {
                    assert_eq!(view.activations, if disabled { 0 } else { 3 }, "{kind:?}");
                    if !disabled {
                        assert_eq!(view.parent_activations, 0, "{kind:?}");
                    }
                });
            }
        }
    }
}

struct ToggleHarness {
    checkbox: bool,
    focus: FocusHandle,
    value: bool,
    disabled: bool,
    proposed: Vec<bool>,
}

impl Render for ToggleHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        let on_change = |this: &mut Self, value, _: &mut Window, cx: &mut Context<Self>| {
            this.proposed.push(value);
            this.value = value;
            cx.notify();
        };
        let control = if self.checkbox {
            Checkbox::new("toggle", self.value, "Include unstaged")
                .track_focus(&self.focus)
                .on_change(cx, on_change)
                .disabled(self.disabled)
                .into_any_element()
        } else {
            Toggle::new("toggle", self.value)
                .track_focus(&self.focus)
                .on_change(cx, on_change)
                .disabled(self.disabled)
                .into_any_element()
        };
        div().p(px(8.0)).child(control)
    }
}

#[gpui::test]
fn boolean_controls_propose_values_from_the_latest_owner_render_and_keep_focus(
    cx: &mut TestAppContext,
) {
    for checkbox in [false, true] {
        let focus = cx.update(|cx| cx.focus_handle());
        let (view, cx) = cx.add_window_view(|_, _| ToggleHarness {
            checkbox,
            focus: focus.clone(),
            value: false,
            disabled: false,
            proposed: Vec::new(),
        });
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("enter space cmd-enter");
        cx.simulate_click(point(px(20.0), px(16.0)), Modifiers::none());
        view.read_with(cx, |view, _| assert_eq!(view.proposed, [true, false, true]));
        cx.update(|window, cx| {
            window.focus(&focus, cx);
            view.update(cx, |view, cx| {
                view.disabled = true;
                cx.notify();
            });
        });
        cx.simulate_keystrokes("enter space");
        cx.simulate_click(point(px(20.0), px(16.0)), Modifiers::none());
        view.read_with(cx, |view, _| assert_eq!(view.proposed, [true, false, true]));
        cx.update(|window, _| assert!(focus.is_focused(window)));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum StyleKind {
    Button,
    Icon,
    Toolbar,
    Chip,
    Toggle,
    Field,
}

struct StyleHarness {
    kind: StyleKind,
    input: gpui::Entity<TextInput>,
}

impl Render for StyleHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        macro_rules! styled {
            ($control:expr) => {
                $control
                    .w(px(80.0))
                    .h(px(44.0))
                    .px(px(12.0))
                    .bg(rgb(0xAB1234))
                    .hover(|style| style.bg(rgb(0x1256AB)))
                    .hover(|style| style.text_color(rgb(0x123456)))
                    .into_any_element()
            };
        }
        let control = match self.kind {
            StyleKind::Button => {
                styled!(Button::new("styled", "Save").active(|style| style.bg(rgb(0x56AB12))))
            }
            StyleKind::Icon => {
                styled!(IconButton::new("styled", div()).active(|style| style.bg(rgb(0x56AB12))))
            }
            StyleKind::Toolbar => styled!(
                ToolbarButton::new("styled", "arrow.clockwise")
                    .active(|style| style.bg(rgb(0x56AB12)))
            ),
            StyleKind::Chip => styled!(
                MenuChip::new("styled")
                    .label("Menu")
                    .outlined()
                    .active(|style| style.bg(rgb(0x56AB12)))
            ),
            StyleKind::Toggle => {
                styled!(Toggle::new("styled", true).active(|style| style.bg(rgb(0x56AB12))))
            }
            // TextField passes through native GPUI hover without control defaults.
            StyleKind::Field => TextField::new("styled", self.input.clone())
                .w(px(80.0))
                .h(px(44.0))
                .px(px(12.0))
                .bg(rgb(0xAB1234))
                .hover(|style| style.bg(rgb(0x1256AB)))
                .into_any_element(),
        };
        div().p(px(8.0)).child(control)
    }
}

#[gpui::test]
fn caller_geometry_and_state_colors_reach_gpui_without_duplicate_hover(cx: &mut TestAppContext) {
    let check = |cx: &mut gpui::VisualTestContext, color: gpui::Hsla| {
        cx.update(|window, cx| {
            let quad = crate::ui::squircle::test_support::painted_surfaces(window, cx)
                .into_iter()
                .find(|quad| quad.background.as_solid() == Some(color))
                .expect("caller background must survive render");
            let scale = window.scale_factor();
            assert_eq!(quad.bounds.size.width.0, 80.0 * scale);
            assert_eq!(quad.bounds.size.height.0, 44.0 * scale);
        });
    };
    for kind in [
        StyleKind::Button,
        StyleKind::Icon,
        StyleKind::Toolbar,
        StyleKind::Chip,
        StyleKind::Toggle,
        StyleKind::Field,
    ] {
        let (_, cx) = cx.add_window_view(|window, cx| StyleHarness {
            kind,
            input: cx.new(|cx| TextInput::new(window, cx)),
        });
        check(cx, rgb(0xAB1234).into());
        cx.simulate_mouse_move(point(px(20.0), px(20.0)), None, Modifiers::none());
        check(cx, rgb(0x1256AB).into());
        if kind != StyleKind::Field {
            cx.simulate_mouse_down(
                point(px(20.0), px(20.0)),
                MouseButton::Left,
                Modifiers::none(),
            );
            check(cx, rgb(0x56AB12).into());
        }
    }
}

#[test]
fn defaults_preserve_explicit_padding_typography_and_border() {
    let mut element = super::with_defaults(
        div()
            .px(px(12.0))
            .text_size(px(17.0))
            .border_color(rgb(0x123456)),
        |element| {
            element
                .px(px(8.0))
                .text_size(px(10.0))
                .border_color(rgb(0xFFFFFF))
        },
    );
    let style = element.style();
    assert_eq!(style.padding.left, Some(px(12.0).into()));
    assert_eq!(style.padding.right, Some(px(12.0).into()));
    assert_eq!(style.text.font_size, Some(px(17.0).into()));
    assert_eq!(style.border_color, Some(rgb(0x123456).into()));
}

#[gpui::test]
fn disabled_focus_preserves_caller_treatment_and_adds_an_accent_ring(cx: &mut TestAppContext) {
    let focus = cx.update(|cx| cx.focus_handle());
    let (_, cx) = cx.add_window_view(|_, _| ToggleHarness {
        checkbox: false,
        focus: focus.clone(),
        value: false,
        disabled: true,
        proposed: Vec::new(),
    });
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_keystrokes("enter");
    cx.draw(
        point(px(0.0), px(0.0)),
        gpui::size(px(80.0), px(44.0)),
        |window, cx| {
            let theme = crate::ui::UiColors::current(cx);
            let custom_shadow = gpui::BoxShadow::new(px(0.0), px(2.0), rgb(0x123456).into());
            let mut element = super::ControlState {
                disabled: true,
                focus_visible: Some(Box::new(
                    gpui::StyleRefinement::default()
                        .bg(rgb(0xAB1234))
                        .shadow(vec![custom_shadow.clone()]),
                )),
                ..Default::default()
            }
            .apply(
                div().id("focus-probe").track_focus(&focus),
                theme,
                Default::default(),
                Default::default(),
            );
            let style = element
                .interactivity()
                .compute_style(None, None, window, cx);
            assert_eq!(style.background, Some(rgb(0xAB1234).into()));
            assert_eq!(style.opacity, Some(0.55));
            assert_eq!(style.box_shadow.len(), 2);
            assert_eq!(style.box_shadow[0], custom_shadow);
            assert_eq!(style.box_shadow[1].color, theme.accent);
            assert!(style.box_shadow[1].inset);
            assert!(focus.is_focused(window));
            element
        },
    );
}
