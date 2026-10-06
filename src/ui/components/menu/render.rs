use gpui::{BoxShadow, black, point};

use super::*;
use crate::ui::{
    IconSize, SfSymbol, StyledTypography, SymbolWeight, Tooltip,
    primitives::timing::TOOLTIP_SHOW_DELAY_MS,
    squircle::{SquircleStyled, squircle},
};

/// A chrome-less card: dismissal and the menu key context, nothing else.
#[derive(IntoElement)]
pub(super) struct PopoverCard {
    pub(super) handle: ContextMenuHandle,
    #[allow(clippy::type_complexity)]
    pub(super) content: Rc<dyn Fn(&ContextMenuHandle, &mut Window, &mut App) -> AnyElement>,
}

impl RenderOnce for PopoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = (self.content)(&self.handle, window, cx);
        div()
            .occlude()
            .key_context(MENU_CONTEXT)
            .on_action({
                let handle = self.handle.clone();
                move |_: &DismissMenu, window, cx| {
                    handle.close(window, cx);
                    window.refresh();
                }
            })
            .on_mouse_down_out({
                let handle = self.handle.clone();
                move |event, window, cx| handle.dismiss_on_down_out(event, window, cx)
            })
            .child(body)
    }
}

#[derive(IntoElement)]
pub(super) struct MenuCard {
    pub(super) id: ElementId,
    pub(super) handle: ContextMenuHandle,
    #[allow(clippy::type_complexity)]
    pub(super) items: Rc<dyn Fn(&mut App) -> Vec<MenuItem>>,
}

impl RenderOnce for MenuCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = UiColors::current(cx);
        let item_builder: Rc<dyn Fn(&mut App) -> Vec<MenuItem>> = {
            let handle = self.handle.clone();
            let base_items = self.items.clone();
            Rc::new(move |cx| {
                let mut items = handle.state.borrow().context_items.clone();
                items.extend(base_items(cx));
                items
            })
        };
        let items = item_builder(cx);
        let focusable = focusable_indexes(&items);
        let columns = MenuColumns::for_items(&items);
        let (highlighted, active_submenu, submenu_highlighted) = {
            let state = self.handle.state.borrow();
            (
                state.highlighted,
                state.active_submenu,
                state.submenu_highlighted,
            )
        };
        let submenu = active_submenu.and_then(|index| {
            let MenuItem::Submenu { items, .. } = items.get(index)? else {
                return None;
            };
            Some((index, items(cx)))
        });

        let mut root_card = div()
            .id(self.id)
            .relative()
            .min_w(px(176.0))
            .max_w(px(320.0))
            .py(px(5.5))
            .rounded(px(12.0))
            .child(
                squircle()
                    .rounded(px(12.0))
                    .bg(theme.raised)
                    .border(px(0.5))
                    .border_color(theme.border_strong)
                    .border_inside()
                    .absolute_expand(),
            )
            .shadow_lg()
            .flex()
            .flex_col();

        for (index, item) in items.into_iter().enumerate() {
            root_card = root_card.child(render_menu_item(
                item,
                index,
                highlighted == Some(index) || active_submenu == Some(index),
                false,
                columns,
                &theme,
                self.handle.clone(),
                window,
                cx,
            ));
        }

        let mut surface = div()
            .occlude()
            .track_focus(&self.handle.focus)
            .key_context(MENU_CONTEXT)
            .flex()
            .items_start()
            .on_action({
                let handle = self.handle.clone();
                move |_: &DismissMenu, window, cx| {
                    handle.close(window, cx);
                    window.refresh();
                }
            })
            .on_mouse_down_out({
                let handle = self.handle.clone();
                move |event, window, cx| handle.dismiss_on_down_out(event, window, cx)
            })
            .on_key_down({
                let handle = self.handle.clone();
                let focusable = focusable.clone();
                let items = item_builder.clone();
                move |event: &KeyDownEvent, window, cx| {
                    on_menu_key(&handle, &focusable, &items, event, window, cx);
                }
            })
            .child(root_card);

        if let Some((parent_index, submenu_items)) = submenu {
            let columns = MenuColumns::for_items(&submenu_items);
            let mut submenu_card = div()
                .id(SharedString::from(format!("submenu-{parent_index}")))
                .relative()
                .ml(px(-4.0))
                .min_w(px(176.0))
                .max_w(px(320.0))
                .py(px(5.0))
                .rounded(px(12.0))
                .child(
                    squircle()
                        .rounded(px(12.0))
                        .bg(theme.raised)
                        .border(px(0.5))
                        .border_color(theme.border_strong)
                        .border_inside()
                        .absolute_expand(),
                )
                .shadow(vec![BoxShadow {
                    color: black().alpha(0.3),
                    offset: point(px(0.0), px(4.0)),
                    blur_radius: px(24.0),
                    spread_radius: px(0.0),
                    inset: false,
                }])
                .flex()
                .flex_col();
            for (index, item) in submenu_items.into_iter().enumerate() {
                submenu_card = submenu_card.child(render_menu_item(
                    item,
                    index,
                    submenu_highlighted == Some(index),
                    true,
                    columns,
                    &theme,
                    self.handle.clone(),
                    window,
                    cx,
                ));
            }
            surface = surface.child(submenu_card);
        }
        surface
    }
}

