use super::*;

pub(super) mod activation;
pub(super) mod checkpoints;
pub(super) mod interactions;
pub(super) mod model;
pub(super) mod queue;
use activation::SessionActivationTransition;

fn retain_runtime_after_cancel(provider: ProviderKind) -> bool {
    // Codex's app-server owns the Computer Use process tree, and Amp offers no
    // interrupt on its stream — stopping it means ending the process. Both
    // resume their native thread on the next prompt.
    !matches!(provider, ProviderKind::Codex | ProviderKind::Amp)
}

fn new_task_runtime_mode(current: Option<&AgentSession>, remembered: RuntimeMode) -> RuntimeMode {
    current
        .map(|session| session.runtime_mode)
        .unwrap_or(remembered)
}

impl Michelle {
    pub(crate) fn open_task_from_notification(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        self.select_session(session_id, cx);
    }

    /// Drops cached answers about the workspace on disk.
    ///
    /// These queries cache to keep `git` and directory walks out of frames, but
    /// nothing tells us when the working tree changes underneath. Rather than
    /// expire on a timer, they are dropped at the moments the answer plausibly
    /// moved — coming back to the window, or a turn finishing.
    pub(super) fn invalidate_workspace_queries(&mut self, cx: &mut Context<Self>) {
        let Some(workspace_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            return;
        };
        self.branches.snapshots.invalidate(&workspace_path);
        self.refresh_workspace_surfaces(cx);
        self.invalidate_composer_sources(cx);
    }

    pub(super) fn new_session_action(
        &mut self,
        _: &NewSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.page = None;
        let current_project = self
            .selected_project()
            .map(|project| (project.id, project.is_projectless()));
        match current_project {
            Some((_, true)) => self.create_projectless_session(cx),
            Some((project_id, false)) => {
                if let Some(session_id) = self
                    .sessions
                    .activation
                    .navigation
                    .remembered_new_task(&self.state.sessions, project_id)
                {
                    self.select_session(session_id, cx);
                } else {
                    self.create_session_for(project_id, self.state.last_provider, cx);
                }
            }
            None => self.create_projectless_session(cx),
        }
        let focus_handle = self.composer_focus(cx);
        window.focus(&focus_handle, cx);
    }

    pub(super) fn new_project_action(
        &mut self,
        _: &NewProject,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.add_project(cx);
    }

    pub(super) fn open_settings_action(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.page = Some(SettingsPage::General);
        self.settings_ui.scroll.set_offset(gpui::Point::default());
        // Warm the Usage page's transcript scan while the user is still on
        // General, so clicking Usage lands on data instead of a spinner.
        self.ensure_usage_history(false, cx);
        window.focus(&self.settings_ui.focus, cx);
        cx.notify();
    }

    pub(super) fn toggle_sidebar_action(
        &mut self,
        _: &ToggleSidebar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_sidebar_visible(!self.shell_ui.sidebar_visible, cx);
    }

    pub(super) fn toggle_right_panel_action(
        &mut self,
        _: &ToggleRightPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_right_panel_visible(!self.shell_ui.right_panel_visible, cx);
    }

    pub(super) fn toggle_fps_counter_action(
        &mut self,
        _: &ToggleFpsCounter,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shell_ui.fps_counter_visible = !self.shell_ui.fps_counter_visible;
        cx.notify();
    }

