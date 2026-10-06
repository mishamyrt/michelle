use super::*;

/// How long the bar stays at full strength after the last scroll, and how long
/// it then takes to fade out. Tuned to feel like AppKit's overlay scrollers.
pub(super) const HOLD: Duration = Duration::from_millis(900);
pub(super) const FADE: Duration = Duration::from_millis(350);

/// Cross-frame scrollbar state. The owner holds one per scrollable surface.
#[derive(Debug, Default)]
pub struct ScrollbarState {
    /// While dragging: the pointer's offset inside the thumb, in pixels.
    pub(super) grab_offset: Cell<Option<f32>>,
    pub(super) hovered: Cell<bool>,
    /// When the content last moved, which starts the hold-then-fade timer.
    pub(super) last_scroll: Cell<Option<Instant>>,
    /// Offset at the previous paint, to notice movement.
    pub(super) last_offset: Cell<Option<Pixels>>,
    /// A hold-expiry wake is in flight; see `arm_fade_wake`.
    pub(super) fade_wake_armed: Cell<bool>,
}

impl ScrollbarState {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// True while the thumb is held. The bar writes offsets straight into the
    /// surface without going through its scroll handler, so a surface that
    /// tracks its own scroll intent has no other way to hear about a drag.
    pub fn is_grabbed(&self) -> bool {
        self.grab_offset.get().is_some()
    }

    /// True while the pointer is over the track or a drag is in progress.
    /// Bubble-phase mouse listeners run in reverse registration order, so a
    /// surface painted beneath the bar hears these events too and must be able
    /// to ignore the ones the bar is handling.
    pub fn engaged(&self) -> bool {
        self.hovered.get() || self.is_grabbed()
    }

    /// Note the current scroll offset, starting the reveal timer when it moved.
    /// The first observation only seeds the baseline, so a transcript that opens
    /// already scrolled to its tail does not flash its scrollbar.
    pub(super) fn observe(&self, offset: Pixels, now: Instant) {
        match self.last_offset.replace(Some(offset)) {
            Some(previous) if (offset - previous).abs() > px(0.5) => {
                self.last_scroll.set(Some(now));
            }
            _ => {}
        }
    }
}

/// Overlay opacity: solid while hovered or dragging, otherwise held briefly
/// after a scroll and then faded out. Pure, so the timing is testable.
pub(super) fn opacity(since_scroll: Option<Duration>, hovered: bool, grabbed: bool) -> f32 {
    if hovered || grabbed {
        return 1.0;
    }
    let Some(elapsed) = since_scroll else {
        return 0.0;
    };
    if elapsed < HOLD {
        return 1.0;
    }
    let fading = (elapsed - HOLD).as_secs_f32() / FADE.as_secs_f32();
    (1.0 - fading).clamp(0.0, 1.0)
}

/// One in-flight wake for the end of the reveal hold. If the content keeps
/// moving, the paint that this wake triggers finds the hold extended and arms
/// the next wake — one timer alive at a time, one no-op frame per expiry.
pub(super) fn arm_fade_wake(
    state: &Rc<ScrollbarState>,
    view: gpui::EntityId,
    delay: Duration,
    cx: &mut App,
) {
    if state.fade_wake_armed.replace(true) {
        return;
    }
    let state = state.clone();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(delay + Duration::from_millis(16))
            .await;
        state.fade_wake_armed.set(false);
        cx.update(|cx| cx.notify(view));
    })
    .detach();
}