#[derive(Clone, Copy, Default)]
struct MenuColumns {
    checks: bool,
    icons: bool,
}

impl MenuColumns {
    fn for_items(items: &[MenuItem]) -> Self {
        let mut columns = Self::default();
        for item in items {
            match item {
                MenuItem::Entry {
                    icon,
                    image,
                    selected,
                    ..
                } => {
                    columns.checks |= *selected;
                    columns.icons |= icon.is_some() || image.is_some();
                }
                MenuItem::Custom {
                    selected,
                    on_click: Some(_),
                    ..
                } => {
                    columns.checks |= *selected;
                }
                _ => {}
            }
        }
        columns
    }
}

#[allow(clippy::too_many_arguments)]
fn render_menu_item(
    item: MenuItem,
    index: usize,
    highlighted: bool,
    in_submenu: bool,
    columns: MenuColumns,
    theme: &UiColors,
    handle: ContextMenuHandle,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match item {
        MenuItem::Separator => div()
            .my(px(4.0))
            .mx(px(16.5))
            .h(px(1.0))
            .bg(theme.border)
            .into_any_element(),
        MenuItem::Header(label) => div()
            .px(px(16.5))
            .pt(px(6.0))
            .pb(px(2.0))
            .text_style(TextStyle::CaptionEmphasized)
            .text_color(theme.text_tertiary)
            .child(label)
            .into_any_element(),
        MenuItem::Entry {
            label,
            tooltip,
            icon: item_icon,
            image,
            selected,
            disabled,
            on_click,
        } => {
            let entry = row(
                index,
                highlighted,
                theme,
                handle.clone(),
                (!disabled).then_some(on_click),
            )
            .when(disabled, |element| element.text_color(theme.text_ghost))
            .when(columns.checks, |element| {
                element.child(check_slot(selected))
            })
            .when(columns.icons, |element| {
                element.child(
                    icon_slot()
                        .when_some(item_icon, |slot, path| {
                            slot.child(SfSymbol::new(path).size(IconSize::Tiny))
                        })
                        .when_some(image, |slot, image| {
                            slot.child(img(image).size(px(16.0)).flex_none())
                        }),
                )
            })
            .child(div().flex_1().min_w_0().truncate().child(label))
            .when_some(tooltip, |entry, tooltip| {
                let keyboard_hint = highlighted
                    && handle.state.borrow().keyboard_navigation
                    && handle.focus.is_focused(window);
                entry
                    .when(!keyboard_hint, |entry| {
                        entry
                            .tooltip(Tooltip::text(tooltip.clone()))
                            .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
                    })
                    .when(keyboard_hint, |entry| {
                        // GPUI's hover tooltip does not follow the menu's keyboard cursor.
                        entry.child(
                            div().absolute().left_full().top_0().child(
                                deferred(
                                    anchored()
                                        .offset(point(px(8.0), px(0.0)))
                                        .snap_to_window_with_margin(px(8.0))
                                        .child(Tooltip::new(tooltip).build(window, cx)),
                                )
                                .with_priority(2),
                            ),
                        )
                    })
            });
            track_pointer_highlight(entry, index, in_submenu, disabled, handle).into_any_element()
        }
        MenuItem::Submenu {
            label,
            value,
            items: _,
        } => {
            // Michelle currently exposes one flyout level. Keeping a nested
            // submenu row inert prevents a child builder from accidentally
            // stealing the parent flyout's keyboard state.
            if in_submenu {
                return row(index, highlighted, theme, handle, None)
                    .text_color(theme.text_ghost)
                    .when(columns.checks, |element| element.child(check_slot(false)))
                    .when(columns.icons, |element| element.child(icon_slot()))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .child(SfSymbol::new("chevron.right").size(IconSize::Tiny))
                    .into_any_element();
            }
            let hover_handle = handle.clone();
            let click_handle = handle.clone();
            row(index, highlighted, theme, handle, None)
                .cursor_default()
                .on_hover(move |hovered, window, _| {
                    if *hovered {
                        open_submenu(&hover_handle, index, false);
                        window.refresh();
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    open_submenu(&click_handle, index, false);
                    window.refresh();
                    cx.stop_propagation();
                })
                .when(columns.checks, |element| element.child(check_slot(false)))
                .when(columns.icons, |element| element.child(icon_slot()))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when_some(value, |element, value| {
                    element.child(div().flex_none().child(value))
                })
                .child(SfSymbol::new("chevron.right").size(IconSize::Tiny))
                .into_any_element()
        }
        MenuItem::Custom {
            render,
            selected,
            on_click,
        } => {
            let body = render(window, cx);
            match on_click {
                Some(on_click) => {
                    let entry = row(index, highlighted, theme, handle.clone(), Some(on_click))
                        .when(columns.checks, |element| {
                            element.child(check_slot(selected))
                        })
                        .when(columns.icons, |element| element.child(icon_slot()))
                        .child(body);
                    track_pointer_highlight(entry, index, in_submenu, false, handle)
                        .into_any_element()
                }
                // Non-interactive rows still need the row's insets so they
                // line up with the entries around them.
                None => div().mx(px(4.0)).px(px(8.0)).child(body).into_any_element(),
            }
        }
    }
}

fn icon_slot() -> gpui::Div {
    div()
        .w(px(16.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

fn check_slot(selected: bool) -> gpui::Div {
    div()
        .w(px(10.0))
        .flex_none()
        .flex()
        .ml(px(-3.0))
        .mr(px(3.0))
        .items_center()
        .justify_center()
        .when(selected, |slot| {
            slot.child(
                SfSymbol::new("checkmark")
                    .size(IconSize::Tiny)
                    .weight(SymbolWeight::Bold)
                    .w(px(10.0))
                    .h(px(10.0)),
            )
        })
}

fn track_pointer_highlight(
    row: gpui::Stateful<gpui::Div>,
    index: usize,
    in_submenu: bool,
    disabled: bool,
    handle: ContextMenuHandle,
) -> gpui::Stateful<gpui::Div> {
    row.when(!disabled, |row| {
        row.on_hover(move |hovered, window, _| {
            if !*hovered {
                return;
            }
            let mut state = handle.state.borrow_mut();
            state.keyboard_navigation = false;
            if in_submenu {
                state.submenu_highlighted = Some(index);
            } else {
                state.highlighted = Some(index);
                state.active_submenu = None;
                state.submenu_highlighted = None;
                state.submenu_focused = false;
            }
            window.refresh();
        })
    })
}

/// The shared row: consistent insets, plus hover, keyboard highlight and
/// close-then-act when it has a handler. A `None` handler renders the same
/// geometry inert, which is how a disabled entry keeps the menu's shape.
fn row(
    index: usize,
    highlighted: bool,
    theme: &UiColors,
    handle: ContextMenuHandle,
    on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
) -> gpui::Stateful<gpui::Div> {
    let accent = theme.accent;
    div()
        .id(index)
        .relative()
        .mx(px(5.5))
        .px(px(11.0))
        .min_h(px(24.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_style(TextStyle::Body)
        .text_color(theme.text)
        .when(highlighted, |element| {
            element
                .text_color(gpui::white())
                .child(squircle().rounded(px(8.0)).bg(accent).absolute_expand())
        })
        .when_some(on_click, |element, on_click| {
            element
                .cursor_default()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    handle.close(window, cx);
                    on_click(window, cx);
                    window.refresh();
                })
        })
}
