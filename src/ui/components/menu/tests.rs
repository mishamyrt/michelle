use std::{cell::Cell, rc::Rc};

use gpui::{Context, Modifiers, Render, TestAppContext, point, size};

use super::placement::resolve_floating_placement;
use super::state::next_highlight;
use super::*;

/// Which anchored surface the harness mounts; both share
/// `anchored_surface` but dismiss through different cards.
#[derive(Clone, Copy)]
enum Surface {
    Popover,
    Dropdown,
    Context,
}

struct Harness {
    handle: ContextMenuHandle,
    surface: Surface,
}

struct FocusedPopoverHarness {
    handle: ContextMenuHandle,
    descendant_focus: FocusHandle,
}

struct SubmenuHarness {
    handle: ContextMenuHandle,
    activated: Rc<Cell<bool>>,
}

#[test]
fn floating_surface_keeps_a_preferred_side_that_fits() {
    let placement = resolve_floating_placement(
        Bounds::new(point(px(600.0), px(200.0)), size(px(100.0), px(20.0))),
        size(px(240.0), px(180.0)),
        Bounds::new(Point::default(), size(px(800.0), px(600.0))),
        MenuAlign::BelowRight,
        px(4.0),
        px(8.0),
    );

    assert_eq!(placement.align, MenuAlign::BelowRight);
    assert_eq!(placement.bounds.origin, point(px(460.0), px(224.0)));
}

#[test]
fn floating_surface_flips_across_the_trigger_when_below_does_not_fit() {
    let trigger = Bounds::new(point(px(600.0), px(500.0)), size(px(100.0), px(20.0)));
    let placement = resolve_floating_placement(
        trigger,
        size(px(240.0), px(180.0)),
        Bounds::new(Point::default(), size(px(800.0), px(600.0))),
        MenuAlign::BelowRight,
        px(4.0),
        px(8.0),
    );

    assert_eq!(placement.align, MenuAlign::AboveRight);
    assert_eq!(placement.bounds.origin, point(px(460.0), px(316.0)));
    assert_eq!(placement.bounds.bottom() + px(4.0), trigger.top());
}

#[test]
fn floating_surface_flips_alignment_before_shifting() {
    let placement = resolve_floating_placement(
        Bounds::new(point(px(10.0), px(200.0)), size(px(40.0), px(20.0))),
        size(px(240.0), px(180.0)),
        Bounds::new(Point::default(), size(px(800.0), px(600.0))),
        MenuAlign::BelowRight,
        px(4.0),
        px(8.0),
    );

    assert_eq!(placement.align, MenuAlign::BelowLeft);
    assert_eq!(placement.bounds.origin, point(px(10.0), px(224.0)));
}

#[test]
fn floating_surface_shifts_oversized_content_to_the_viewport_margin() {
    let placement = resolve_floating_placement(
        Bounds::new(point(px(100.0), px(200.0)), size(px(40.0), px(20.0))),
        size(px(900.0), px(180.0)),
        Bounds::new(Point::default(), size(px(800.0), px(600.0))),
        MenuAlign::BelowLeft,
        px(4.0),
        px(8.0),
    );

    assert_eq!(placement.bounds.origin.x, px(8.0));
}

impl Render for Harness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let trigger = div().w(px(120.0)).h(px(32.0));
        div()
            .size_full()
            .tab_index(0)
            .tab_group()
            .tab_stop(false)
            .child(match self.surface {
                Surface::Popover => {
                    popover(trigger, &self.handle, MenuAlign::BelowLeft, |_, _, _| {
                        div().w(px(200.0)).h(px(100.0)).into_any_element()
                    })
                }
                Surface::Dropdown => dropdown_menu(
                    trigger,
                    "dropdown",
                    &self.handle,
                    MenuAlign::BelowLeft,
                    |_| vec![MenuItem::new("Entry", |_, _| {}).tooltip("Description")],
                ),
                Surface::Context => context_menu(trigger, "context", &self.handle, |_| {
                    vec![MenuItem::new("Entry", |_, _| {})]
                }),
            })
    }
}

