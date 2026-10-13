use super::control::with_defaults;
use crate::ui::primitives::activation::{ActivationHandler, activation_listener, with_activation};
use crate::ui::primitives::navigation::horizontal_navigation_target;
use crate::ui::squircle::SquircleExt;
use crate::ui::{ActivationExt, StyledTypography, TextStyle, UiColors};
use gpui::{
    AnyElement, App, Context, Div, ElementId, FocusHandle, KeyDownEvent, MouseButton, RenderOnce,
    ScrollHandle, SharedString, Stateful, StyleRefinement, Window, div, prelude::*, px,
};

#[derive(IntoElement)]
pub struct TabItem {
    base: Stateful<Div>,
    label: SharedString,
    focus: FocusHandle,
    selected: bool,
    leading: Option<AnyElement>,
    dirty: Option<AnyElement>,
    close: Option<AnyElement>,
    activate: Option<ActivationHandler>,
}

impl TabItem {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        focus: &FocusHandle,
    ) -> Self {
        Self {
            base: div().id(id).track_focus(focus),
            label: label.into(),
            focus: focus.clone(),
            selected: false,
            leading: None,
            dirty: None,
            close: None,
            activate: None,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn leading(mut self, content: impl IntoElement) -> Self {
        self.leading = Some(content.into_any_element());
        self
    }
    pub fn dirty_indicator(mut self, content: impl IntoElement) -> Self {
        self.dirty = Some(content.into_any_element());
        self
    }
    pub fn close_button(mut self, content: impl IntoElement) -> Self {
        self.close = Some(content.into_any_element());
        self
    }
}

impl Styled for TabItem {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ActivationExt for TabItem {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.activate = Some(activation_listener(cx, handler));
        self
    }
}

impl RenderOnce for TabItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let base = with_defaults(self.base, |base| {
            base.h(px(28.0))
                .min_w(px(100.0))
                .max_w(px(176.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_default()
                .text_style(TextStyle::BODY)
                .text_color(if self.selected {
                    colors.text
                } else {
                    colors.text_secondary
                })
                .when(self.selected, |base| base.bg(colors.overlay_strong))
        })
        .tab_index(0)
        .tab_stop(true)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .when(!self.selected, |base| {
            base.hover(|style| style.bg(colors.overlay))
        })
        .focus_visible(|style| {
            style.shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(0.0), colors.accent)
                    .spread_radius(px(1.0))
                    .inset(),
            ])
        })
        .children(self.leading)
        .child(div().min_w_0().flex_1().truncate().child(self.label))
        .children(self.dirty)
        .children(self.close);
        (match self.activate {
            Some(handler) => with_activation(base, handler),
            None => base,
        })
        .squircle()
    }
}

#[derive(IntoElement)]
pub struct TabBar {
    base: Stateful<Div>,
    tabs: Vec<TabItem>,
    trailing: Vec<AnyElement>,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>, scroll: &ScrollHandle) -> Self {
        Self {
            base: div().id(id).track_scroll(scroll),
            tabs: Vec::new(),
            trailing: Vec::new(),
        }
    }
    pub fn tab(mut self, tab: TabItem) -> Self {
        self.tabs.push(tab);
        self
    }
}

impl Styled for TabBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for TabBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.trailing.extend(elements);
    }
}

impl RenderOnce for TabBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let targets = self
            .tabs
            .iter()
            .map(|tab| (tab.focus.clone(), tab.activate.clone()))
            .collect::<Vec<_>>();
        with_defaults(self.base, |base| {
            base.h_full()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(4.0))
                .overflow_x_scroll()
        })
        .tab_group()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.modifiers.modified() {
                return;
            }
            let Some(index) = targets
                .iter()
                .position(|(focus, _)| focus.contains_focused(window, cx))
            else {
                return;
            };
            let Some(next) =
                horizontal_navigation_target(index, targets.len(), &event.keystroke.key)
            else {
                return;
            };
            window.focus(&targets[next].0, cx);
            if let Some(activate) = &targets[next].1 {
                activate(window, cx);
            }
            cx.stop_propagation();
        })
        .children(self.tabs)
        .children(self.trailing)
    }
}
