use super::*;

/// A zero-cost canvas that records its parent's bounds into the handle.
///
/// `inset_0` rather than `size_full`: an absolutely positioned child sizes
/// against its containing block, so `size_full` inside a padded trigger reports
/// the *content* box and the menu ends up indented by the trigger's padding.
fn trigger_bounds_probe(handle: &ContextMenuHandle) -> impl IntoElement {
    let bounds = handle.trigger_bounds.clone();
    canvas(
        move |probe: Bounds<Pixels>, _, _| bounds.set(Some(probe)),
        |_, _, _, _| (),
    )
    .absolute()
    .inset_0()
}

/// A dropdown menu anchored under its trigger, toggled by a left click.
///
/// Unlike a context menu it aligns to the trigger rather than the pointer, so
/// the handle carries the trigger's last-known bounds. Those are a frame old,
/// which is invisible in practice: a trigger does not move between the click
/// and the menu appearing.
pub fn dropdown_menu<E>(
    trigger: E,
    id: impl Into<ElementId>,
    handle: &ContextMenuHandle,
    align: MenuAlign,
    items: impl Fn(&mut App) -> Vec<MenuItem> + 'static,
) -> AnyElement
where
    E: ParentElement + Styled + InteractiveElement + IntoElement + 'static,
{
    let id: ElementId = id.into();
    let items = Rc::new(items);
    anchored_surface(trigger, handle, align, SurfaceFocus::Card, move |handle| {
        MenuCard {
            id: id.clone(),
            handle: handle.clone(),
            items: items.clone(),
        }
        .into_any_element()
    })
}

/// A dropdown-anchored panel holding arbitrary content.
///
/// Same trigger, anchoring and dismissal as [`dropdown_menu`], but the card
/// draws no chrome — the content owns its own surface — and it does not take
/// focus, so a search field inside can. Escape still works: the card declares
/// the menu key context, and key dispatch walks up to it from the focused
/// descendant.
pub fn popover<E>(
    trigger: E,
    handle: &ContextMenuHandle,
    align: MenuAlign,
    content: impl Fn(&ContextMenuHandle, &mut Window, &mut App) -> AnyElement + 'static,
) -> AnyElement
where
    E: ParentElement + Styled + InteractiveElement + IntoElement + 'static,
{
    let content = Rc::new(content);
    anchored_surface(
        trigger,
        handle,
        align,
        SurfaceFocus::Content,
        move |handle| {
            PopoverCard {
                handle: handle.clone(),
                content: content.clone(),
            }
            .into_any_element()
        },
    )
}

/// Toggle a [`popover`] as if its trigger were clicked, for keyboard shortcuts.
///
/// Anchors to the trigger's last recorded bounds, so it no-ops until the
/// trigger has drawn at least once. The handle's toggle observers may update
/// the owning entity, so a caller holding that entity's lease must defer this.
pub fn toggle_popover(
    handle: &ContextMenuHandle,
    align: MenuAlign,
    window: &mut Window,
    cx: &mut App,
) {
    toggle_menu(handle, align, SurfaceFocus::Content, window, cx);
}

/// Toggle a dropdown from a shortcut, giving its entries keyboard focus.
pub fn toggle_dropdown(
    handle: &ContextMenuHandle,
    align: MenuAlign,
    window: &mut Window,
    cx: &mut App,
) {
    toggle_menu(handle, align, SurfaceFocus::Card, window, cx);
}

fn toggle_menu(
    handle: &ContextMenuHandle,
    align: MenuAlign,
    focus_target: SurfaceFocus,
    window: &mut Window,
    cx: &mut App,
) {
    if handle.is_open() {
        handle.close(window, cx);
        window.refresh();
        return;
    }
    let Some(anchor) = handle
        .trigger_bounds
        .get()
        .map(|bounds| align.anchor_point(bounds, px(TRIGGER_GAP)))
    else {
        return;
    };
    open_menu(handle, anchor, focus_target, true, window, cx);
}

/// The shared half of both dropdown surfaces: a trigger that records its bounds
/// and toggles the handle, plus the open card deferred and anchored to it.
fn anchored_surface<E>(
    trigger: E,
    handle: &ContextMenuHandle,
    align: MenuAlign,
    focus_target: SurfaceFocus,
    card: impl Fn(&ContextMenuHandle) -> AnyElement + 'static,
) -> AnyElement
where
    E: ParentElement + Styled + InteractiveElement + IntoElement + 'static,
{
    let open_at = handle.state.borrow().open;
    let toggle_handle = handle.clone();
    let key_handle = handle.clone();

    let trigger = trigger
        .relative()
        .track_focus(&handle.trigger_focus)
        .tab_index(0)
        .child(trigger_bounds_probe(handle))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            toggle_anchored_surface(&toggle_handle, align, focus_target, window, cx);
            cx.stop_propagation();
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if key_handle.trigger_focus.is_focused(window)
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                toggle_anchored_surface(&key_handle, align, focus_target, window, cx);
                cx.stop_propagation();
            }
        });

    let Some(position) = open_at else {
        return trigger.into_any_element();
    };
    let trigger_bounds = handle
        .trigger_bounds
        .get()
        .unwrap_or_else(|| Bounds::new(position, Size::default()));

    trigger
        .child(
            deferred(FloatingSurface::new(
                card(handle),
                trigger_bounds,
                align,
                px(TRIGGER_GAP),
                px(8.0),
            ))
            .with_priority(1),
        )
        .into_any_element()
}

fn toggle_anchored_surface(
    handle: &ContextMenuHandle,
    align: MenuAlign,
    focus_target: SurfaceFocus,
    window: &mut Window,
    cx: &mut App,
) {
    if handle.is_open() {
        handle.close(window, cx);
        window.refresh();
        return;
    }
    let anchor = handle
        .trigger_bounds
        .get()
        .map(|bounds| align.anchor_point(bounds, px(TRIGGER_GAP)))
        .unwrap_or_else(|| window.mouse_position());
    open_menu(handle, anchor, focus_target, true, window, cx);
}

/// Attach a context menu to `element`.
///
/// `items` is called only when the menu opens, so building the item list — which
/// may capture message content or run availability checks — never costs
/// anything on an ordinary frame.
pub fn context_menu<E>(
    element: E,
    id: impl Into<ElementId>,
    handle: &ContextMenuHandle,
    items: impl Fn(&mut App) -> Vec<MenuItem> + 'static,
) -> AnyElement
where
    E: ParentElement + Styled + InteractiveElement + IntoElement + 'static,
{
    let id: ElementId = id.into();
    let open_at = handle.state.borrow().open;
    let handle_for_down = handle.clone();
    let items = Rc::new(items);
    let items_for_down = items.clone();

    let element = element
        .relative()
        .child(trigger_bounds_probe(handle))
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, window, cx| {
                if handle_for_down
                    .state
                    .borrow()
                    .pending_context_items
                    .is_empty()
                    && items_for_down(cx).is_empty()
                {
                    return;
                }
                open_menu(
                    &handle_for_down,
                    event.position,
                    SurfaceFocus::Card,
                    false,
                    window,
                    cx,
                );
                cx.stop_propagation();
                window.prevent_default();
            },
        );

    let Some(position) = open_at else {
        return element.into_any_element();
    };

    element
        .child(
            deferred(
                anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.0))
                    .child(MenuCard {
                        id,
                        handle: handle.clone(),
                        items,
                    }),
            )
            .with_priority(1),
        )
        .into_any_element()
}
