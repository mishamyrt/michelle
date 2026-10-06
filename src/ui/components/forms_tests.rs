use gpui::{ClipboardItem, Context, Entity, Render, TestAppContext, Window, prelude::*, px, rgb};

use crate::ui::{
    Button, Divider, FormRow, SearchField, Section, Surface, TextArea, TextInput, UiColors, v_stack,
};

struct FormHarness {
    width: f32,
    content_size: f32,
}

impl Render for FormHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        v_stack().w(px(self.width)).text_size(px(self.content_size)).child(
            Surface::new()
                .child(Section::new("Отображение").description("Настройки ответов"))
                .child(Divider::horizontal().inset(px(20.0)))
                .child(
                    FormRow::new("Размер текста контента")
                        .description("Размер текста сообщений и документов, включая длинные описания настроек на русском языке, без изменения размеров элементов интерфейса.")
                        .bg(rgb(0x123456))
                        .control(Button::new("control", "Выбрать")
                            .flex_none().w(px(116.0)).h(px(30.0)).bg(rgb(0x654321))),
                ),
        )
    }
}

#[gpui::test]
fn narrow_forms_wrap_descriptions_keep_controls_and_ignore_content_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| FormHarness {
        width: 330.0,
        content_size: 13.0,
    });
    let bounds = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            let quads = crate::ui::squircle::test_support::painted_surfaces(window, cx);
            let find = |color: gpui::Hsla| {
                quads
                    .iter()
                    .find(|quad| quad.background.as_solid() == Some(color))
                    .unwrap()
                    .bounds
            };
            let row = find(rgb(0x123456).into());
            let control = find(rgb(0x654321).into());
            let scale = window.scale_factor();
            assert_eq!(control.size.width.0, 116.0 * scale);
            assert_eq!(control.size.height.0, 30.0 * scale);
            assert!(control.right().0 <= row.right().0 - 20.0 * scale);
            assert!(control.left().0 >= row.left().0 + 20.0 * scale);
            find(UiColors::current(cx).raised);
            (row, control)
        })
    };
    let narrow = bounds(cx);
    assert!(narrow.0.size.height.0 > 60.0);
    for colors in [UiColors::light(), UiColors::dark()] {
        cx.update(|window, cx| {
            crate::ui::appearance::set_colors(colors, cx);
            view.update(cx, |view, cx| {
                view.content_size = 28.0;
                cx.notify();
            });
            window.refresh();
        });
        assert_eq!(
            bounds(cx),
            narrow,
            "content size and theme must preserve form geometry"
        );
    }
    view.update(cx, |view, cx| {
        view.width = 650.0;
        cx.notify();
    });
    let wide = bounds(cx);
    assert!(
        wide.0.size.height < narrow.0.size.height,
        "descriptions must wrap rather than truncate"
    );
}

struct SearchHarness {
    input: Entity<TextInput>,
    width: f32,
}

impl Render for SearchHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        SearchField::new("search", self.input.clone()).w(px(self.width))
    }
}

#[gpui::test]
fn search_field_preserves_the_owners_input_focus_and_undo_across_renders(cx: &mut TestAppContext) {
    cx.update(crate::input::init);
    let (view, cx) = cx.add_window_view(|window, cx| SearchHarness {
        input: cx.new(|cx| {
            let mut input = TextInput::new(window, cx);
            input.set_content("saved query", cx);
            let end = input.content().len();
            input.select_range(end..end, cx);
            input
        }),
        width: 300.0,
    });
    let input = view.read_with(cx, |view, _| view.input.clone());
    let focus = input.read_with(cx, |input, _| input.focus());
    cx.update(|window, cx| {
        window.focus(&focus, cx);
        cx.write_to_clipboard(ClipboardItem::new_string(" appended".to_owned()));
    });
    cx.simulate_keystrokes("secondary-v");
    input.read_with(cx, |input, _| {
        assert_eq!(input.content(), "saved query appended")
    });
    view.update(cx, |view, cx| {
        view.width = 220.0;
        cx.notify();
    });
    view.read_with(cx, |view, _| assert_eq!(view.input, input));
    cx.update(|window, _| assert!(focus.is_focused(window)));
    cx.simulate_keystrokes("secondary-z");
    input.read_with(cx, |input, _| assert_eq!(input.content(), "saved query"));
}

struct AreaHarness {
    input: Entity<TextInput>,
    text_size: f32,
}

impl Render for AreaHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        TextArea::new("area", self.input.clone())
            .w(px(300.0))
            .text_size(px(self.text_size))
            .line_height(px(self.text_size * 1.5))
    }
}

#[gpui::test]
fn text_area_retains_draft_undo_and_real_readonly_input_behavior(cx: &mut TestAppContext) {
    cx.update(crate::input::init);
    let (view, cx) = cx.add_window_view(|window, cx| AreaHarness {
        input: cx.new(|cx| {
            let mut input = TextInput::new(window, cx).multi_line();
            input.set_content("draft\nobjective", cx);
            let end = input.content().len();
            input.select_range(end..end, cx);
            input
        }),
        text_size: 13.0,
    });
    let input = view.read_with(cx, |view, _| view.input.clone());
    cx.update(|window, cx| {
        window.focus(&input.read(cx).focus(), cx);
        cx.write_to_clipboard(ClipboardItem::new_string("\nmore".to_owned()));
    });
    cx.simulate_keystrokes("secondary-v");
    input.read_with(cx, |input, _| {
        assert_eq!(input.content(), "draft\nobjective\nmore")
    });
    view.update(cx, |view, cx| {
        view.text_size = 20.0;
        cx.notify();
    });
    view.read_with(cx, |view, _| assert_eq!(view.input, input));
    cx.simulate_keystrokes("secondary-z");
    input.read_with(cx, |input, _| {
        assert_eq!(input.content(), "draft\nobjective")
    });
    input.update(cx, |input, cx| {
        input.set_read_only(true);
        cx.notify();
    });
    cx.simulate_keystrokes("secondary-v backspace secondary-z");
    input.read_with(cx, |input, _| {
        assert_eq!(input.content(), "draft\nobjective")
    });
    cx.simulate_keystrokes("secondary-a secondary-c");
    cx.update(|window, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("draft\nobjective")
        );
        assert!(input.read(cx).focus().is_focused(window));
    });
}
