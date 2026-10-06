//! Right panel session state operations.
use super::*;

pub(in crate::app) struct RightPanelSessionState {
    pub(in crate::app) visible: bool,
    pub(in crate::app) surfaces: Vec<RightPanelSurface>,
    pub(in crate::app) active_surface: Option<usize>,
    pub(in crate::app) tabs_scroll_handle: ScrollHandle,
    pub(in crate::app) pending_tab_reveal: Option<usize>,
    pub(in crate::app) expanded_paths: HashSet<PathBuf>,
    pub(in crate::app) files_selected_path: Option<String>,
    pub(in crate::app) file_tree_width: f32,
    pub(in crate::app) file_editors: HashMap<String, RightPanelFileEditor>,
    pub(in crate::app) diff_source: ReviewDiffSource,
    pub(in crate::app) diff_snapshot: Option<Arc<ReviewDiffSnapshot>>,
    pub(in crate::app) diff_selected_file: Option<usize>,
    pub(in crate::app) diff_expanded_paths: HashSet<String>,
}

impl RightPanelSessionState {
    pub(in crate::app) fn empty(visible: bool) -> Self {
        Self {
            visible,
            surfaces: Vec::new(),
            active_surface: None,
            tabs_scroll_handle: ScrollHandle::new(),
            pending_tab_reveal: None,
            expanded_paths: HashSet::new(),
            files_selected_path: None,
            file_tree_width: DEFAULT_FILE_TREE_WIDTH,
            file_editors: HashMap::new(),
            diff_source: ReviewDiffSource::default(),
            diff_snapshot: None,
            diff_selected_file: None,
            diff_expanded_paths: HashSet::new(),
        }
    }

    pub(in crate::app) fn take_or_closed(
        states: &mut HashMap<Uuid, Self>,
        session_id: Uuid,
    ) -> Self {
        states
            .remove(&session_id)
            .unwrap_or_else(|| Self::empty(false))
    }
}

