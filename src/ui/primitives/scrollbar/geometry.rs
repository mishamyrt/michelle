use super::*;

/// Track width, and the thumb's resting and hovered widths inside it.
pub(super) const TRACK_WIDTH: f32 = 11.0;
pub(super) const THUMB_WIDTH: f32 = 5.0;
pub(super) const THUMB_WIDTH_ACTIVE: f32 = 8.0;
pub(super) const THUMB_MIN_HEIGHT: f32 = 28.0;
pub(super) const TRACK_INSET: f32 = 2.0;

/// Resolved scrollbar geometry, or `None` when the surface does not scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Geometry {
    /// Thumb rect within the track.
    pub(super) thumb: Bounds<Pixels>,
    /// Travel available to the thumb along the track.
    pub(super) travel: Pixels,
    /// Scrollable content beyond the viewport.
    pub(super) max_offset: Pixels,
}

/// Compute the thumb rect for a track. Pure, so the mapping between scroll
/// offset and thumb position is unit-testable.
pub(super) fn geometry(
    track: Bounds<Pixels>,
    viewport_height: Pixels,
    max_offset: Pixels,
    offset: Pixels,
    thumb_width: Pixels,
) -> Option<Geometry> {
    if viewport_height <= Pixels::ZERO || max_offset <= px(0.5) || track.size.height <= Pixels::ZERO
    {
        return None;
    }
    let content_height = viewport_height + max_offset;
    let track_height = track.size.height;
    let thumb_height = (track_height * (viewport_height / content_height))
        .max(px(THUMB_MIN_HEIGHT))
        .min(track_height);
    let travel = (track_height - thumb_height).max(Pixels::ZERO);
    let progress = (offset / max_offset).clamp(0.0, 1.0);
    Some(Geometry {
        thumb: Bounds::new(
            point(
                track.right() - thumb_width - px(TRACK_INSET),
                track.top() + travel * progress,
            ),
            size(thumb_width, thumb_height),
        ),
        travel,
        max_offset,
    })
}

/// How far down the content a thumb top of `thumb_top` corresponds to.
pub(super) fn offset_for_thumb_top(
    track_top: Pixels,
    thumb_top: Pixels,
    geometry: &Geometry,
) -> Pixels {
    if geometry.travel <= Pixels::ZERO {
        return Pixels::ZERO;
    }
    let progress = ((thumb_top - track_top) / geometry.travel).clamp(0.0, 1.0);
    geometry.max_offset * progress
}
