use super::*;

/// Where a dropdown's card sits relative to its trigger.
///
/// Side matters as much as alignment here: the composer's controls live at the
/// bottom of the window, so their menus have to grow upward or they open off
/// screen and get snapped back over the trigger.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuAlign {
    /// Below the trigger, left edges aligned.
    #[default]
    BelowLeft,
    /// Below the trigger, right edges aligned.
    BelowRight,
    /// Above the trigger, left edges aligned.
    AboveLeft,
    /// Above the trigger, right edges aligned.
    AboveRight,
}

impl MenuAlign {
    fn above(self) -> bool {
        matches!(self, Self::AboveLeft | Self::AboveRight)
    }

    fn right_aligned(self) -> bool {
        matches!(self, Self::BelowRight | Self::AboveRight)
    }

    fn from_sides(above: bool, right_aligned: bool) -> Self {
        match (above, right_aligned) {
            (false, false) => Self::BelowLeft,
            (false, true) => Self::BelowRight,
            (true, false) => Self::AboveLeft,
            (true, true) => Self::AboveRight,
        }
    }

    /// The point on the trigger the card's corner attaches to.
    pub(super) fn anchor_point(self, bounds: gpui::Bounds<Pixels>, gap: Pixels) -> Point<Pixels> {
        let x = if self.right_aligned() {
            bounds.right()
        } else {
            bounds.left()
        };
        let y = if self.above() {
            bounds.top() - gap
        } else {
            bounds.bottom() + gap
        };
        Point::new(x, y)
    }
}

/// Final placement for an anchored surface after `flip` and `shift`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FloatingPlacement {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) align: MenuAlign,
}

/// Resolve a trigger-aware placement using Floating UI's core policy:
///
/// 1. Keep the requested vertical side while it fits.
/// 2. Flip to the opposite side when it fits better.
/// 3. Try the opposite horizontal alignment, then shift inside the viewport.
///
/// Unlike GPUI's point-based anchor switching, a vertical flip uses the
/// trigger's opposite edge, so the card never lands across the trigger merely
/// because the preferred side ran out of room.
pub(super) fn resolve_floating_placement(
    trigger: Bounds<Pixels>,
    surface_size: Size<Pixels>,
    viewport: Bounds<Pixels>,
    preferred: MenuAlign,
    gap: Pixels,
    margin: Pixels,
) -> FloatingPlacement {
    let viewport_left = f32::from(viewport.left() + margin);
    let viewport_right = f32::from(viewport.right() - margin);
    let viewport_top = f32::from(viewport.top() + margin);
    let viewport_bottom = f32::from(viewport.bottom() - margin);
    let trigger_left = f32::from(trigger.left());
    let trigger_right = f32::from(trigger.right());
    let trigger_top = f32::from(trigger.top());
    let trigger_bottom = f32::from(trigger.bottom());
    let width = f32::from(surface_size.width);
    let height = f32::from(surface_size.height);
    let gap = f32::from(gap);

    let above_space = (trigger_top - gap - viewport_top).max(0.0);
    let below_space = (viewport_bottom - trigger_bottom - gap).max(0.0);
    let preferred_above = preferred.above();
    let preferred_space = if preferred_above {
        above_space
    } else {
        below_space
    };
    let opposite_space = if preferred_above {
        below_space
    } else {
        above_space
    };
    let above = if height <= preferred_space || preferred_space >= opposite_space {
        preferred_above
    } else {
        !preferred_above
    };

    let left_aligned_x = trigger_left;
    let right_aligned_x = trigger_right - width;
    let overflow = |x: f32| (viewport_left - x).max(0.0) + (x + width - viewport_right).max(0.0);
    let preferred_right = preferred.right_aligned();
    let preferred_x = if preferred_right {
        right_aligned_x
    } else {
        left_aligned_x
    };
    let opposite_x = if preferred_right {
        left_aligned_x
    } else {
        right_aligned_x
    };
    let right_aligned = if overflow(preferred_x) <= overflow(opposite_x) {
        preferred_right
    } else {
        !preferred_right
    };
    let mut x = if right_aligned {
        right_aligned_x
    } else {
        left_aligned_x
    };
    let mut y = if above {
        trigger_top - gap - height
    } else {
        trigger_bottom + gap
    };

    // `shift`: keep the chosen side and alignment, moving only enough to stay
    // inside the viewport. If a card is larger than the usable viewport, pin
    // it to the leading edge; a caller can then constrain its own contents.
    let usable_width = (viewport_right - viewport_left).max(0.0);
    if width <= usable_width {
        x = x.clamp(viewport_left, viewport_right - width);
    } else {
        x = viewport_left;
    }
    let usable_height = (viewport_bottom - viewport_top).max(0.0);
    if height <= usable_height {
        y = y.clamp(viewport_top, viewport_bottom - height);
    } else {
        y = viewport_top;
    }

    FloatingPlacement {
        bounds: Bounds::new(Point::new(px(x), px(y)), surface_size),
        align: MenuAlign::from_sides(above, right_aligned),
    }
}

/// A measured, trigger-aware deferred surface. This mirrors GPUI's
/// `Anchored` element lifecycle, but resolves placement from the trigger's
/// rectangle instead of a single point so vertical flips remain attached to
/// the correct edge.
pub(super) struct FloatingSurface {
    child: AnyElement,
    trigger: Bounds<Pixels>,
    preferred: MenuAlign,
    gap: Pixels,
    margin: Pixels,
}

pub(super) struct FloatingSurfaceState {
    child_layout_id: LayoutId,
}

impl FloatingSurface {
    pub(super) fn new(
        child: AnyElement,
        trigger: Bounds<Pixels>,
        preferred: MenuAlign,
        gap: Pixels,
        margin: Pixels,
    ) -> Self {
        Self {
            child,
            trigger,
            preferred,
            gap,
            margin,
        }
    }
}

impl Element for FloatingSurface {
    type RequestLayoutState = FloatingSurfaceState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let child_layout_id = self.child.request_layout(window, cx);
        let layout_id = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                ..Style::default()
            },
            [child_layout_id],
            cx,
        );
        (layout_id, FloatingSurfaceState { child_layout_id })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let surface_size = window.layout_bounds(request_layout.child_layout_id).size;
        let viewport = Bounds::new(Point::default(), window.viewport_size());
        let margin = self.margin + window.client_inset().unwrap_or(px(0.0));
        let placement = resolve_floating_placement(
            self.trigger,
            surface_size,
            viewport,
            self.preferred,
            self.gap,
            margin,
        );
        let offset = placement.bounds.origin - bounds.origin;
        let offset = Point::new(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for FloatingSurface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
