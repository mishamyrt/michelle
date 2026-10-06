use super::*;

/// Where an open menu is anchored, in window coordinates.
#[derive(Default)]
pub(super) struct MenuState {
    pub(super) open: Option<Point<Pixels>>,
    /// A nested hit target can contribute actions to its ancestor's menu.
    /// Snapshot them on opening so streaming/reflow cannot retarget an action.
    pub(super) pending_context_items: Vec<MenuItem>,
    pub(super) context_items: Vec<MenuItem>,
    /// Keyboard cursor over focusable entries.
    pub(super) highlighted: Option<usize>,
    pub(super) keyboard_navigation: bool,
    /// Parent item whose flyout is visible.
    pub(super) active_submenu: Option<usize>,
    /// Keyboard cursor inside the visible flyout.
    pub(super) submenu_highlighted: Option<usize>,
    /// Whether arrow-key navigation currently belongs to the flyout.
    pub(super) submenu_focused: bool,
    /// A dropdown/popover trigger toggles its own surface on left click. The
    /// outside-click capture must leave that click alone so the later trigger
    /// handler can close it; a context-menu row has no such handler.
    pub(super) trigger_click_toggles: bool,
}

/// Cross-frame state for one context menu. The owner keeps one per menu site.
#[derive(Clone)]
pub struct ContextMenuHandle {
    pub(super) state: Rc<RefCell<MenuState>>,
    /// Stable focus identity shared by dropdown and keyboard context triggers.
    pub(super) trigger_focus: FocusHandle,
    pub(super) focus: FocusHandle,
    /// The trigger's bounds as of the last frame, so a dropdown can align under
    /// it. Recorded by a zero-cost canvas inside the trigger.
    pub(super) trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Notified with the new open state whenever the menu toggles, in order.
    /// The composer's caret preservation is one of these; a site can add its
    /// own on top.
    #[allow(clippy::type_complexity)]
    pub(super) on_toggle: Rc<Vec<Rc<dyn Fn(bool, &mut Window, &mut App)>>>,
}

impl ContextMenuHandle {
    pub fn new(cx: &mut App) -> Self {
        Self {
            state: Rc::new(RefCell::new(MenuState::default())),
            trigger_focus: cx.focus_handle(),
            focus: cx.focus_handle(),
            trigger_bounds: Rc::new(Cell::new(None)),
            on_toggle: Rc::new(Vec::new()),
        }
    }

    /// Observe open/close transitions. Called only on an actual change, in the
    /// order the observers were added.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        let mut handlers = (*self.on_toggle).clone();
        handlers.push(Rc::new(handler));
        self.on_toggle = Rc::new(handlers);
        self
    }

    fn notify_toggle(&self, open: bool, window: &mut Window, cx: &mut App) {
        for handler in self.on_toggle.iter() {
            handler(open, window, cx);
        }
    }

    pub fn is_open(&self) -> bool {
        self.state.borrow().open.is_some()
    }

    pub fn set_context_items(&self, items: Vec<MenuItem>) {
        self.state.borrow_mut().pending_context_items = items;
    }

    /// The card's focus handle, for content-focusing surfaces whose panel has
    /// no input of its own: focusing the card puts the menu key context on
    /// the dispatch path, which is what lets `escape` dismiss it.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// Stable focus identity for a keyboard-operable menu trigger.
    pub fn trigger_focus_handle(&self) -> &FocusHandle {
        &self.trigger_focus
    }

    /// Opens a context menu from its trigger instead of a pointer event. The
    /// card begins just under the row, avoiding an overlap with its focus ring.
    pub fn open_context_menu(&self, window: &mut Window, cx: &mut App) {
        let position = self
            .trigger_bounds
            .get()
            .map(|bounds| Point::new(bounds.left() + px(8.0), bounds.bottom()))
            .unwrap_or_else(|| window.mouse_position());
        open_menu(self, position, SurfaceFocus::Card, false, window, cx);
    }

    pub fn close(&self, window: &mut Window, cx: &mut App) {
        let was_open = {
            let mut state = self.state.borrow_mut();
            let was_open = state.open.is_some();
            state.open = None;
            state.pending_context_items.clear();
            state.context_items.clear();
            state.highlighted = None;
            state.keyboard_navigation = false;
            state.active_submenu = None;
            state.submenu_highlighted = None;
            state.submenu_focused = false;
            state.trigger_click_toggles = false;
            was_open
        };
        if was_open {
            self.notify_toggle(false, window, cx);
        }
    }

    /// Dismiss for a mouse down outside the card, except a left click on a
    /// dropdown/popover trigger: its own bubble-phase handler is the toggle,
    /// and it runs after this capture-phase listener. Closing here first would
    /// make that handler see a closed menu and reopen it.
    pub(super) fn dismiss_on_down_out(
        &self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        let on_toggling_trigger = self.state.borrow().trigger_click_toggles
            && event.button == MouseButton::Left
            && self
                .trigger_bounds
                .get()
                .is_some_and(|bounds| bounds.contains(&event.position));
        if on_toggling_trigger {
            return;
        }
        self.close(window, cx);
        window.refresh();
    }

    fn open_at(
        &self,
        position: Point<Pixels>,
        trigger_click_toggles: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let was_open = {
            let mut state = self.state.borrow_mut();
            let was_open = state.open.is_some();
            state.open = Some(position);
            state.context_items = std::mem::take(&mut state.pending_context_items);
            state.highlighted = None;
            state.keyboard_navigation = false;
            state.active_submenu = None;
            state.submenu_highlighted = None;
            state.submenu_focused = false;
            state.trigger_click_toggles = trigger_click_toggles;
            was_open
        };
        if !was_open {
            self.notify_toggle(true, window, cx);
        }
    }
}

