use crate::ui::{ActionRow, ActivationExt, Dialog, TextArea, TextInput, focus_dialog_input};
use gpui::{
    ClipboardItem, Context, Entity, KeyBinding, Modifiers, MouseButton, Render, TestAppContext,
    Window, actions, div, point, prelude::*, px,
};

actions!(dialog_test, [ConfirmDialog, DismissDialog]);

struct Harness {
    composer: Entity<TextInput>,
    input: Option<Entity<TextInput>>,
    confirms: usize,
    dismisses: usize,
    operation_pending: bool,
}

impl Harness {
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextInput> {
        let input = cx.new(|cx| TextInput::new(window, cx).multi_line());
        self.input = Some(input.clone());
        let id = input.entity_id();
        focus_dialog_input(window, cx, input.read(cx).focus(), move |owner, _| {
            owner
                .input
                .as_ref()
                .is_some_and(|current| current.entity_id() == id)
        });
        cx.notify();
        input
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input = None;
        self.dismisses += 1;
        window.focus(&self.composer.read(cx).focus(), cx);
        cx.notify();
    }
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.composer.clone()).when_some(
            self.input.clone(),
            |root, input| {
                root.child(
                    Dialog::new("layer", "card")
                        .key_context("TestDialog")
                        .on_action(cx.listener(|this, _: &ConfirmDialog, _, _| {
                            this.confirms += 1;
                            this.operation_pending = true;
                        }))
                        .on_action(
                            cx.listener(|this, _: &DismissDialog, window, cx| {
                                this.close(window, cx)
                            }),
                        )
                        .on_dismiss(cx, |this, window, cx| this.close(window, cx))
                        .header(div().h(px(48.0)).child("Title"))
                        .child(TextArea::new("objective", input))
                        .actions(
                            div().p(px(8.0)).child(
                                ActionRow::new("save", "Save")
                                    .on_activation(cx, |this, _, _| this.confirms += 1),
                            ),
                        ),
                )
            },
        )
    }
}

fn setup(cx: &mut TestAppContext) -> (Entity<Harness>, &mut gpui::VisualTestContext) {
    cx.update(|cx| {
        crate::input::init(cx);
        cx.bind_keys([
            KeyBinding::new(
                "secondary-enter",
                ConfirmDialog,
                Some("TestDialog > TextInput"),
            ),
            KeyBinding::new("secondary-enter", ConfirmDialog, Some("TestDialog")),
            KeyBinding::new("escape", DismissDialog, Some("TestDialog")),
        ]);
    });
    cx.add_window_view(|window, cx| Harness {
        composer: cx.new(|cx| TextInput::new(window, cx)),
        input: None,
        confirms: 0,
        dismisses: 0,
        operation_pending: false,
    })
}

fn mount(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.update(|window, cx| window.simulate_next_frame(cx));
}

#[gpui::test]
fn dialog_confirms_dismisses_and_returns_focus_without_cancelling_operations(
    cx: &mut TestAppContext,
) {
    let (view, cx) = setup(cx);
    let input = cx.update(|window, cx| view.update(cx, |view, cx| view.open(window, cx)));
    mount(cx);
    cx.update(|window, cx| assert!(input.read(cx).focus().is_focused(window)));
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("objective".to_owned())));
    cx.simulate_keystrokes("secondary-v secondary-enter");
    view.read_with(cx, |view, _| {
        assert_eq!(view.confirms, 1);
        assert!(view.operation_pending);
    });
    input.read_with(cx, |input, _| assert_eq!(input.content(), "objective"));
    let center = cx.update(|window, _| {
        let size = window.viewport_size();
        point(size.width / 2.0, size.height / 2.0)
    });
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    view.read_with(cx, |view, _| assert_eq!(view.dismisses, 0));
    cx.simulate_mouse_down(
        point(px(2.0), px(2.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.update(|window, cx| {
        view.read_with(cx, |view, cx| {
            assert!(view.input.is_none());
            assert_eq!(view.dismisses, 1);
            assert!(view.operation_pending);
            assert!(view.composer.read(cx).focus().is_focused(window));
        })
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open(window, cx);
        })
    });
    mount(cx);
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| {
        assert!(view.input.is_none());
        assert_eq!(view.dismisses, 2);
        assert_eq!(view.confirms, 1);
    });
}

#[gpui::test]
fn deferred_focus_ignores_closed_and_replaced_dialogs(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let old = cx.update(|window, cx| view.update(cx, |view, cx| view.open(window, cx)));
    cx.update(|window, cx| view.update(cx, |view, cx| view.close(window, cx)));
    mount(cx);
    cx.update(|window, cx| {
        view.read_with(cx, |view, cx| {
            assert!(view.composer.read(cx).focus().is_focused(window));
            assert!(!old.read(cx).focus().is_focused(window));
        })
    });
    let (first, replacement) = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            (view.open(window, cx), view.open(window, cx))
        })
    });
    mount(cx);
    cx.update(|window, cx| {
        assert!(replacement.read(cx).focus().is_focused(window));
        assert!(!first.read(cx).focus().is_focused(window));
    });
}
