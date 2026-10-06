//! Session activation, asynchronous hydration and navigation history.
use super::*;

pub(in crate::app) struct SessionActivation {
    pub(in crate::app) hydrations: HashSet<Uuid>,
    pub(in crate::app) pending: Option<PendingSessionActivation>,
    pub(in crate::app) navigation: SessionNavigation,
}

#[derive(Debug, Default)]
pub(in crate::app) struct SessionNavigation {
    back: Vec<Uuid>,
    forward: Vec<Uuid>,
    /// The most recently selected unstarted task. The global New Task entry
    /// may reuse it only when it belongs to the currently selected project.
    pub(in crate::app) new_task: Option<Uuid>,
}

impl SessionNavigation {
    pub(in crate::app) fn visit(&mut self, current: Option<Uuid>, next: Uuid) {
        if let Some(current) = current.filter(|current| *current != next) {
            self.back.push(current);
            self.forward.clear();
        }
    }

    pub(in crate::app) fn go_back(&mut self, current: Uuid) -> Option<Uuid> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }

    pub(in crate::app) fn back_target(&self) -> Option<Uuid> {
        self.back.last().copied()
    }

    pub(in crate::app) fn go_forward(&mut self, current: Uuid) -> Option<Uuid> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
    }

    pub(in crate::app) fn forward_target(&self) -> Option<Uuid> {
        self.forward.last().copied()
    }

    pub(in crate::app) fn remove(&mut self, session_id: Uuid) {
        self.back.retain(|entry| *entry != session_id);
        self.forward.retain(|entry| *entry != session_id);
        if self.new_task == Some(session_id) {
            self.new_task = None;
        }
    }

    pub(in crate::app) fn remember_new_task(&mut self, session_id: Uuid) {
        self.new_task = Some(session_id);
    }

    pub(in crate::app) fn remembered_new_task(
        &self,
        sessions: &[AgentSession],
        current_project_id: Uuid,
    ) -> Option<Uuid> {
        self.new_task.filter(|session_id| {
            sessions.iter().any(|session| {
                session.id == *session_id
                    && session.project_id == current_project_id
                    && !session.has_started()
            })
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum SessionActivationTransition {
    Visit,
    Back { from: Uuid },
    Forward { from: Uuid },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct PendingSessionActivation {
    pub(in crate::app) session_id: Uuid,
    pub(in crate::app) transition: SessionActivationTransition,
}

impl Michelle {
    pub(in crate::app) fn select_session(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        self.request_session_activation(session_id, SessionActivationTransition::Visit, cx);
    }

    pub(in crate::app) fn request_session_activation(
        &mut self,
        session_id: Uuid,
        transition: SessionActivationTransition,
        cx: &mut Context<Self>,
    ) {
        if !self
            .state
            .sessions
            .iter()
            .any(|session| session.id == session_id)
        {
            return;
        }
        self.reveal_sidebar_session(session_id);
        let needs_hydration = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .is_some_and(|session| !session.detail_loaded);
        if needs_hydration {
            self.sessions.activation.pending = Some(PendingSessionActivation {
                session_id,
                transition,
            });
            // Keep the current transcript visible until the daemon returns the
            // target session, but acknowledge the click immediately in the
            // sidebar instead of making the UI appear unresponsive.
            cx.notify();
            self.ensure_session_loaded(session_id, cx);
            return;
        }
        self.sessions.activation.pending = None;
        self.finish_session_activation(session_id, transition, cx);
    }

    fn finish_session_activation(
        &mut self,
        session_id: Uuid,
        transition: SessionActivationTransition,
        cx: &mut Context<Self>,
    ) {
        match transition {
            SessionActivationTransition::Visit => self
                .sessions
                .activation
                .navigation
                .visit(self.state.selected_session, session_id),
            SessionActivationTransition::Back { from } => {
                if self.state.selected_session != Some(from)
                    || self.sessions.activation.navigation.back_target() != Some(session_id)
                {
                    return;
                }
                let _ = self.sessions.activation.navigation.go_back(from);
            }
            SessionActivationTransition::Forward { from } => {
                if self.state.selected_session != Some(from)
                    || self.sessions.activation.navigation.forward_target() != Some(session_id)
                {
                    return;
                }
                let _ = self.sessions.activation.navigation.go_forward(from);
            }
        }
        self.activate_session(session_id, cx);
    }

    /// Loads a session's transcript if startup only fetched its list columns.
    ///
    /// The SQLite query and daemon round trip both stay off the UI thread. The
    /// current selection stays rendered until the requested session is whole.
    fn ensure_session_loaded(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        let needs_hydration = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .is_some_and(|session| !session.detail_loaded);
        if !needs_hydration || !self.sessions.activation.hydrations.insert(session_id) {
            return;
        }
        let daemon = self.daemon.clone();
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match michelle_client::persistence::hydrate_session(&daemon, session_id)? {
                        Some(session) => Ok(session),
                        None => {
                            anyhow::bail!("the task no longer exists")
                        }
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                if !michelle.sessions.activation.hydrations.remove(&session_id) {
                    return;
                }
                match result {
                    Ok(session) => {
                        let replaced = if let Some(existing) = michelle
                            .state
                            .sessions
                            .iter_mut()
                            .find(|existing| existing.id == session_id)
                        {
                            *existing = session;
                            true
                        } else {
                            false
                        };
                        let pending = michelle
                            .sessions
                            .activation
                            .pending
                            .filter(|pending| pending.session_id == session_id);
                        if pending.is_some() {
                            michelle.sessions.activation.pending = None;
                        }
                        if replaced && let Some(pending) = pending {
                            michelle.finish_session_activation(session_id, pending.transition, cx);
                        } else if michelle.state.selected_session == Some(session_id) {
                            michelle.reset_visible_state();
                            michelle.reset_transcript_rows(michelle.transcript_row_count());
                            michelle.refresh_composer_sources(cx);
                        }
                    }
                    Err(error) => {
                        if michelle
                            .sessions
                            .activation
                            .pending
                            .is_some_and(|pending| pending.session_id == session_id)
                        {
                            michelle.sessions.activation.pending = None;
                        }
                        michelle.show_toast(tr!("errors.open_session", error = error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn activate_session(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        let session_changed = self.state.selected_session != Some(session_id);
        if session_changed {
            self.capture_and_save_current_composer_draft(cx);
            self.store_selected_right_panel_state();
        }
        self.state.selected_session = Some(session_id);
        self.task_switcher.record_access(session_id);
        if let Some((
            project_id,
            provider,
            runtime_mode,
            model,
            reasoning_effort,
            service_tier,
            context_window,
        )) = self.selected_session().map(|session| {
            (
                session.project_id,
                session.provider,
                session.runtime_mode,
                session.model.clone(),
                session.reasoning_effort.clone(),
                session.service_tier.clone(),
                session.context_window.clone(),
            )
        }) {
            self.state.selected_project = Some(project_id);
            self.state.last_provider = provider;
            self.state.last_runtime_mode = runtime_mode;
            self.state.last_model = model;
            self.state.last_reasoning_effort = reasoning_effort;
            self.state.last_service_tier = service_tier;
            self.state.last_context_window = context_window;
        }
        if self
            .selected_session()
            .is_some_and(|session| !session.has_started())
        {
            self.sessions
                .activation
                .navigation
                .remember_new_task(session_id);
        }
        if session_changed {
            self.restore_selected_composer_draft(cx);
            self.sync_user_input_answer(cx);
            self.restore_right_panel_state(session_id, cx);
        } else {
            self.ensure_right_panel_terminals(cx);
        }
        self.reset_visible_state();
        if session_changed {
            // Each materialized worktree has its own cache entry. A task that
            // finished while another session was selected could otherwise
            // retain the clean snapshot captured before its agent made edits.
            self.refresh_selected_branch_snapshot(cx);
        }
        self.refresh_composer_sources(cx);
        self.reset_transcript_rows(self.transcript_row_count());
        self.save();
        if self
            .selected_session()
            .is_some_and(AgentSession::has_started)
        {
            self.start_runtime_attachment(session_id, cx);
        }
        cx.notify();
    }
}
