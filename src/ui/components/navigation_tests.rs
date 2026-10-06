use std::{cell::Cell, rc::Rc};

use gpui::{
    Context, FocusHandle, ListAlignment, ListState, Modifiers, Render, ScrollHandle, SharedString,
    TestAppContext, Window, div, list, point, prelude::*, px, rgb,
};

use crate::ui::{
    ActivationExt, IconButton, Segment, SegmentedControl, SelectableRow, SidebarItem, TabBar,
    TabItem, UiColors,
};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Rows,
    Tabs,
    Segments,
    Sidebar,
}

struct NavigationHarness {
    kind: Kind,
    order: Vec<usize>,
    selected: usize,
    highlighted: usize,
    focuses: [FocusHandle; 3],
    nested_focuses: [FocusHandle; 3],
    changes: Vec<usize>,
    keyboard_activations: Vec<bool>,
    nested: Vec<usize>,
    scroll: ScrollHandle,
}

impl Render for NavigationHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        let activate = |value| {
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.selected = value;
                this.changes.push(value);
                this.keyboard_activations
                    .push(window.last_input_was_keyboard());
                cx.notify();
            }
        };
        let control = match self.kind {
            Kind::Segments => SegmentedControl::new(
                "segments",
                self.selected,
                self.order.iter().map(|&value| {
                    Segment::new(
                        SharedString::from(format!("segment-{value}")),
                        value,
                        format!("Choice {value}"),
                        &self.focuses[value],
                    )
                }),
            )
            .on_change(cx, |this, value, _, cx| {
                this.selected = value;
                this.changes.push(value);
                cx.notify();
            })
            .into_any_element(),
            kind => {
                let mut tabs = TabBar::new("tabs", &self.scroll).w(px(360.0)).h(px(28.0));
                let mut rows = div().flex().flex_col().w(px(240.0));
                for &value in &self.order {
                    let nested =
                        IconButton::new(SharedString::from(format!("nested-{value}")), div())
                            .track_focus(&self.nested_focuses[value])
                            .w(px(24.0))
                            .h(px(24.0))
                            .p(px(0.0))
                            .bg(rgb(0xAB1234 + value as u32))
                            .on_activation(cx, move |this, _, _| this.nested.push(value));
                    let id = SharedString::from(format!("row-{value}"));
                    if kind == Kind::Tabs {
                        tabs = tabs.tab(
                            TabItem::new(id, format!("Tab {value}"), &self.focuses[value])
                                .selected(value == self.selected)
                                .close_button(nested)
                                .on_activation(cx, activate(value)),
                        );
                    } else if kind == Kind::Sidebar {
                        rows = rows.child(
                            SidebarItem::new(id, "gearshape", format!("Page {value}"))
                                .selected(value == self.selected)
                                .track_focus(&self.focuses[value])
                                .on_activation(cx, activate(value)),
                        );
                    } else {
                        rows = rows.child(
                            SelectableRow::new(id, format!("Model {value}"))
                                .h(px(28.0))
                                .selected(value == self.selected)
                                .highlighted(value == self.highlighted)
                                .track_focus(&self.focuses[value])
                                .trailing(nested)
                                .on_activation(cx, activate(value)),
                        );
                    }
                }
                if kind == Kind::Tabs {
                    tabs.into_any_element()
                } else {
                    rows.into_any_element()
                }
            }
        };
        div().p(px(8.0)).child(control)
    }
}

fn harness(kind: Kind, cx: &mut gpui::Context<NavigationHarness>) -> NavigationHarness {
    NavigationHarness {
        kind,
        order: vec![0, 1, 2],
        selected: 0,
        highlighted: 1,
        focuses: std::array::from_fn(|_| cx.focus_handle()),
        nested_focuses: std::array::from_fn(|_| cx.focus_handle()),
        changes: Vec::new(),
        keyboard_activations: Vec::new(),
        nested: Vec::new(),
        scroll: ScrollHandle::new(),
    }
}

