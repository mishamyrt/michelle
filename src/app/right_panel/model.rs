//! Shared data and resources for the right panel.
use super::*;

pub(in crate::app) struct RightPanelModel {
    pub(in crate::app) session_states: HashMap<Uuid, RightPanelSessionState>,
    pub(in crate::app) file_editors: HashMap<String, RightPanelFileEditor>,
    pub(in crate::app) diff_source: ReviewDiffSource,
    pub(in crate::app) diff_snapshot: Option<Arc<ReviewDiffSnapshot>>,
    pub(in crate::app) diff_loading: bool,
    pub(in crate::app) diff_error: Option<String>,
    pub(in crate::app) diff_generation: u64,
    /// The working tree as currently drawn. Held so a refresh can redraw the
    /// previous listing instead of blanking the panel.
    pub(in crate::app) working_tree: Vec<right_panel::WorkingTreeEntry>,
    /// Working tree per project path. Walking it is filesystem I/O and must
    /// never happen in a frame.
    pub(in crate::app) working_trees: QueryCache<PathBuf, Vec<right_panel::WorkingTreeEntry>>,
    /// Set when a turn finishes; the drain loop drops the workspace queries,
    /// since the event handler has no `Context` to refresh them itself.
    pub(in crate::app) workspace_queries_stale: bool,
    pub(in crate::app) terminals: HashMap<Uuid, Entity<TerminalView>>,
}
