//! Shell geometry and background label-clock state.
use super::*;

pub(in crate::app) struct ShellModel {
    /// Cached before rendering so path labels can abbreviate the home prefix
    /// without consulting the environment or account database in a frame.
    pub(in crate::app) home_directory: Option<PathBuf>,
    /// Cached before rendering to classify projectless workspaces by path.
    pub(in crate::app) projectless_root: Option<PathBuf>,
    /// Installed folder-capable apps for the header's "open project in"
    /// control, icons included, resolved once at launch on the background
    /// executor. Render only reads this; empty means not resolved yet (or
    /// nothing to offer) and hides the control.
    pub(in crate::app) open_in_apps: Rc<Vec<crate::platform::ExternalApp>>,
    /// The unix second the pending time-label wake-up targets, or `None` when
    /// none is armed. See `schedule_time_label_wake`.
    pub(in crate::app) time_label_wake: Cell<Option<u64>>,
    /// Bumped per (re)arm so a superseded wake-up discards itself.
    pub(in crate::app) time_label_wake_generation: Cell<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum PanelResizeTarget {
    Sidebar,
    RightPanel,
    FileTree,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct PanelResizeDrag {
    pub(in crate::app) target: PanelResizeTarget,
    pub(in crate::app) start_mouse_x: f32,
    pub(in crate::app) start_width: f32,
}

pub(in crate::app) fn sanitize_panel_width(width: f32, default: f32, min: f32, max: f32) -> f32 {
    if width.is_finite() {
        width.clamp(min, max)
    } else {
        default
    }
}

pub(in crate::app) fn persisted_window_state(
    bounds: Bounds<Pixels>,
    maximized: bool,
    display: Option<Uuid>,
) -> PersistedWindowState {
    PersistedWindowState {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
        maximized,
        display,
    }
}

pub(in crate::app) fn fitted_file_tree_width(panel_width: f32, file_tree_width: f32) -> f32 {
    let maximum = FILE_TREE_MAX_WIDTH
        .min(panel_width - FILE_EDITOR_MIN_WIDTH)
        .max(FILE_TREE_MIN_WIDTH);
    sanitize_panel_width(
        file_tree_width,
        DEFAULT_FILE_TREE_WIDTH.clamp(FILE_TREE_MIN_WIDTH, maximum),
        FILE_TREE_MIN_WIDTH,
        maximum,
    )
}

pub(in crate::app) fn widened_panel_width_for_file_editor(
    panel_width: f32,
    file_tree_width: f32,
) -> f32 {
    let panel_width = sanitize_panel_width(
        panel_width,
        DEFAULT_RIGHT_PANEL_WIDTH,
        RIGHT_PANEL_MIN_WIDTH,
        RIGHT_PANEL_MAX_WIDTH,
    );
    let file_tree_width = sanitize_panel_width(
        file_tree_width,
        DEFAULT_FILE_TREE_WIDTH,
        FILE_TREE_MIN_WIDTH,
        FILE_TREE_MAX_WIDTH,
    );
    panel_width
        .max(file_tree_width + FILE_EDITOR_INITIAL_WIDTH)
        .min(RIGHT_PANEL_MAX_WIDTH)
}

pub(in crate::app) fn widened_panel_width_for_review(panel_width: f32) -> f32 {
    sanitize_panel_width(
        panel_width,
        DEFAULT_RIGHT_PANEL_WIDTH,
        RIGHT_PANEL_MIN_WIDTH,
        RIGHT_PANEL_MAX_WIDTH,
    )
    .max(REVIEW_INITIAL_WIDTH)
}

pub(in crate::app) fn fitted_panel_widths(
    viewport_width: f32,
    sidebar_visible: bool,
    right_panel_visible: bool,
    sidebar_width: f32,
    right_panel_width: f32,
) -> (f32, f32) {
    let sidebar_min = if sidebar_visible {
        SIDEBAR_MIN_WIDTH
    } else {
        0.0
    };
    let right_panel_min = if right_panel_visible {
        RIGHT_PANEL_MIN_WIDTH
    } else {
        0.0
    };
    let mut sidebar = if sidebar_visible {
        sanitize_panel_width(
            sidebar_width,
            DEFAULT_SIDEBAR_WIDTH,
            SIDEBAR_MIN_WIDTH,
            SIDEBAR_MAX_WIDTH,
        )
    } else {
        0.0
    };
    let mut right_panel = if right_panel_visible {
        sanitize_panel_width(
            right_panel_width,
            DEFAULT_RIGHT_PANEL_WIDTH,
            RIGHT_PANEL_MIN_WIDTH,
            RIGHT_PANEL_MAX_WIDTH,
        )
    } else {
        0.0
    };

    let available = (viewport_width - MAIN_PANEL_MIN_WIDTH).max(0.0);
    let mut overflow = (sidebar + right_panel - available).max(0.0);
    let right_reduction = overflow.min((right_panel - right_panel_min).max(0.0));
    right_panel -= right_reduction;
    overflow -= right_reduction;
    let sidebar_reduction = overflow.min((sidebar - sidebar_min).max(0.0));
    sidebar -= sidebar_reduction;
    overflow -= sidebar_reduction;

    // The configured minimum window easily fits both panel minima. This final
    // fallback only protects layout if the host temporarily reports a smaller
    // viewport during a resize or display transition.
    if overflow > 0.0 {
        let right_reduction = overflow.min(right_panel);
        right_panel -= right_reduction;
        overflow -= right_reduction;
        sidebar = (sidebar - overflow).max(0.0);
    }

    (sidebar, right_panel)
}
impl Michelle {
    /// Arm one wake-up for the moment a time-derived label next changes —
    /// the sidebar's relative reply times and every "Working for Ns" elapsed.
    ///
    /// There is deliberately no standing timer. Render calls this each frame;
    /// while the scheduled instant is unchanged it is a `Cell` comparison and
    /// nothing spawns. The timer fires exactly when a visible label rolls to
    /// its next value, notifies once, and the frame that draws the new value
    /// arms the next boundary. An idle window with hour-old sessions wakes
    /// once an hour; with nothing to show it wakes never. (T3 Code's
    /// equivalent is one minute-aligned interval gated on subscribers; label
    /// boundaries make even that unnecessary.) A busy session pins the chain
    /// to one-second steps — that is what keeps its elapsed counters moving
    /// under reduce-motion, where the pulse animations that normally drive
    /// frames are suppressed, and while a background turn sits between
    /// stream events.
    pub(in crate::app) fn schedule_time_label_wake(&self, cx: &mut Context<Self>) {
        let now = unix_time();
        let target = next_time_label_change(&self.state.sessions, now).map(|seconds| now + seconds);
        if self.shell_model.time_label_wake.get() == target {
            return;
        }
        self.shell_model.time_label_wake.set(target);
        let generation = self
            .shell_model
            .time_label_wake_generation
            .get()
            .wrapping_add(1);
        self.shell_model.time_label_wake_generation.set(generation);
        let Some(target) = target else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let delay = target.saturating_sub(unix_time()).max(1);
            cx.background_executor()
                .timer(Duration::from_secs(delay))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.shell_model.time_label_wake_generation.get() != generation {
                    return;
                }
                // Consumed: the notified frame re-arms the next boundary.
                this.shell_model.time_label_wake.set(None);
                cx.notify();
            });
        })
        .detach();
    }
}

/// Seconds until any session's time label next changes value, or `None` when
/// no label is on the clock at all. A running turn's elapsed counter moves
/// every second; a settled reply's "5m"/"3h"/"2d" moves only at its unit
/// boundary, so the wake-up this feeds gets rarer as the history ages.
pub(in crate::app) fn next_time_label_change(sessions: &[AgentSession], now: u64) -> Option<u64> {
    let mut next: Option<u64> = None;
    for session in sessions {
        if session.is_busy()
            && session
                .turns
                .last()
                .is_some_and(|turn| turn.status == TurnStatus::Running)
        {
            return Some(1);
        }
        if let Some(last_reply_at) = session.last_reply_at {
            let elapsed = now.saturating_sub(last_reply_at);
            let step = match elapsed {
                0..=3_599 => 60,
                3_600..=86_399 => 3_600,
                _ => 86_400,
            };
            let remaining = (step - elapsed % step).max(1);
            next = Some(next.map_or(remaining, |next| next.min(remaining)));
        }
    }
    next
}