impl Michelle {
    pub(in crate::app) fn store_selected_right_panel_state(&mut self) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let state = self.take_active_right_panel_state();
        self.right_panel_model
            .session_states
            .insert(session_id, state);
    }

    pub(in crate::app) fn restore_right_panel_state(
        &mut self,
        session_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let state = RightPanelSessionState::take_or_closed(
            &mut self.right_panel_model.session_states,
            session_id,
        );
        self.replace_active_right_panel_state(state);
        self.sync_right_panel_diff_tree_rows(cx);
        // A read in flight when this session was switched away from had its
        // result dropped, and the flag it left behind would stop the editor
        // ever asking again. Clear it and read afresh, which also picks up
        // edits made while another session was on screen.
        for editor in self.right_panel_model.file_editors.values_mut() {
            editor.reading = false;
        }
        // The find bar pointed into the editors that were just swapped out;
        // its match list means nothing here, and restored editors may carry
        // washes stored mid-search.
        self.reset_file_search_for_session(cx);
        self.reload_clean_right_panel_file_editors(cx);
        self.state.right_panel_visible = self.shell_ui.right_panel_visible;
        if self.active_right_panel_surface() == Some(&RightPanelSurface::Diff) {
            self.refresh_right_panel_diff(cx);
        }
        if matches!(
            self.active_right_panel_surface(),
            Some(RightPanelSurface::Files | RightPanelSurface::File(_))
        ) {
            self.refresh_right_panel_working_tree(cx);
        }
        self.ensure_right_panel_terminals(cx);
        if self.shell_ui.right_panel_visible {
            self.request_active_terminal_focus();
        }
    }

    pub(in crate::app) fn remove_right_panel_session_state(&mut self, session_id: Uuid) {
        let state = if self.state.selected_session == Some(session_id) {
            let state = self.take_active_right_panel_state();
            self.replace_active_right_panel_state(RightPanelSessionState::empty(false));
            Some(state)
        } else {
            self.right_panel_model.session_states.remove(&session_id)
        };
        if let Some(state) = state {
            for surface in &state.surfaces {
                if let Some(terminal_id) = surface.terminal_id() {
                    self.right_panel_model.terminals.remove(&terminal_id);
                }
            }
        }
    }

    fn take_active_right_panel_state(&mut self) -> RightPanelSessionState {
        RightPanelSessionState {
            visible: self.shell_ui.right_panel_visible,
            surfaces: std::mem::take(&mut self.right_panel_ui.surfaces),
            active_surface: self.right_panel_ui.active_surface.take(),
            tabs_scroll_handle: std::mem::replace(
                &mut self.right_panel_ui.tabs_scroll_handle,
                ScrollHandle::new(),
            ),
            pending_tab_reveal: self.right_panel_ui.pending_tab_reveal.take(),
            expanded_paths: std::mem::take(&mut self.right_panel_ui.expanded_paths),
            files_selected_path: self.right_panel_ui.files_selected_path.take(),
            file_tree_width: self.right_panel_ui.file_tree_width,
            file_editors: std::mem::take(&mut self.right_panel_model.file_editors),
            diff_source: self.right_panel_model.diff_source,
            diff_snapshot: self.right_panel_model.diff_snapshot.take(),
            diff_selected_file: self.right_panel_ui.diff_selected_file.take(),
            diff_expanded_paths: std::mem::take(&mut self.right_panel_ui.diff_expanded_paths),
        }
    }

    fn replace_active_right_panel_state(&mut self, state: RightPanelSessionState) {
        self.shell_ui.right_panel_visible = state.visible;
        self.right_panel_ui.surfaces = state.surfaces;
        self.right_panel_ui.active_surface = state.active_surface;
        self.right_panel_ui.tabs_scroll_handle = state.tabs_scroll_handle;
        self.right_panel_ui.pending_tab_reveal = state.pending_tab_reveal;
        self.right_panel_ui.expanded_paths = state.expanded_paths;
        self.right_panel_ui.files_selected_path = state.files_selected_path;
        self.right_panel_ui.file_tree_width = state.file_tree_width;
        self.right_panel_model.file_editors = state.file_editors;
        self.right_panel_model.diff_generation =
            self.right_panel_model.diff_generation.wrapping_add(1);
        self.right_panel_ui.diff_selection.clear();
        self.right_panel_model.diff_source = state.diff_source;
        self.right_panel_model.diff_snapshot = state.diff_snapshot;
        self.right_panel_model.diff_loading = false;
        self.right_panel_model.diff_error = None;
        self.right_panel_ui.diff_selected_file = state.diff_selected_file;
        self.right_panel_ui.diff_expanded_paths = state.diff_expanded_paths;
        self.right_panel_ui.diff_tree_cursor = None;
        self.right_panel_ui.diff_tree_rows.borrow_mut().clear();
        self.right_panel_ui.diff_tree_list_state.reset(0);
        let line_count = self
            .right_panel_model
            .diff_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.lines.len());
        self.right_panel_ui.diff_list_state.reset(line_count);
    }
}

impl Michelle {
    pub(in crate::app) fn open_right_panel_surface(
        &mut self,
        surface: RightPanelSurface,
        cx: &mut Context<Self>,
    ) {
        let reusable_index = reusable_surface_index(&self.right_panel_ui.surfaces, &surface);
        if matches!(&surface, RightPanelSurface::File(_)) {
            self.ensure_initial_right_panel_file_editor_width();
        }
        if surface == RightPanelSurface::Diff {
            if reusable_index.is_none() {
                self.shell_ui.right_panel_width =
                    widened_panel_width_for_review(self.shell_ui.right_panel_width);
            }
            self.refresh_right_panel_diff(cx);
        }
        if matches!(
            surface,
            RightPanelSurface::Files | RightPanelSurface::File(_)
        ) {
            self.refresh_right_panel_working_tree(cx);
        }
        if let Some(terminal_id) = surface.terminal_id() {
            self.ensure_right_panel_terminal(terminal_id, cx);
        }
        let index = match reusable_index {
            Some(index) => index,
            None => {
                self.right_panel_ui.surfaces.push(surface);
                self.right_panel_ui.surfaces.len() - 1
            }
        };
        self.right_panel_ui.active_surface = Some(index);
        self.reveal_right_panel_tab(index);
        self.request_active_terminal_focus();
        self.set_right_panel_visible(true, cx);
        cx.notify();
    }