impl Render for FocusedPopoverHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let descendant_focus = self.descendant_focus.clone();
        popover(
            div().w(px(120.0)).h(px(32.0)),
            &self.handle,
            MenuAlign::BelowLeft,
            move |_, _, _| {
                div()
                    .track_focus(&descendant_focus)
                    .w(px(200.0))
                    .h(px(100.0))
                    .into_any_element()
            },
        )
    }
}

impl Render for SubmenuHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let activated = self.activated.clone();
        dropdown_menu(
            div().w(px(120.0)).h(px(32.0)),
            "submenu-dropdown",
            &self.handle,
            MenuAlign::BelowLeft,
            move |_| {
                let activated = activated.clone();
                vec![MenuItem::submenu_with_value(
                    "Grouping",
                    "Updated",
                    move |_| {
                        let activated = activated.clone();
                        vec![MenuItem::new("Project", move |_, _| {
                            activated.set(true);
                        })]
                    },
                )]
            },
        )
    }
}

/// The trigger sits at the window origin, 120×32; the card hangs below it,
/// so a point inside the trigger is outside the card and vice versa.
fn assert_trigger_toggles(surface: Surface, cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let harness = Harness {
        handle: handle.clone(),
        surface,
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);
    let on_trigger = point(px(10.0), px(10.0));
    let outside = point(px(500.0), px(400.0));

    cx.simulate_mouse_down(on_trigger, MouseButton::Left, Modifiers::none());
    assert!(handle.is_open(), "first trigger click should open");

    // The card's capture-phase `on_mouse_down_out` sees this click first;
    // without the trigger exemption it closes the menu and the trigger's
    // own handler reopens it.
    cx.simulate_mouse_down(on_trigger, MouseButton::Left, Modifiers::none());
    assert!(!handle.is_open(), "second trigger click should close");

    cx.simulate_mouse_down(on_trigger, MouseButton::Left, Modifiers::none());
    assert!(handle.is_open(), "trigger click after close should reopen");

    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::none());
    assert!(!handle.is_open(), "click outside should dismiss");
}

#[gpui::test]
fn popover_trigger_toggles(cx: &mut TestAppContext) {
    assert_trigger_toggles(Surface::Popover, cx);
}

#[gpui::test]
fn dropdown_trigger_toggles(cx: &mut TestAppContext) {
    assert_trigger_toggles(Surface::Dropdown, cx);
}

#[gpui::test]
fn context_menu_trigger_click_dismisses(cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let harness = Harness {
        handle: handle.clone(),
        surface: Surface::Context,
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);
    let on_trigger = point(px(10.0), px(10.0));

    cx.update(|window, cx| handle.open_context_menu(window, cx));
    assert!(handle.is_open());
    cx.run_until_parked();
    cx.simulate_mouse_down(on_trigger, MouseButton::Left, Modifiers::none());
    assert!(!handle.is_open(), "a context-menu row click should dismiss");
}

fn assert_trigger_opens_from_keyboard(surface: Surface, cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let harness = Harness {
        handle: handle.clone(),
        surface,
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);

    cx.update(|window, cx| window.focus(&handle.trigger_focus, cx));
    cx.simulate_keystrokes("enter");
    assert!(handle.is_open(), "enter on the tab stop should open");
}

#[gpui::test]
fn popover_trigger_is_keyboard_operable(cx: &mut TestAppContext) {
    assert_trigger_opens_from_keyboard(Surface::Popover, cx);
}

