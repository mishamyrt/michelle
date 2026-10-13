use crate::md::{
    self,
    render::{
        Ctx as MarkdownCtx, Metrics as MarkdownMetrics, Palette as MarkdownPalette,
        TranscriptSelection,
    },
};
use crate::ui::{
    ActivationExt, Badge, ControlSize, IconButton, InlineNotice, ProgressBar, ProgressRing,
    StatusTone, TextStyle, Toast, UiColors,
};
use gpui::{
    ClipboardItem, Context, FocusHandle, FontWeight, KeyDownEvent, Modifiers, Render,
    TestAppContext, Window, canvas, div, point, prelude::*, px, rgb,
};

struct ProgressHarness {
    fraction: Option<f64>,
    segments: bool,
    invalid_segment: bool,
    content_size: f32,
}

impl Render for ProgressHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        div()
            .w(px(200.0))
            .text_size(px(self.content_size))
            .flex()
            .flex_col()
            .child(
                ProgressBar::new(self.fraction)
                    .h(px(8.0))
                    .bg(rgb(0x123456))
                    .fill_color(rgb(0xAB1234).into())
                    .when(self.segments, |bar| {
                        bar.segments([
                            (
                                if self.invalid_segment { f64::NAN } else { 0.25 },
                                rgb(0xAB1234).into(),
                            ),
                            (0.75, rgb(0x1256AB).into()),
                        ])
                    }),
            )
            .child(ProgressRing::new(self.fraction).size(px(26.0)))
            .child(
                Badge::new("Ready")
                    .tone(StatusTone::Success)
                    .bg(rgb(0x12AB34)),
            )
            .child(
                InlineNotice::new("Some records could not be loaded")
                    .tone(StatusTone::Warning)
                    .bg(rgb(0xAB3412)),
            )
    }
}

#[gpui::test]
fn progress_clamps_before_layout_and_status_geometry_ignores_content_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| ProgressHarness {
        fraction: None,
        segments: false,
        invalid_segment: false,
        content_size: 13.0,
    });
    for (fraction, width) in [
        (None, 0.0),
        (Some(f64::NAN), 0.0),
        (Some(f64::INFINITY), 0.0),
        (Some(f64::NEG_INFINITY), 0.0),
        (Some(-10.0), 0.0),
        (Some(0.0), 0.0),
        (Some(0.5), 100.0),
        (Some(1.0), 200.0),
        (Some(100.0), 200.0),
    ] {
        view.update(cx, |view, cx| {
            view.fraction = fraction;
            cx.notify();
        });
        cx.update(|window, cx| {
            let quads = crate::ui::squircle::test_support::painted_surfaces(window, cx);
            let actual = quads
                .iter()
                .find(|quad| quad.background.as_solid() == Some(rgb(0xAB1234).into()))
                .map_or(0.0, |quad| quad.bounds.size.width.0 / window.scale_factor());
            assert_eq!(actual, width);
            for quad in quads {
                assert!(
                    quad.bounds.size.width.0.is_finite() && quad.bounds.size.height.0.is_finite()
                );
            }
        });
    }
    let bounds = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            let quads = crate::ui::squircle::test_support::painted_surfaces(window, cx);
            [0x123456, 0x12AB34, 0xAB3412].map(|color| {
                quads
                    .iter()
                    .find(|quad| quad.background.as_solid() == Some(rgb(color).into()))
                    .unwrap()
                    .bounds
            })
        })
    };
    let original = bounds(cx);
    for colors in [UiColors::light(), UiColors::dark()] {
        cx.update(|window, cx| {
            crate::ui::appearance::set_colors(colors, cx);
            view.update(cx, |view, cx| {
                view.content_size = 32.0;
                cx.notify();
            });
            window.refresh();
        });
        assert_eq!(bounds(cx), original);
    }
}

#[gpui::test]
fn progress_segments_keep_their_mix_and_invalid_segments_show_only_the_track(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, _| ProgressHarness {
        fraction: Some(0.5),
        segments: true,
        invalid_segment: false,
        content_size: 13.0,
    });
    cx.update(|window, cx| {
        let quads = crate::ui::squircle::test_support::painted_surfaces(window, cx);
        for (color, width) in [(0xAB1234, 25.0), (0x1256AB, 75.0)] {
            let quad = quads
                .iter()
                .find(|quad| quad.background.as_solid() == Some(rgb(color).into()))
                .unwrap();
            assert_eq!(quad.bounds.size.width.0 / window.scale_factor(), width);
            if color == 0xAB1234 {
                assert!(quad.corner_radii.top_left.0 > 0.0);
                assert_eq!(quad.corner_radii.top_right.0, 0.0);
            } else {
                assert_eq!(quad.corner_radii.top_left.0, 0.0);
                assert!(quad.corner_radii.top_right.0 > 0.0);
            }
        }
    });
    view.update(cx, |view, cx| {
        view.invalid_segment = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        assert!(
            !crate::ui::squircle::test_support::painted_surfaces(window, cx)
                .iter()
                .any(|quad| {
                    quad.background.as_solid().is_some_and(|color| {
                        [rgb(0xAB1234).into(), rgb(0x1256AB).into()].contains(&color)
                    })
                })
        );
    });
}