    pub(in crate::app) fn open_turn_diff(&mut self, turn_id: Uuid, cx: &mut Context<Self>) {
        let Some((session_id, turn_count)) = self.selected_session().and_then(|session| {
            session
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .map(|turn| (session.id, turn.turn_count))
        }) else {
            return;
        };
        self.right_panel_model.diff_source = ReviewDiffSource::LastTurn {
            session_id,
            turn_id,
            turn_count,
        };
        self.right_panel_ui.diff_selection.clear();
        self.right_panel_model.diff_snapshot = None;
        self.right_panel_ui.diff_selected_file = None;
        self.open_right_panel_surface(RightPanelSurface::Diff, cx);
    }

    pub(in crate::app) fn open_right_panel_file(
        &mut self,
        relative_path: String,
        cx: &mut Context<Self>,
    ) {
        self.ensure_initial_right_panel_file_editor_width();
        let Some(active) = self.right_panel_ui.active_surface else {
            self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
            return;
        };
        match self.right_panel_ui.surfaces.get(active).cloned() {
            Some(RightPanelSurface::Files) => {
                let dirty_file_would_be_replaced = self
                    .right_panel_ui
                    .files_selected_path
                    .as_deref()
                    .is_some_and(|current_path| {
                        current_path != relative_path
                            && self.right_panel_file_is_dirty(current_path)
                    });
                if dirty_file_would_be_replaced {
                    self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
                    return;
                }

                self.right_panel_ui.files_selected_path = Some(relative_path);
                self.set_right_panel_visible(true, cx);
                cx.notify();
            }
            Some(RightPanelSurface::File(current_path)) => {
                if current_path == relative_path {
                    return;
                }
                if self.right_panel_file_is_dirty(&current_path) {
                    self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx);
                    return;
                }

                let requested = RightPanelSurface::File(relative_path);
                if let Some(existing) =
                    reusable_surface_index(&self.right_panel_ui.surfaces, &requested)
                {
                    self.right_panel_ui.surfaces.remove(active);
                    let existing = if existing > active {
                        existing - 1
                    } else {
                        existing
                    };
                    self.right_panel_ui.active_surface = Some(existing);
                    self.reveal_right_panel_tab(existing);
                } else {
                    self.right_panel_ui.surfaces[active] = requested;
                    self.reveal_right_panel_tab(active);
                }
                self.set_right_panel_visible(true, cx);
                cx.notify();
            }
            _ => self.open_right_panel_surface(RightPanelSurface::File(relative_path), cx),
        }
    }

    pub(in crate::app) fn close_right_panel_surface(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if index >= self.right_panel_ui.surfaces.len() {
            return;
        }
        if let Some(terminal_id) = self.right_panel_ui.surfaces[index].terminal_id() {
            self.right_panel_model.terminals.remove(&terminal_id);
        }
        self.right_panel_ui.surfaces.remove(index);
        self.right_panel_ui.active_surface = if self.right_panel_ui.surfaces.is_empty() {
            None
        } else {
            Some(match self.right_panel_ui.active_surface {
                Some(active) if active > index => active - 1,
                Some(active) if active == index => index.saturating_sub(1),
                Some(active) => active.min(self.right_panel_ui.surfaces.len() - 1),
                None => 0,
            })
        };
        if let Some(active) = self.right_panel_ui.active_surface {
            self.reveal_right_panel_tab(active);
            self.request_active_terminal_focus();
        } else {
            self.right_panel_ui.pending_tab_reveal = None;
            self.right_panel_ui.pending_terminal_focus = None;
            self.set_right_panel_visible(false, cx);
        }
        cx.notify();
    }
}
