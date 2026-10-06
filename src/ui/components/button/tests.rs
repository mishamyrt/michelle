use gpui::{Modifiers, Render, TestAppContext, point};

use super::*;

struct Harness {
    focus: FocusHandle,
    disabled: bool,
    activations: usize,
}

impl Render for Harness {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().p(px(8.0)).child(
            Button::new("save", "Save")
                .button_style(ButtonStyle::Secondary)
                .role(ButtonRole::Destructive)
                .control_size(ControlSize::Regular)
                .track_focus(&self.focus)
                .on_activation(cx, |this, _, _| this.activations += 1)
                // Intentionally after the handler: order must not enable it.
                .disabled(self.disabled),
        )
    }
}

#[test]
fn destructive_role_preserves_colors_and_builder_order() {
    let before = Button::new("before", "Delete")
        .role(ButtonRole::Destructive)
        .button_style(ButtonStyle::Secondary)
        .control_size(ControlSize::Small);
    let after = Button::new("after", "Delete")
        .button_style(ButtonStyle::Secondary)
        .control_size(ControlSize::Small)
        .role(ButtonRole::Destructive);
    assert_eq!(before.size, ControlSize::Small);
    assert_eq!(before.size, after.size);
    for palette in [UiColors::light(), UiColors::dark()] {
        let expected = (palette.danger_soft, palette.danger, palette.danger_soft);
        assert_eq!(
            button_colors(before.appearance, before.role, palette),
            expected
        );
        assert_eq!(
            button_colors(after.appearance, after.role, palette),
            expected
        );
        let ghost = button_colors(ButtonStyle::Ghost, ButtonRole::Destructive, palette);
        assert_eq!(
            ghost,
            (transparent_black(), palette.danger, transparent_black())
        );
        assert_eq!(
            button_colors(ButtonStyle::Primary, ButtonRole::Destructive, palette),
            (palette.danger, rgb(0xFFFFFF).into(), palette.danger)
        );
    }
}

#[gpui::test]
fn activation_honors_keyboard_modifiers_and_disabled_state(cx: &mut TestAppContext) {
    for disabled in [false, true] {
        let focus = cx.update(|cx| cx.focus_handle());
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            focus: focus.clone(),
            disabled,
            activations: 0,
        });
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("enter");
        cx.simulate_keystrokes("space");
        cx.simulate_keystrokes("cmd-enter");
        cx.simulate_click(point(px(12.0), px(12.0)), Modifiers::none());
        view.read_with(cx, |view, _| {
            assert_eq!(view.activations, if disabled { 0 } else { 3 })
        });
    }
}