struct ToastHarness {
    selection: TranscriptSelection,
    focus: FocusHandle,
    message: String,
    visible: bool,
    hovered: bool,
    dismisses: usize,
    parent_activations: usize,
}

impl Render for ToastHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        let colors = UiColors::current(cx);
        let theme = crate::theme::Theme::dark();
        let palette = MarkdownPalette::from_theme(&theme);
        let text_ctx = MarkdownCtx::new(
            "toast",
            &palette,
            MarkdownMetrics {
                text_size: TextStyle::BODY.size().into(),
                line_height: TextStyle::BODY.line_height().into(),
                ..MarkdownMetrics::COMPACT
            },
            self.selection.clone(),
        );
        let content = md::render::plain_text(
            self.message.clone(),
            md::render::SANS_FAMILY,
            FontWeight::NORMAL,
            colors.text,
            &text_ctx,
        );
        let selection = self.selection.clone();
        div()
            .id("parent")
            .p(px(8.0))
            .on_activation(cx, |this, _, _| this.parent_activations += 1)
            .when(self.visible, |root| {
                root.child(
                    Toast::new("toast", content)
                        .w(px(340.0))
                        .child(md::render::frame_reset(self.selection.clone()))
                        .on_hover(
                            cx.listener(|this, hovering: &bool, _, _| this.hovered = *hovering),
                        )
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape"
                                && !event.keystroke.modifiers.modified()
                            {
                                this.visible = false;
                                this.dismisses += 1;
                                cx.notify();
                                cx.stop_propagation();
                            }
                        }))
                        .dismiss_button(
                            IconButton::new("dismiss", div())
                                .control_size(ControlSize::Small)
                                .track_focus(&self.focus)
                                .bg(rgb(0xAB1234))
                                .on_activation(cx, |this, _, cx| {
                                    this.visible = false;
                                    this.dismisses += 1;
                                    cx.notify();
                                }),
                        )
                        .overlay(
                            canvas(
                                |_, _, _| (),
                                move |_, _, window, _| {
                                    md::render::install_selection_input(window, &selection)
                                },
                            )
                            .absolute()
                            .size(px(0.0)),
                        ),
                )
            })
    }
}

#[gpui::test]
fn toast_keeps_selection_hover_and_independent_keyboard_dismissal(cx: &mut TestAppContext) {
    let message = "Ошибка: файл не найден.\nПроверьте путь.";
    let (view, cx) = cx.add_window_view(|_, cx| ToastHarness {
        selection: TranscriptSelection::default(),
        focus: cx.focus_handle(),
        message: message.into(),
        visible: true,
        hovered: false,
        dismisses: 0,
        parent_activations: 0,
    });
    view.update(cx, |view, cx| {
        let registry = view.selection.registry.borrow();
        assert_eq!(registry.entries().len(), 1);
        let entry = &registry.entries()[0];
        assert_eq!(entry.text.as_ref(), message);
        view.selection
            .selection
            .borrow_mut()
            .set_spans(registry.resolve((0, 0), (0, message.len())));
        cx.write_to_clipboard(ClipboardItem::new_string(
            view.selection.selection.borrow().text(),
        ));
        cx.notify();
    });
    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), message);
    view.read_with(cx, |view, _| {
        assert_eq!(view.selection.selection.borrow().text(), message)
    });
    cx.simulate_mouse_move(point(px(20.0), px(20.0)), None, Modifiers::none());
    view.read_with(cx, |view, _| assert!(view.hovered));
    cx.simulate_mouse_move(point(px(600.0), px(400.0)), None, Modifiers::none());
    view.read_with(cx, |view, _| assert!(!view.hovered));
    let focus = view.read_with(cx, |view, _| view.focus.clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_keystrokes("cmd-enter shift-space alt-escape");
    view.read_with(cx, |view, _| assert_eq!(view.dismisses, 0));
    for (index, key) in ["enter", "space", "escape"].into_iter().enumerate() {
        view.update(cx, |view, cx| {
            view.visible = true;
            cx.notify();
        });
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes(key);
        view.read_with(cx, |view, _| {
            assert_eq!(view.dismisses, index + 1);
            assert_eq!(view.parent_activations, 0);
        });
    }
    view.update(cx, |view, cx| {
        view.visible = true;
        cx.notify();
    });
    let target = cx.update(|window, cx| {
        let quad = crate::ui::squircle::test_support::painted_surfaces(window, cx)
            .into_iter()
            .find(|quad| quad.background.as_solid() == Some(rgb(0xAB1234).into()))
            .unwrap();
        let scale = window.scale_factor();
        point(
            px((quad.bounds.left().0 + 5.0) / scale),
            px((quad.bounds.top().0 + 5.0) / scale),
        )
    });
    cx.simulate_click(target, Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.dismisses, 4);
        assert_eq!(view.parent_activations, 0);
    });
}