#[gpui::test]
fn navigation_keeps_identity_and_keyboard_focus_after_reordering(cx: &mut TestAppContext) {
    for kind in [Kind::Tabs, Kind::Segments] {
        let (view, cx) = cx.add_window_view(|_, cx| harness(kind, cx));
        let focuses = view.read_with(cx, |view, _| view.focuses.clone());
        cx.update(|window, cx| window.focus(&focuses[0], cx));
        cx.simulate_keystrokes("right end right home left cmd-right shift-end enter space");
        view.read_with(cx, |view, _| {
            assert_eq!(view.changes, [1, 2, 2, 0, 0, 0, 0])
        });
        cx.update(|window, _| assert!(focuses[0].is_focused(window)));
        view.update(cx, |view, cx| {
            view.order = vec![2, 0, 1];
            cx.notify();
        });
        cx.simulate_keystrokes("right");
        view.read_with(cx, |view, _| assert_eq!(view.selected, 1));
        cx.update(|window, _| assert!(focuses[1].is_focused(window)));
        view.update(cx, |view, cx| {
            view.order = vec![1, 2];
            cx.notify();
        });
        cx.simulate_keystrokes("right");
        view.read_with(cx, |view, _| assert_eq!(view.selected, 2));
    }
    assert_eq!(
        crate::ui::primitives::navigation::horizontal_navigation_target(0, 0, "right"),
        None
    );
}

#[gpui::test]
fn nested_row_and_tab_actions_do_not_activate_the_parent(cx: &mut TestAppContext) {
    for kind in [Kind::Rows, Kind::Tabs] {
        let (view, cx) = cx.add_window_view(|_, cx| harness(kind, cx));
        let focus = view.read_with(cx, |view, _| view.nested_focuses[0].clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("enter space cmd-enter shift-space");
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
            assert_eq!(view.nested, [0, 0, 0]);
            assert!(view.changes.is_empty());
        });
        view.update(cx, |view, cx| {
            view.order.reverse();
            cx.notify();
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.nested, [0, 0, 0, 0]);
            assert!(view.changes.is_empty());
        });
        if kind == Kind::Tabs {
            cx.simulate_keystrokes("home");
            view.read_with(cx, |view, _| assert_eq!(view.selected, 2));
        }
    }
}

#[gpui::test]
fn row_highlight_is_independent_of_selection_and_sidebar_activates_by_keyboard(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| harness(Kind::Rows, cx));
    let highlight_bounds = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            let color = UiColors::current(cx).overlay_strong;
            crate::ui::squircle::test_support::painted_surfaces(window, cx)
                .into_iter()
                .find(|quad| quad.background.as_solid() == Some(color))
                .unwrap()
                .bounds
        })
    };
    let original = highlight_bounds(cx);
    view.update(cx, |view, cx| {
        view.selected = 2;
        cx.notify();
    });
    assert_eq!(highlight_bounds(cx), original);
    let (view, cx) = cx.cx.add_window_view(|_, cx| harness(Kind::Sidebar, cx));
    let focus = view.read_with(cx, |view, _| view.focuses[1].clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_keystrokes("enter space cmd-enter");
    view.read_with(cx, |view, _| assert_eq!(view.changes, [1, 1]));
}

struct ListHarness {
    state: ListState,
    builds: Rc<Cell<usize>>,
}

impl Render for ListHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::squircle::test_support::clear(window, cx);
        let builds = self.builds.clone();
        div().w(px(300.0)).h(px(286.0)).child(
            list(self.state.clone(), move |index, _, _| {
                builds.set(builds.get() + 1);
                SelectableRow::new(("row", index), "Prepared row").into_any_element()
            })
            .size_full(),
        )
    }
}

#[gpui::test]
fn selectable_rows_stay_bounded_by_the_virtualized_viewport(cx: &mut TestAppContext) {
    for count in [100, 10_000] {
        let builds = Rc::new(Cell::new(0));
        let state = ListState::new(count, ListAlignment::Top, px(0.0));
        state.reset_with_uniform_height(count, px(26.0));
        let (_, cx) = cx.add_window_view(|_, _| ListHarness {
            state: state.clone(),
            builds: builds.clone(),
        });
        assert!(
            builds.get() > 0 && builds.get() <= 22,
            "{count} rows built {} elements",
            builds.get()
        );
        builds.set(0);
        cx.update(|window, _| {
            state.scroll_to_reveal_item(count - 1);
            window.refresh();
        });
        assert!(
            builds.get() > 0 && builds.get() <= 22,
            "revealing a distant row must stay bounded"
        );
    }
}

#[gpui::test]
fn clicking_a_keyboard_focused_tab_reports_pointer_activation(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| harness(Kind::Tabs, cx));
    let focus = view.read_with(cx, |view, _| view.focuses[0].clone());
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_keystrokes("enter");
    cx.simulate_click(point(px(20.0), px(20.0)), Modifiers::none());
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |view, _| {
        assert_eq!(view.keyboard_activations, [true, false, true])
    });
}
