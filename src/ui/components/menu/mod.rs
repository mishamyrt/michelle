//! Context menus and dropdown menus.
//!
//! Both share one card and one dismissal model; they differ only in where they
//! anchor — a context menu at the pointer, a dropdown under its trigger.
//!
//! A menu is built lazily: the item list is only constructed once the menu
//! actually opens, and while closed the wrapper contributes one `Rc<Cell>` read
//! and no children. The open menu renders through `deferred(anchored(..))` so it
//! escapes its row's clipping and paints above every sibling.
//!
//! Dismissal follows Zed's own context menus:
//!
//! - **Click outside** uses `on_mouse_down_out`, which tests the card's own
//!   hitbox during the capture phase. An occluding full-window backdrop would
//!   also work but has to guess the window size and swallows hover elsewhere.
//!   A left click on the trigger is exempt — capture runs before the trigger's
//!   bubble-phase toggle, so closing here would make the toggle see a closed
//!   menu and reopen it.
//! - **Escape** is an action bound in the menu's own key context, so it beats
//!   the transcript's `escape` binding instead of also cancelling the turn.
//! - **Focus** is taken two frames after opening. Deferred elements are not
//!   linked into the dispatch tree until after the deferred draw runs, so
//!   focusing any earlier silently does nothing — and then no key reaches the
//!   menu at all.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Display, Element, ElementId, FocusHandle, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, Position, RenderOnce, SharedString, Size,
    StatefulInteractiveElement, Style, Styled, Window, actions, anchored, canvas, deferred, div,
    img, prelude::FluentBuilder, px,
};

actions!(
    michelle_menu,
    [
        DismissMenu,
        SelectNextEntry,
        SelectPreviousEntry,
        SelectNextTab,
        SelectPreviousTab,
        ConfirmEntry
    ]
);

/// Key context the open menu declares, and the scope its bindings live in.
const MENU_CONTEXT: &str = "MichelleMenu";

/// Vertical gap between a trigger and its anchored card.
const TRIGGER_GAP: f32 = 4.0;

/// A text field inside an open panel, such as a picker's filter box.
///
/// The field holds real focus the whole time — the list's selection is drawn,
/// never focused, which is how Zed's picker works. So the list's keys have to
/// be claimed from under the focused field, and only a binding can do that:
/// `enter`, `tab`, and the arrows reach the field as *actions*, and an action
/// consumes the keystroke before any `on_key_down` listener above it ever runs.
const PANEL_FIELD_CONTEXT: &str = "MichelleMenu > TextInput";

use crate::ui::{TextStyle, UiColors};

mod item;
mod placement;
mod render;
mod state;
mod surface;
#[cfg(test)]
mod tests;

pub use item::MenuItem;
pub use placement::MenuAlign;
pub use state::{ContextMenuHandle, init};
pub use surface::{context_menu, dropdown_menu, popover, toggle_dropdown, toggle_popover};

use placement::FloatingSurface;
use render::{MenuCard, PopoverCard};
use state::{SurfaceFocus, focusable_indexes, on_menu_key, open_menu, open_submenu};