#[gpui::test]
fn popover_descendant_space_does_not_toggle_trigger(cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let descendant_focus = cx.update(|cx| cx.focus_handle());
    let harness = FocusedPopoverHarness {
        handle: handle.clone(),
        descendant_focus: descendant_focus.clone(),
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(handle.is_open());
    cx.run_until_parked();
    cx.update(|window, cx| window.focus(&descendant_focus, cx));
    cx.simulate_keystrokes("space");

    assert!(
        handle.is_open(),
        "space from focused popover content must not toggle the trigger"
    );
}

#[gpui::test]
fn dropdown_trigger_is_keyboard_operable(cx: &mut TestAppContext) {
    assert_trigger_opens_from_keyboard(Surface::Dropdown, cx);
}

#[gpui::test]
fn menu_descriptions_follow_keyboard_and_pointer_navigation(cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let harness = Harness {
        handle: handle.clone(),
        surface: Surface::Dropdown,
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.focus(&handle.focus, cx));
    cx.simulate_keystrokes("down");
    assert_eq!(handle.state.borrow().highlighted, Some(0));
    assert!(handle.state.borrow().keyboard_navigation);

    cx.simulate_mouse_move(point(px(20.0), px(50.0)), None, Modifiers::none());
    assert_eq!(handle.state.borrow().highlighted, Some(0));
    assert!(!handle.state.borrow().keyboard_navigation);

    cx.simulate_keystrokes("end");
    assert!(handle.state.borrow().keyboard_navigation);
    cx.simulate_keystrokes("escape");
    assert!(!handle.is_open());
    assert!(!handle.state.borrow().keyboard_navigation);
}

#[gpui::test]
fn dropdown_shortcut_takes_focus_and_toggles(cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let harness = Harness {
        handle: handle.clone(),
        surface: Surface::Dropdown,
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);

    cx.update(|window, cx| toggle_dropdown(&handle, MenuAlign::AboveLeft, window, cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(handle.is_open());
    cx.update(|window, _| assert!(handle.focus.is_focused(window)));

    cx.simulate_keystrokes("down");
    assert_eq!(handle.state.borrow().highlighted, Some(0));
    cx.update(|window, cx| toggle_dropdown(&handle, MenuAlign::AboveLeft, window, cx));
    assert!(!handle.is_open());
}

#[gpui::test]
fn submenu_is_keyboard_operable(cx: &mut TestAppContext) {
    let handle = cx.update(ContextMenuHandle::new);
    let activated = Rc::new(Cell::new(false));
    let harness = SubmenuHarness {
        handle: handle.clone(),
        activated: activated.clone(),
    };
    let (_view, cx) = cx.add_window_view(|_, _| harness);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.focus(&handle.focus, cx));
    cx.simulate_keystrokes("down");
    cx.simulate_keystrokes("right");
    assert!(handle.state.borrow().submenu_focused);
    cx.simulate_keystrokes("enter");

    assert!(activated.get());
    assert!(!handle.is_open());
}

fn items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Copy", |_, _| {}),
        MenuItem::Separator,
        MenuItem::Separator,
        MenuItem::new("Revert", |_, _| {}),
    ]
}

#[test]
fn separators_are_not_focusable() {
    assert_eq!(*focusable_indexes(&items()), vec![0, 3]);
}

#[test]
fn submenus_are_focusable() {
    let items = vec![MenuItem::submenu_with_value("Grouping", "Updated", |_| {
        Vec::new()
    })];
    assert_eq!(*focusable_indexes(&items), vec![0]);
}

#[test]
fn keyboard_navigation_wraps_at_both_ends() {
    let focusable = focusable_indexes(&items());
    // Two focusable entries at indexes 0 and 3: down from the last wraps to
    // the first, and up from the first wraps to the last.
    assert_eq!(next_highlight(&focusable, None, "down"), Some(0));
    assert_eq!(next_highlight(&focusable, Some(0), "down"), Some(3));
    assert_eq!(next_highlight(&focusable, Some(3), "down"), Some(0));
    assert_eq!(next_highlight(&focusable, None, "up"), Some(3));
    assert_eq!(next_highlight(&focusable, Some(0), "up"), Some(3));
    assert_eq!(next_highlight(&focusable, Some(0), "home"), Some(0));
    assert_eq!(next_highlight(&focusable, Some(0), "end"), Some(3));
    assert_eq!(next_highlight(&focusable, Some(0), "tab"), None);
    assert_eq!(next_highlight(&[], None, "down"), None);
}
