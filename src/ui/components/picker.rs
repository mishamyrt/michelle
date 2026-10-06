use std::rc::Rc;

use gpui::{App, ElementId, RenderOnce, StyleRefinement, Window, prelude::*};

use super::MenuChip;
use super::menu::{ContextMenuHandle, MenuAlign, MenuItem, dropdown_menu_with_enabled};
use crate::ui::ControlSize;

/// Small choices built with the existing menu. The owner supplies selected
/// MenuItems, their handlers, and a stable handle; no selection is stored here.
#[derive(IntoElement)]
pub struct Picker {
    trigger: MenuChip,
    menu_id: ElementId,
    handle: ContextMenuHandle,
    align: MenuAlign,
    disabled: bool,
    items: Rc<dyn Fn(&mut App) -> Vec<MenuItem>>,
}

impl Picker {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<gpui::SharedString>,
        menu_id: impl Into<ElementId>,
        handle: &ContextMenuHandle,
        items: impl Fn(&mut App) -> Vec<MenuItem> + 'static,
    ) -> Self {
        Self {
            trigger: MenuChip::new(id)
                .label(label)
                .outlined()
                .flex_none()
                .justify_between(),
            menu_id: menu_id.into(),
            handle: handle.clone(),
            align: MenuAlign::BelowRight,
            disabled: false,
            items: Rc::new(items),
        }
    }

    pub fn align(mut self, align: MenuAlign) -> Self {
        self.align = align;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.trigger = self.trigger.control_size(size);
        self
    }

    /// When disabling an open picker, the owner closes its handle in the
    /// state-change handler, outside render.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Styled for Picker {
    fn style(&mut self) -> &mut StyleRefinement {
        self.trigger.style()
    }
}

impl RenderOnce for Picker {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        dropdown_menu_with_enabled(
            self.trigger
                .selected(self.handle.is_open())
                .disabled(self.disabled),
            self.menu_id,
            &self.handle,
            self.align,
            !self.disabled,
            self.items,
        )
    }
}
