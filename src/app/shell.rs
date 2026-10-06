//! Window state, cached pane presentation and shell coordination.
use super::*;
pub(super) mod model;
pub(super) use model::*;

pub(super) struct ShellUi {
    pub(in crate::app) sidebar_visible: bool,
    pub(in crate::app) sidebar_width: f32,
    pub(in crate::app) right_panel_visible: bool,
    pub(in crate::app) right_panel_width: f32,
    /// The show/hide slide each panel is in the middle of, if any. Driven by
    /// hand from `render` (see [`motion::WidthTween`]) because the width these
    /// produce is what the transcript column between them is laid out against.
    pub(in crate::app) sidebar_slide: Option<motion::WidthTween>,
    pub(in crate::app) right_panel_slide: Option<motion::WidthTween>,
    /// Width each panel actually occupied in the last frame — where a toggle
    /// starts its slide from, and what the transcript measures itself against
    /// while one is running.
    pub(in crate::app) sidebar_rendered_width: f32,
    pub(in crate::app) right_panel_rendered_width: f32,
    pub(in crate::app) fps_counter_visible: bool,
    pub(in crate::app) panel_resize_drag: Option<PanelResizeDrag>,
    pub(in crate::app) header_drag_armed: bool,
    /// Every menu site in the app, keyed by a stable id. Handles are created on
    /// first use and live as long as the window.
    pub(in crate::app) menus: RefCell<HashMap<SharedString, ContextMenuHandle>>,
    /// Cached islands of the root view; see [`MichellePane`].
    pub(in crate::app) sidebar_pane: Entity<MichellePane>,
    pub(in crate::app) transcript_pane: Entity<MichellePane>,
    pub(in crate::app) right_panel_pane: Entity<MichellePane>,
    /// Live frames-per-second measurement for the header counter.
    pub(in crate::app) fps_last_frame: Instant,
    pub(in crate::app) fps_frame_count: u64,
    pub(in crate::app) fps_value: u32,
}

/// One cached island of the root view: a region rendered by delegating back
/// into [`Michelle`] under its own view identity.
///
/// All state stays on the root entity; what the island buys is scope for
/// gpui's cached-view machinery. The pulse clock and the streaming veil lease
/// `window.current_view()`, so their ~30 fps ticks dirty only the island
/// hosting the animation while every sibling island replays its cached
/// subtree instead of rebuilding. Observing the root preserves the old
/// invalidation semantics exactly — any root notify still re-renders every
/// island — so caching cannot show state the single-view architecture would
/// have repainted.
pub(in crate::app) struct MichellePane {
    pub(in crate::app) michelle: Option<WeakEntity<Michelle>>,
    pub(in crate::app) content:
        fn(&mut Michelle, &mut Window, &mut Context<Michelle>) -> AnyElement,
}

impl MichellePane {
    pub(in crate::app) fn new(
        content: fn(&mut Michelle, &mut Window, &mut Context<Michelle>) -> AnyElement,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|_| Self {
            michelle: None,
            content,
        })
    }

    pub(in crate::app) fn bind(&mut self, michelle: &Entity<Michelle>, cx: &mut Context<Self>) {
        self.michelle = Some(michelle.downgrade());
        cx.observe(michelle, |_, michelle, cx| {
            // A panel slide notifies the root at display rate for its 200ms,
            // and this fan-out would price every one of those ticks at a
            // three-island rebuild. Skipping it hands the decision to the
            // cached-view keys: the sliding panel (its clip moves) and the
            // transcript (its bounds move) miss their caches and re-render
            // with fresh state anyway, while the island nothing is moving
            // re-plays its cached subtree. Root-state changes it displays
            // can wait out the slide: updates born inside an island
            // (terminal output, pulse leases) dirty their ancestor pane
            // without this observer, and the slide's retirement notify
            // below re-runs the fan-out, so nothing outlasts the 200ms.
            if !michelle.read(cx).panels_sliding() {
                cx.notify();
            }
        })
        .detach();
    }
}

impl Render for MichellePane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(michelle) = self.michelle.as_ref().and_then(WeakEntity::upgrade) else {
            return gpui::div().into_any_element();
        };
        let content = self.content;
        michelle.update(cx, |michelle, cx| content(michelle, window, cx))
    }
}
