use super::*;

fn track() -> Bounds<Pixels> {
    Bounds::new(point(px(500.0), px(100.0)), size(px(11.0), px(400.0)))
}

#[test]
fn the_bar_rests_hidden_and_reveals_on_scroll() {
    // Nothing has scrolled yet, so there is nothing to show.
    assert_eq!(opacity(None, false, false), 0.0);

    // A scroll reveals it at full strength, and it holds there.
    assert_eq!(opacity(Some(Duration::ZERO), false, false), 1.0);
    assert_eq!(
        opacity(Some(HOLD - Duration::from_millis(1)), false, false),
        1.0
    );

    // Then it fades out over FADE and stays gone.
    let midway = opacity(Some(HOLD + FADE / 2), false, false);
    assert!(
        (0.4..0.6).contains(&midway),
        "expected a half fade, got {midway}"
    );
    assert_eq!(opacity(Some(HOLD + FADE), false, false), 0.0);
    assert_eq!(opacity(Some(HOLD + FADE * 10), false, false), 0.0);
}

#[test]
fn hovering_or_dragging_pins_the_bar_visible() {
    // Long past the fade, but the pointer is on the track.
    assert_eq!(opacity(Some(HOLD + FADE * 10), true, false), 1.0);
    // Mid-drag the pointer may leave the track entirely.
    assert_eq!(opacity(Some(HOLD + FADE * 10), false, true), 1.0);
    // A drag that began before anything scrolled still shows.
    assert_eq!(opacity(None, false, true), 1.0);
}

#[test]
fn the_first_observed_offset_only_seeds_the_baseline() {
    let state = ScrollbarState::default();
    let start = Instant::now();

    // Opening a transcript already scrolled to its tail must not flash.
    state.observe(px(4_000.0), start);
    assert_eq!(state.last_scroll.get(), None);

    // A real movement starts the timer.
    state.observe(px(3_900.0), start);
    assert!(state.last_scroll.get().is_some());

    // Sub-pixel jitter from remeasurement does not count as movement.
    state.last_scroll.set(None);
    state.observe(px(3_900.2), start);
    assert_eq!(state.last_scroll.get(), None);
}

#[test]
fn a_surface_that_does_not_scroll_has_no_thumb() {
    assert!(geometry(track(), px(400.0), Pixels::ZERO, Pixels::ZERO, px(5.0)).is_none());
    assert!(geometry(track(), Pixels::ZERO, px(900.0), Pixels::ZERO, px(5.0)).is_none());
}

#[test]
fn thumb_height_tracks_the_visible_fraction() {
    // Viewport is a quarter of the content, so the thumb is a quarter of
    // the track.
    let geometry = geometry(track(), px(400.0), px(1200.0), Pixels::ZERO, px(5.0)).unwrap();
    assert_eq!(geometry.thumb.size.height, px(100.0));
    assert_eq!(geometry.thumb.top(), px(100.0));
    assert_eq!(geometry.travel, px(300.0));
}

#[test]
fn a_tiny_visible_fraction_still_leaves_a_grabbable_thumb() {
    let geometry = geometry(track(), px(400.0), px(100_000.0), Pixels::ZERO, px(5.0)).unwrap();
    assert_eq!(geometry.thumb.size.height, px(THUMB_MIN_HEIGHT));
}

#[test]
fn thumb_position_and_offset_are_inverse() {
    let track = track();
    let geometry = geometry(track, px(400.0), px(1200.0), px(600.0), px(5.0)).unwrap();
    // Halfway down the content puts the thumb halfway along its travel.
    assert_eq!(geometry.thumb.top(), track.top() + px(150.0));
    assert_eq!(
        offset_for_thumb_top(track.top(), geometry.thumb.top(), &geometry),
        px(600.0)
    );
}

#[test]
fn offsets_clamp_at_both_ends() {
    let track = track();
    let geometry = geometry(track, px(400.0), px(1200.0), px(1200.0), px(5.0)).unwrap();
    assert_eq!(
        geometry.thumb.top(),
        track.bottom() - geometry.thumb.size.height
    );

    assert_eq!(
        offset_for_thumb_top(track.top(), track.top() - px(9_999.0), &geometry),
        Pixels::ZERO
    );
    assert_eq!(
        offset_for_thumb_top(track.top(), track.bottom() + px(9_999.0), &geometry),
        px(1200.0)
    );
}

#[test]
fn overscrolled_offsets_do_not_push_the_thumb_past_the_track() {
    let track = track();
    // A momentum overscroll can report more than max for a frame.
    let geometry = geometry(track, px(400.0), px(1200.0), px(5000.0), px(5.0)).unwrap();
    assert!(geometry.thumb.bottom() <= track.bottom() + px(0.001));
}
