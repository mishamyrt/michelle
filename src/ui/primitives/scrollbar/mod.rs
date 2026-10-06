//! Overlay scrollbar for a virtualized [`ListState`].
//!
//! Drawn as a single quad from geometry the list already tracks, with the drag
//! and click listeners registered during paint. That keeps it to one element
//! and no layout children, so the scrollbar costs the transcript essentially
//! nothing per frame.
//!
//! It follows AppKit's overlay scrollers: hidden at rest, revealed while the
//! content moves, held briefly, then faded out — and revealed again, wider,
//! whenever the pointer is over its track.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, BorderStyle, Bounds, Hsla, IntoElement, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle, Styled, Window, canvas, fill,
    linear_color_stop, linear_gradient, point, prelude::*, px, quad, size,
};

use crate::ui::UiColors;

mod geometry;
mod render;
mod state;
mod surface;
#[cfg(test)]
mod tests;

pub use render::{FadeEdge, edge_fade, vertical};
pub use state::ScrollbarState;
pub use surface::Scrollable;

#[cfg(test)]
use geometry::THUMB_MIN_HEIGHT;
use geometry::{THUMB_WIDTH, THUMB_WIDTH_ACTIVE, TRACK_WIDTH, geometry, offset_for_thumb_top};
#[cfg(test)]
use state::FADE;
use state::{HOLD, arm_fade_wake, opacity};
use surface::scroll_to;
