use super::*;

/// The two things a scrollable surface has to expose. GPUI stores both kinds of
/// offset as a non-positive y; implementations report a downward distance so the
/// geometry above reads the obvious way.
pub trait Scrollable {
    /// Height of the visible area.
    fn viewport_height(&self) -> Pixels;
    /// Content height beyond the viewport.
    fn max_offset(&self) -> Pixels;
    /// How far the content is currently scrolled down.
    fn scrolled(&self) -> Pixels;
    fn scroll_to(&self, offset: Pixels);
}

impl Scrollable for ListState {
    fn viewport_height(&self) -> Pixels {
        self.viewport_bounds().size.height
    }

    fn max_offset(&self) -> Pixels {
        self.max_offset_for_scrollbar().y
    }

    fn scrolled(&self) -> Pixels {
        -self.scroll_px_offset_for_scrollbar().y
    }

    fn scroll_to(&self, offset: Pixels) {
        self.set_offset_from_scrollbar(point(Pixels::ZERO, -offset));
    }
}

impl Scrollable for ScrollHandle {
    fn viewport_height(&self) -> Pixels {
        self.bounds().size.height
    }

    fn max_offset(&self) -> Pixels {
        self.max_offset().y
    }

    fn scrolled(&self) -> Pixels {
        -self.offset().y
    }

    fn scroll_to(&self, offset: Pixels) {
        let x = self.offset().x;
        self.set_offset(Point::new(x, -offset));
    }
}

pub(super) fn scroll_to(surface: &impl Scrollable, offset: Pixels, max_offset: Pixels) {
    surface.scroll_to(offset.clamp(Pixels::ZERO, max_offset));
}