    pub(super) fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.shell_ui.sidebar_visible == visible {
            return;
        }
        self.shell_ui.sidebar_visible = visible;
        self.shell_ui.sidebar_slide =
            self.begin_panel_slide(self.shell_ui.sidebar_rendered_width, cx);
        self.persist_panel_layout();
        cx.notify();
    }

    /// A toggle's slide, starting from the width the panel currently occupies
    /// so an interrupted one reverses from where its edge actually is.
    /// Reduce-motion gets `None`: the panel simply appears at its new width,
    /// and no frames are scheduled for it.
    fn begin_panel_slide(&self, from: f32, cx: &App) -> Option<motion::WidthTween> {
        (!cx.reduce_motion()).then(|| motion::WidthTween::new(from))
    }

    pub(super) fn set_right_panel_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if visible {
            self.request_active_terminal_focus();
        } else {
            self.right_panel_ui.pending_terminal_focus = None;
        }
        if self.shell_ui.right_panel_visible == visible {
            return;
        }
        self.shell_ui.right_panel_visible = visible;
        self.shell_ui.right_panel_slide =
            self.begin_panel_slide(self.shell_ui.right_panel_rendered_width, cx);
        self.persist_panel_layout();
        cx.notify();
    }

    pub(super) fn persist_panel_layout(&mut self) {
        self.state.sidebar_visible = self.shell_ui.sidebar_visible;
        self.state.right_panel_visible = self.shell_ui.right_panel_visible;
        self.state.sidebar_width = self.shell_ui.sidebar_width;
        self.state.right_panel_width = self.shell_ui.right_panel_width;
        self.save();
    }

    /// Mirror the live window frame into persisted state; disk waits for the
    /// app-quit save (any other `save` carries the frame along for free).
    /// macOS reports a zoomed window as `Windowed` with screen-filling bounds,
    /// so while maximized (and while fullscreen) the last floating frame is
    /// kept as the restore size — and the display it was captured on — and
    /// only the flag advances.
    pub(super) fn capture_window_state(&mut self, window: &Window, cx: &App) {
        // Bounds also change when the OS relocates the window — a monitor
        // unplugged, a display asleep. Those moves are not the user's; like
        // Zed, only capture while the window is the active one.
        if !window.is_window_active() {
            return;
        }
        let previous = self.state.window_state;
        let display = window.display(cx).and_then(|display| display.uuid().ok());
        self.state.window_state = Some(match window.window_bounds() {
            WindowBounds::Fullscreen(restore) => {
                previous.unwrap_or_else(|| persisted_window_state(restore, false, display))
            }
            WindowBounds::Maximized(restore) => persisted_window_state(restore, true, display),
            WindowBounds::Windowed(bounds) if window.is_maximized() => PersistedWindowState {
                maximized: true,
                ..previous.unwrap_or_else(|| persisted_window_state(bounds, true, display))
            },
            WindowBounds::Windowed(bounds) => persisted_window_state(bounds, false, display),
        });
    }

    /// The width each panel lays its content out at. A panel mid-slide counts
    /// as on screen and keeps its full width here: the slide narrows the
    /// container that clips it, so nothing inside reflows on the way out.
    /// What the panel actually occupies this frame is
    /// [`Michelle::sidebar_rendered_width`] / [`Michelle::right_panel_rendered_width`].
    pub(super) fn effective_panel_widths(&self, window: &Window) -> (f32, f32) {
        fitted_panel_widths(
            f32::from(window.viewport_size().width),
            self.shell_ui.sidebar_visible || self.shell_ui.sidebar_slide.is_some(),
            self.shell_ui.right_panel_visible || self.shell_ui.right_panel_slide.is_some(),
            self.shell_ui.sidebar_width,
            self.shell_ui.right_panel_width,
        )
    }

    pub(super) fn begin_panel_resize(
        &mut self,
        target: PanelResizeTarget,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (sidebar_width, right_panel_width) = self.effective_panel_widths(window);
        // A drag tracks the pointer directly; whatever slide was still
        // finishing would fight it for the same edge.
        let start_width = match target {
            PanelResizeTarget::Sidebar => {
                self.shell_ui.sidebar_slide = None;
                self.shell_ui.sidebar_width = sidebar_width;
                crate::platform::set_sidebar_material_width(window, sidebar_width);
                sidebar_width
            }
            PanelResizeTarget::RightPanel => {
                self.shell_ui.right_panel_slide = None;
                self.shell_ui.right_panel_width = right_panel_width;
                right_panel_width
            }
            PanelResizeTarget::FileTree => {
                let width =
                    fitted_file_tree_width(right_panel_width, self.right_panel_ui.file_tree_width);
                self.right_panel_ui.file_tree_width = width;
                width
            }
        };
        self.shell_ui.panel_resize_drag = Some(PanelResizeDrag {
            target,
            start_mouse_x: f32::from(event.position.x),
            start_width,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn resize_panel_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.shell_ui.panel_resize_drag else {
            return;
        };
        let viewport_width = f32::from(window.viewport_size().width);
        let (sidebar_width, right_panel_width) = self.effective_panel_widths(window);
        let delta = f32::from(event.position.x) - drag.start_mouse_x;
        match drag.target {
            PanelResizeTarget::Sidebar => {
                let maximum = SIDEBAR_MAX_WIDTH
                    .min(viewport_width - MAIN_PANEL_MIN_WIDTH - right_panel_width)
                    .max(SIDEBAR_MIN_WIDTH);
                let width = (drag.start_width + delta).clamp(SIDEBAR_MIN_WIDTH, maximum);
                if (self.shell_ui.sidebar_width - width).abs() < 0.5 {
                    return;
                }
                self.shell_ui.sidebar_width = width;
                crate::platform::set_sidebar_material_width(window, width);
            }
            PanelResizeTarget::RightPanel => {
                let maximum = RIGHT_PANEL_MAX_WIDTH
                    .min(viewport_width - MAIN_PANEL_MIN_WIDTH - sidebar_width)
                    .max(RIGHT_PANEL_MIN_WIDTH);
                let width = (drag.start_width - delta).clamp(RIGHT_PANEL_MIN_WIDTH, maximum);
                if (self.shell_ui.right_panel_width - width).abs() < 0.5 {
                    return;
                }
                self.shell_ui.right_panel_width = width;
            }
            PanelResizeTarget::FileTree => {
                let maximum = FILE_TREE_MAX_WIDTH
                    .min(right_panel_width - FILE_EDITOR_MIN_WIDTH)
                    .max(FILE_TREE_MIN_WIDTH);
                let width = (drag.start_width - delta).clamp(FILE_TREE_MIN_WIDTH, maximum);
                if (self.right_panel_ui.file_tree_width - width).abs() < 0.5 {
                    return;
                }
                self.right_panel_ui.file_tree_width = width;
            }
        }
        cx.notify();
    }

    pub(super) fn finish_panel_resize(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Left
            && let Some(drag) = self.shell_ui.panel_resize_drag.take()
        {
            if drag.target != PanelResizeTarget::FileTree {
                self.persist_panel_layout();
            }
            cx.notify();
        }
    }

    pub(super) fn navigate_back_action(
        &mut self,
        _: &NavigateBack,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.take().is_some() {
            let focus_handle = self.composer_focus(cx);
            window.focus(&focus_handle, cx);
            cx.notify();
            return;
        }

        let Some(current) = self.state.selected_session else {
            return;
        };
        if let Some(target) = self.sessions.activation.navigation.back_target() {
            self.settings_ui.page = None;
            self.request_session_activation(
                target,
                SessionActivationTransition::Back { from: current },
                cx,
            );
        }
    }

    pub(super) fn navigate_forward_action(
        &mut self,
        _: &NavigateForward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.is_some() {
            return;
        }

        let Some(current) = self.state.selected_session else {
            return;
        };
        if let Some(target) = self.sessions.activation.navigation.forward_target() {
            self.settings_ui.page = None;
            self.request_session_activation(
                target,
                SessionActivationTransition::Forward { from: current },
                cx,
            );
        }
    }

    pub(super) fn navigation_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.button {
            MouseButton::Navigate(NavigationDirection::Back) => {
                cx.stop_propagation();
                self.navigate_back_action(&NavigateBack, window, cx);
            }
            MouseButton::Navigate(NavigationDirection::Forward) => {
                cx.stop_propagation();
                self.navigate_forward_action(&NavigateForward, window, cx);
            }
            _ => {}
        }
    }

    pub(super) fn focus_composer_action(
        &mut self,
        _: &FocusComposer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.page = None;
        let focus_handle = self.composer_focus(cx);
        window.focus(&focus_handle, cx);
        cx.notify();
    }

    pub(super) fn cancel_turn_action(
        &mut self,
        _: &CancelTurn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The switcher focus lands after its deferred overlay is painted.
        // Route the root Escape action here too so an immediate press always
        // cancels the provisional selection instead of reaching the session.
        if self.task_switcher.is_open() {
            self.cancel_task_switcher(window, cx);
            return;
        }
        if self.settings_ui.page.take().is_some() {
            let focus_handle = self.composer_focus(cx);
            window.focus(&focus_handle, cx);
            cx.notify();
            return;
        }
        if self.transcript_ui.message_edit.is_some() {
            self.cancel_message_edit(window, cx);
            return;
        }
        let Some(target) = self.selected_escape_stop_target() else {
            self.cancel_turn(cx);
            return;
        };
        match self
            .session_ui
            .escape_stop_confirmation
            .press(target, Instant::now())
        {
            EscapeStopPress::Stop => self.cancel_turn(cx),
            EscapeStopPress::Arm(arm) => {
                cx.notify();
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(ESCAPE_STOP_CONFIRMATION_TIMEOUT)
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if this.session_ui.escape_stop_confirmation.expire(arm) {
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
    }

    fn selected_escape_stop_target(&self) -> Option<EscapeStopTarget> {
        let session = self.selected_session()?;
        (!self
            .sessions
            .runtime
            .submission_preparations
            .contains(&session.id)
            && session.status.is_busy())
        .then(|| EscapeStopTarget::for_session(session))
    }

    pub(super) fn reset_visible_state(&mut self) {
        self.transcript_ui.activities_expanded.clear();
        self.transcript_ui.expanded_activity_items.clear();
        self.transcript_ui.expanded_turns.clear();
        self.transcript_ui.expanded_changed_files.clear();
        self.transcript_ui.control_focuses.borrow_mut().clear();
        self.transcript_ui
            .user_message_viewports
            .borrow_mut()
            .clear();
        self.transcript_ui.hovered_response_row = None;
        // Selection belongs to the session being left.
        self.transcript_ui.selection.selection.borrow_mut().clear();
        self.transcript_ui.selection.registry.borrow_mut().clear();
        self.reset_transcript_search_for_session();
        let (streaming_messages, live_reasoning) = self.selected_session().map_or_else(
            || (Vec::new(), Vec::new()),
            |session| {
                let messages = session
                    .messages
                    .iter()
                    .filter(|message| message.role == MessageRole::Assistant && message.streaming)
                    .map(|message| message.id)
                    .collect();
                let reasoning = session
                    .transcript_blocks
                    .iter()
                    .flat_map(|block| &block.activities)
                    .filter(|activity| activity.reasoning.is_some() && !activity.complete)
                    .map(|activity| activity.id)
                    .collect();
                (messages, reasoning)
            },
        );
        // Parsed messages are keyed by message id, which is unique across
        // sessions, so they stay cached — switching back to a recent session
        // then costs no re-parse. Bounded so a long-running window cannot grow
        // without limit.
        let mut message_markdown = self.transcript_model.message_markdown.borrow_mut();
        let cached_bytes: usize = message_markdown
            .values()
            .map(md::render::MarkdownView::source_len)
            .sum();
        if cached_bytes > MAX_CACHED_MESSAGE_SOURCE_BYTES {
            message_markdown.clear();
        }
        for id in streaming_messages {
            message_markdown
                .entry(id)
                .or_insert_with(MarkdownView::new)
                .seed_streaming_baseline();
        }
        drop(message_markdown);
        // Block parses are keyed by position within the session, so they would
        // be read as another session's blocks.
        let mut activity_markdown = self.transcript_model.activity_markdown.borrow_mut();
        activity_markdown.clear();
        for id in live_reasoning {
            activity_markdown.insert(id, MarkdownView::seeded());
        }
        drop(activity_markdown);
        self.transcript_model
            .reasoning_window_starts
            .borrow_mut()
            .clear();
        self.transcript_ui
            .activity_scroll_viewports
            .borrow_mut()
            .clear();
        self.shell_ui.menus.borrow_mut().clear();
        self.transcript_ui.message_edit = None;
        self.hide_toast();
        self.transcript_ui.navigation_rail_reset_generation.set(
            self.transcript_ui
                .navigation_rail_reset_generation
                .get()
                .wrapping_add(1),
        );
        self.transcript_ui.anchor.set(None);
        self.transcript_ui.anchor_end_space.set(Pixels::ZERO);
        self.transcript_ui.anchor_following.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_carries_the_current_tasks_access_mode() {
        let mut current = AgentSession::new(Uuid::new_v4(), ProviderKind::OpenCode);
        current.runtime_mode = RuntimeMode::Ask;

        assert_eq!(
            new_task_runtime_mode(Some(&current), RuntimeMode::FullAccess),
            RuntimeMode::Ask
        );
        assert_eq!(
            new_task_runtime_mode(None, RuntimeMode::AutoAcceptEdits),
            RuntimeMode::AutoAcceptEdits
        );
    }

    #[test]
    fn new_task_navigation_reuses_a_draft_from_the_current_project() {
        let project_id = Uuid::new_v4();
        let draft = AgentSession::new(project_id, ProviderKind::Codex);
        let mut started = AgentSession::new(project_id, ProviderKind::Claude);
        started.begin_turn("Existing task");
        let mut navigation = SessionNavigation::default();

        navigation.remember_new_task(draft.id);
        navigation.visit(Some(draft.id), started.id);

        assert_eq!(
            navigation.remembered_new_task(&[draft.clone(), started], project_id),
            Some(draft.id)
        );
    }

    #[test]
    fn new_task_navigation_does_not_reopen_a_draft_from_another_project() {
        let draft = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        let current_project_id = Uuid::new_v4();
        let mut navigation = SessionNavigation::default();

        navigation.remember_new_task(draft.id);

        assert_eq!(
            navigation.remembered_new_task(&[draft], current_project_id),
            None
        );
    }

    #[test]
    fn new_task_navigation_does_not_reopen_a_started_or_removed_draft() {
        let project_id = Uuid::new_v4();
        let mut draft = AgentSession::new(project_id, ProviderKind::Codex);
        let mut navigation = SessionNavigation::default();
        navigation.remember_new_task(draft.id);

        draft.begin_turn("Start it");
        assert_eq!(
            navigation.remembered_new_task(&[draft.clone()], project_id),
            None
        );

        navigation.remove(draft.id);
        assert_eq!(navigation.new_task, None);
    }

    #[test]
    fn stopping_releases_the_runtimes_that_cannot_be_interrupted_in_place() {
        // Codex owns a Computer Use process tree; Amp has no stream interrupt.
        assert!(!retain_runtime_after_cancel(ProviderKind::Codex));
        assert!(!retain_runtime_after_cancel(ProviderKind::Amp));
        for provider in ProviderKind::ALL {
            if !matches!(provider, ProviderKind::Codex | ProviderKind::Amp) {
                assert!(retain_runtime_after_cancel(provider));
            }
        }
    }
}
