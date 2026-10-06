use super::*;

#[derive(Clone, Copy)]
pub enum FadeEdge {
    Top,
    Bottom,
}

/// A paint-only cue at an edge with content outside the viewport. Place after
/// the scrollable child so its freshly laid-out bounds decide visibility.
pub fn edge_fade<S>(scroll: S, side: FadeEdge, surface: Hsla) -> impl IntoElement
where
    S: Scrollable + 'static,
{
    canvas(
        move |bounds, _, _| {
            let scrolled = scroll.scrolled();
            let max_offset = Scrollable::max_offset(&scroll);
            let visible = match side {
                FadeEdge::Top => scrolled > px(0.5),
                FadeEdge::Bottom => max_offset - scrolled > px(0.5),
            };
            visible.then(|| {
                let transparent = surface.opacity(0.0);
                let background = match side {
                    FadeEdge::Top => linear_gradient(
                        180.0,
                        linear_color_stop(surface, 0.0),
                        linear_color_stop(transparent, 1.0),
                    ),
                    FadeEdge::Bottom => linear_gradient(
                        180.0,
                        linear_color_stop(transparent, 0.0),
                        linear_color_stop(surface, 1.0),
                    ),
                };
                fill(bounds, background)
            })
        },
        |_, fade, window, _| {
            if let Some(fade) = fade {
                window.paint_quad(fade);
            }
        },
    )
    .absolute()
    .left_0()
    .w_full()
    .h(px(18.0))
    .when(matches!(side, FadeEdge::Top), |element| element.top_0())
    .when(matches!(side, FadeEdge::Bottom), |element| {
        element.bottom_0()
    })
}

/// An overlay vertical scrollbar pinned to the right edge of its parent.
///
/// The parent must be `relative()`; this element positions itself absolutely
/// and never participates in layout, so adding it cannot change content size.
pub fn vertical<S>(surface: &S, state: &Rc<ScrollbarState>) -> impl IntoElement + Styled + use<S>
where
    S: Scrollable + Clone + 'static,
{
    let list = surface.clone();
    let state = state.clone();
    canvas(
        |_, _, _| (),
        move |track: Bounds<Pixels>, _, window: &mut Window, cx: &mut App| {
            let theme = UiColors::current(cx);
            let viewport_height = list.viewport_height();
            let max_offset = Scrollable::max_offset(&list);
            let offset = list.scrolled();
            let now = Instant::now();
            state.observe(offset, now);

            let hovered = state.hovered.get();
            let grabbed = state.is_grabbed();
            let active = hovered || grabbed;
            let thumb_width = px(if active {
                THUMB_WIDTH_ACTIVE
            } else {
                THUMB_WIDTH
            });

            let Some(geometry) = geometry(track, viewport_height, max_offset, offset, thumb_width)
            else {
                // Not scrollable: drop any stale drag so a later resize cannot
                // resume one, and paint nothing.
                state.grab_offset.set(None);
                state.hovered.set(false);
                return;
            };

            let since_scroll = state
                .last_scroll
                .get()
                .map(|last| now.saturating_duration_since(last));
            let opacity = opacity(since_scroll, hovered, grabbed);
            if opacity > 0.0 {
                window.paint_quad(quad(
                    geometry.thumb,
                    thumb_width / 2.0,
                    if active {
                        theme.text_tertiary
                    } else {
                        theme.text_ghost.opacity(0.55)
                    }
                    .opacity(opacity),
                    px(0.0),
                    gpui::transparent_black(),
                    BorderStyle::default(),
                ));
                if !active {
                    match since_scroll {
                        // The hold is constant-opacity: it needs no repaints,
                        // only a wake at the moment the fade should begin. A
                        // streaming transcript moves its content every commit
                        // and so holds its bar for the whole turn — driving
                        // frames through that hold pinned the pane at pulse
                        // rate for nothing.
                        Some(elapsed) if elapsed < HOLD => {
                            arm_fade_wake(&state, window.current_view(), HOLD - elapsed, cx);
                        }
                        // The fade itself animates; ride the shared pulse
                        // clock, which parks shortly after the bar hides.
                        _ => crate::ui::motion::pulse_lease(window.current_view(), cx),
                    }
                }
            }

            // Hover is tracked from move events rather than by an interactive
            // child: the bar has to be able to reveal itself while invisible,
            // and a hidden child could not be hovered.
            window.on_mouse_event({
                let state = state.clone();
                move |event: &MouseMoveEvent, phase, window, _| {
                    if phase != gpui::DispatchPhase::Bubble {
                        return;
                    }
                    let hovering = track.contains(&event.position);
                    if state.hovered.replace(hovering) != hovering {
                        window.refresh();
                    }
                }
            });

            window.on_mouse_event({
                let list = list.clone();
                let state = state.clone();
                move |event: &MouseDownEvent, phase, window, _| {
                    if phase != gpui::DispatchPhase::Bubble
                        || event.button != MouseButton::Left
                        || !track.contains(&event.position)
                    {
                        return;
                    }
                    if geometry.thumb.contains(&event.position) {
                        state
                            .grab_offset
                            .set(Some(f32::from(event.position.y - geometry.thumb.top())));
                    } else {
                        // A click on bare track centres the thumb there and
                        // begins dragging from its middle.
                        let half = geometry.thumb.size.height / 2.0;
                        state.grab_offset.set(Some(f32::from(half)));
                        scroll_to(
                            &list,
                            offset_for_thumb_top(track.top(), event.position.y - half, &geometry),
                            geometry.max_offset,
                        );
                    }
                    window.refresh();
                }
            });

            window.on_mouse_event({
                let list = list.clone();
                let state = state.clone();
                move |event: &MouseMoveEvent, phase, window, _| {
                    if phase != gpui::DispatchPhase::Bubble {
                        return;
                    }
                    let Some(grab) = state.grab_offset.get() else {
                        return;
                    };
                    scroll_to(
                        &list,
                        offset_for_thumb_top(track.top(), event.position.y - px(grab), &geometry),
                        geometry.max_offset,
                    );
                    window.refresh();
                }
            });

            window.on_mouse_event({
                let state = state.clone();
                move |_: &MouseUpEvent, phase, window, _| {
                    if phase != gpui::DispatchPhase::Bubble || state.grab_offset.get().is_none() {
                        return;
                    }
                    state.grab_offset.set(None);
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .right_0()
    .h_full()
    .w(px(TRACK_WIDTH))
}
