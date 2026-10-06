//! Session scenario owners. Persisted projects and sessions stay on Michelle.
use super::*;
pub(in crate::app) struct SessionModel {
    pub(in crate::app) activation: activation::SessionActivation,
    pub(in crate::app) runtime: runtime::model::RuntimeModel,
    pub(in crate::app) checkpoints: checkpoints::CheckpointModel,
}

impl Michelle {
    pub(in crate::app) fn select_project(&mut self, project_id: Uuid, cx: &mut Context<Self>) {
        self.state.selected_project = Some(project_id);
        self.create_session_for(project_id, self.state.last_provider, cx);
    }

    pub(in crate::app) fn create_session_for(
        &mut self,
        project_id: Uuid,
        provider: ProviderKind,
        cx: &mut Context<Self>,
    ) {
        if let Some(draft_id) = self
            .state
            .sessions
            .iter()
            .find(|session| session.project_id == project_id && !session.has_started())
            .map(|session| session.id)
        {
            self.select_session(draft_id, cx);
            return;
        }
        // A task opened from the current task carries its working access mode.
        // `last_runtime_mode` covers launch and the few creation paths without
        // a selected source task.
        let runtime_mode =
            new_task_runtime_mode(self.selected_session(), self.state.last_runtime_mode);
        let mut session = self.state.new_session(project_id, provider);
        session.runtime_mode = runtime_mode;
        let id = session.id;
        self.state.push_session(session);
        self.select_session(id, cx);
    }

    pub(in crate::app) fn select_workspace(
        &mut self,
        workspace: SessionWorkspace,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.selected_session_mut() else {
            return;
        };
        if session.has_started() || session.is_busy() || session.workspace == workspace {
            return;
        }
        session.workspace = workspace;
        self.save();
        cx.notify();
    }

    pub(in crate::app) fn remove_session(&mut self, session_id: Uuid, cx: &mut Context<Self>) {
        if self
            .sessions
            .runtime
            .response_fork_preparations
            .contains_key(&session_id)
        {
            self.show_toast(tr!("session.response_fork_in_progress"));
            cx.notify();
            return;
        }
        let Some(index) = self
            .state
            .sessions
            .iter()
            .position(|session| session.id == session_id)
        else {
            return;
        };
        let project_id = self.state.sessions[index].project_id;
        let composer_draft_key =
            crate::persistence::ComposerDraftKey::for_session(&self.state.sessions[index]);
        let projectless = self
            .state
            .projects
            .iter()
            .find(|project| project.id == project_id)
            .is_some_and(Project::is_projectless);
        let project_path = self
            .workspace_path_for_session(&self.state.sessions[index])
            .map(std::path::Path::to_path_buf);
        let was_selected = self.state.selected_session == Some(session_id);
        self.sessions
            .runtime
            .submission_preparations
            .remove(&session_id);
        self.goal_model.runtime_starts.remove(&session_id);
        self.goal_model.pending_operations.remove(&session_id);
        self.goal_model.observed_at.remove(&session_id);
        self.reset_session_runtime(session_id);
        self.sessions.runtime.background_work.remove(&session_id);
        self.remove_right_panel_session_state(session_id);
        self.remove_composer_draft(composer_draft_key, cx);
        self.state.sessions.remove(index);
        if let Err(error) = self.store.remove_session(session_id) {
            self.show_toast(tr!("errors.save_local_state", error = error));
        }
        if self
            .sessions
            .activation
            .pending
            .is_some_and(|pending| pending.session_id == session_id)
        {
            self.sessions.activation.pending = None;
        }
        self.sessions.activation.navigation.remove(session_id);
        self.task_switcher.remove(session_id);
        let project_still_used = self
            .state
            .sessions
            .iter()
            .any(|session| session.project_id == project_id);
        if projectless && !project_still_used {
            self.remove_composer_draft(
                crate::persistence::ComposerDraftKey::NewSession(project_id),
                cx,
            );
            self.state
                .projects
                .retain(|project| project.id != project_id);
            if self.state.selected_project == Some(project_id) {
                self.state.selected_project = None;
            }
        }
        if let Some(project_path) = project_path {
            let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
            cx.background_executor()
                .spawn(async move {
                    let _ =
                        workspace.request(michelle_client::WorkspaceOperation::DeleteSessionRefs {
                            cwd: project_path,
                            session_id,
                        });
                })
                .detach();
        }
        self.invalidate_checkpoint_refs();

        if was_selected {
            self.state.selected_session = None;
            let next_session = self
                .state
                .sessions
                .iter()
                .filter(|session| session.project_id == project_id)
                .max_by_key(|session| session.updated_at)
                .map(|session| session.id);
            if let Some(session_id) = next_session {
                self.select_session(session_id, cx);
            } else if projectless {
                self.create_projectless_session(cx);
            } else {
                self.create_session_for(project_id, self.state.last_provider, cx);
            }
        } else {
            self.save();
            cx.notify();
        }

        // Only now is the row gone, so the sweep can see which blobs are
        // genuinely unreferenced. It reads the database and walks the blob
        // directory, so it stays off the UI thread.
        let sweep = self.store.blob_sweep();
        cx.background_executor()
            .spawn(async move { sweep() })
            .detach();
    }

    pub(in crate::app) fn reset_session_runtime(&mut self, session_id: Uuid) {
        if let Some(runtime) = self.sessions.runtime.runtimes.remove(&session_id) {
            runtime.driver.cancel();
            runtime.driver.close();
            self.mark_background_work_lost(session_id);
        }
    }

    pub(in crate::app) fn add_project(&mut self, cx: &mut Context<Self>) {
        if self.daemon.is_remote() {
            self.show_toast(tr!("errors.remote_project_picker"));
            cx.notify();
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("project.add_project").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if let Some(existing) = this.state.projects.iter().find(|p| p.path == path) {
                        this.select_project(existing.id, cx);
                        return;
                    }
                    let project = Project::from_path(path);
                    let project_id = project.id;
                    this.state.projects.push(project);
                    this.create_session_for(project_id, this.state.last_provider, cx);
                });
            }
        })
        .detach();
    }

    pub(in crate::app) fn create_projectless_session(&mut self, cx: &mut Context<Self>) {
        if let Some(draft_id) = self
            .state
            .sessions
            .iter()
            .find(|session| {
                !session.has_started()
                    && self.state.projects.iter().any(|project| {
                        project.id == session.project_id
                            && project.is_projectless()
                            && !crate::projectless::is_legacy_root_path(&project.path)
                    })
            })
            .map(|session| session.id)
        {
            self.select_session(draft_id, cx);
            return;
        }

        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match workspace.request(
                        michelle_client::WorkspaceOperation::CreateProjectlessWorkspace {
                            prompt: None,
                        },
                    )? {
                        michelle_client::WorkspaceResult::ProjectlessWorkspace { cwd } => Ok(cwd),
                        _ => anyhow::bail!("the daemon returned an invalid projectless response"),
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| match result {
                Ok(cwd) => {
                    let mut project = Project::from_path(cwd);
                    project.name = Project::PROJECTLESS_NAME.to_owned();
                    let project_id = project.id;
                    michelle.state.projects.push(project);
                    michelle.create_session_for(project_id, michelle.state.last_provider, cx);
                }
                Err(error) => {
                    michelle.show_toast(tr!("errors.create_projectless_task", error = error));
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