/// Whether the opened surface takes focus itself.
///
/// A [`MenuCard`] tracks the handle's focus and needs it to see arrow keys. A
/// [`PopoverCard`] does not track it, so focusing the handle would detach focus
/// from the window's dispatch tree — and blur whatever the panel's content
/// focused for itself, such as a search field.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SurfaceFocus {
    Card,
    Content,
}

/// Open at `position`, handing focus to the card when it owns focus.
///
/// The card is deferred, so its focus handle joins the dispatch tree only after
/// the deferred draw. Focusing before then is a silent no-op that leaves the
/// menu unable to see a keystroke — hence the two-frame wait, matching Zed.
pub(super) fn open_menu(
    handle: &ContextMenuHandle,
    position: Point<Pixels>,
    focus_target: SurfaceFocus,
    trigger_click_toggles: bool,
    window: &mut Window,
    cx: &mut App,
) {
    // Runs the toggle observers, which is where a content-focusing surface
    // schedules its own focus. Ours is scheduled after, so it would win — only
    // request it when the card is what should end up focused.
    handle.open_at(position, trigger_click_toggles, window, cx);
    if focus_target == SurfaceFocus::Card {
        let focus = handle.focus.clone();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        });
    }
    window.refresh();
}

pub(super) fn open_submenu(handle: &ContextMenuHandle, index: usize, keyboard: bool) {
    let mut state = handle.state.borrow_mut();
    if state.active_submenu != Some(index) {
        state.submenu_highlighted = None;
    }
    state.highlighted = Some(index);
    state.keyboard_navigation = keyboard;
    state.active_submenu = Some(index);
    state.submenu_focused = keyboard;
}

pub(super) fn focusable_indexes(items: &[MenuItem]) -> Rc<Vec<usize>> {
    Rc::new(
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_focusable())
            .map(|(index, _)| index)
            .collect(),
    )
}

/// The next highlighted item index for a navigation key, wrapping at both ends.
/// `current` and the result are indexes into the *item list*, not into
/// `focusable`. `None` means the key does not navigate.
pub(super) fn next_highlight(
    focusable: &[usize],
    current: Option<usize>,
    key: &str,
) -> Option<usize> {
    if focusable.is_empty() {
        return None;
    }
    let position =
        current.and_then(|item| focusable.iter().position(|candidate| *candidate == item));
    let next = match key {
        "down" => position.map_or(0, |index| (index + 1) % focusable.len()),
        "up" => position.map_or(focusable.len() - 1, |index| {
            (index + focusable.len() - 1) % focusable.len()
        }),
        "home" => 0,
        "end" => focusable.len() - 1,
        _ => return None,
    };
    Some(focusable[next])
}

pub(super) fn on_menu_key(
    handle: &ContextMenuHandle,
    focusable: &[usize],
    items: &Rc<dyn Fn(&mut App) -> Vec<MenuItem>>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    let key = event.keystroke.key.as_str();
    if key == "escape" {
        handle.close(window, cx);
        window.refresh();
        cx.stop_propagation();
        return;
    }

    let (submenu_focused, active_submenu, submenu_current) = {
        let state = handle.state.borrow();
        (
            state.submenu_focused,
            state.active_submenu,
            state.submenu_highlighted,
        )
    };
    if submenu_focused {
        let submenu_items = active_submenu
            .and_then(|index| items(cx).into_iter().nth(index))
            .and_then(|item| match item {
                MenuItem::Submenu { items, .. } => Some(items(cx)),
                _ => None,
            });
        let Some(submenu_items) = submenu_items else {
            let mut state = handle.state.borrow_mut();
            state.active_submenu = None;
            state.submenu_highlighted = None;
            state.submenu_focused = false;
            window.refresh();
            return;
        };

        if key == "left" {
            let mut state = handle.state.borrow_mut();
            state.keyboard_navigation = true;
            state.submenu_focused = false;
            state.submenu_highlighted = None;
            window.refresh();
            cx.stop_propagation();
            return;
        }

        let submenu_focusable = focusable_indexes(&submenu_items);
        if let Some(next) = next_highlight(&submenu_focusable, submenu_current, key) {
            let mut state = handle.state.borrow_mut();
            state.submenu_highlighted = Some(next);
            state.keyboard_navigation = true;
            window.refresh();
            cx.stop_propagation();
            return;
        }

        if matches!(key, "enter" | "space") {
            cx.stop_propagation();
            let Some(highlighted) = handle.state.borrow().submenu_highlighted else {
                return;
            };
            let activated = submenu_items
                .into_iter()
                .nth(highlighted)
                .and_then(MenuItem::click_handler);
            if let Some(on_click) = activated {
                handle.close(window, cx);
                on_click(window, cx);
                window.refresh();
            }
        }
        return;
    }

    if key == "left" && active_submenu.is_some() {
        let mut state = handle.state.borrow_mut();
        state.keyboard_navigation = true;
        state.active_submenu = None;
        state.submenu_highlighted = None;
        state.submenu_focused = false;
        window.refresh();
        cx.stop_propagation();
        return;
    }
    if focusable.is_empty() {
        return;
    }

    let current = handle.state.borrow().highlighted;
    if let Some(next) = next_highlight(focusable, current, key) {
        let mut state = handle.state.borrow_mut();
        state.highlighted = Some(next);
        state.keyboard_navigation = true;
        state.active_submenu = None;
        state.submenu_highlighted = None;
        state.submenu_focused = false;
        window.refresh();
        cx.stop_propagation();
        return;
    }

    if matches!(key, "right" | "enter" | "space") {
        cx.stop_propagation();
        let Some(highlighted) = handle.state.borrow().highlighted else {
            return;
        };
        // Rebuild to reach the entry's closure: the item list is intentionally
        // not retained between frames.
        let Some(item) = items(cx).into_iter().nth(highlighted) else {
            return;
        };
        match item {
            MenuItem::Submenu { items, .. } => {
                let submenu_items = items(cx);
                let first = focusable_indexes(&submenu_items).first().copied();
                let mut state = handle.state.borrow_mut();
                state.keyboard_navigation = true;
                state.active_submenu = Some(highlighted);
                state.submenu_highlighted = first;
                state.submenu_focused = true;
                window.refresh();
            }
            item if matches!(key, "enter" | "space") => {
                if let Some(on_click) = item.click_handler() {
                    handle.close(window, cx);
                    on_click(window, cx);
                    window.refresh();
                }
            }
            _ => {}
        }
    }
}

/// Bind the menu's own keys. Called once at startup.
///
/// Must run after [`crate::input::init`]: these share a context depth with the
/// field's own bindings, and the tie goes to whichever was registered last.
/// That is what lets `enter` here beat the field's submit.
pub fn init(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("escape", DismissMenu, Some(MENU_CONTEXT)),
        KeyBinding::new("down", SelectNextEntry, Some(PANEL_FIELD_CONTEXT)),
        KeyBinding::new("up", SelectPreviousEntry, Some(PANEL_FIELD_CONTEXT)),
        KeyBinding::new("tab", SelectNextTab, Some(PANEL_FIELD_CONTEXT)),
        KeyBinding::new("shift-tab", SelectPreviousTab, Some(PANEL_FIELD_CONTEXT)),
        KeyBinding::new("enter", ConfirmEntry, Some(PANEL_FIELD_CONTEXT)),
    ]);
}
