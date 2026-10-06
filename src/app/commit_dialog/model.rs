//! Git commit preparation and operations. In-flight work outlives the dialog.
use super::*;

#[derive(Default)]
pub(in crate::app) struct CommitModel {
    pub(super) draft: Option<CommitDraft>,
    pub(super) operation: Option<CommitOperationState>,
}

impl CommitModel {
    pub(super) fn dismiss_draft(&mut self) {
        self.draft = None;
    }

    pub(super) fn clear_error(&mut self) {
        if let Some(draft) = self.draft.as_mut() {
            draft.error = None;
        }
    }

    fn is_current(&self, id: Uuid, workspace: &Path, pending: CommitPending) -> bool {
        self.operation.as_ref().is_some_and(|operation| {
            operation.id == id && operation.workspace == workspace && operation.pending == pending
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitAction {
    Commit,
    CommitAndPush,
    Push,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitPending {
    Generating(CommitAction),
    Git(CommitAction),
}

pub(super) struct CommitOperationState {
    pub(super) id: Uuid,
    pub(super) workspace: PathBuf,
    pub(super) pending: CommitPending,
}

impl CommitOperationState {
    pub(super) fn status_label(&self) -> String {
        commit_pending_status_label(self.pending)
    }
}

pub(super) fn commit_pending_status_label(pending: CommitPending) -> String {
    match pending {
        CommitPending::Generating(_) => tr!("commit.generating_message"),
        CommitPending::Git(CommitAction::Commit) => tr!("commit.committing"),
        CommitPending::Git(CommitAction::CommitAndPush) => {
            tr!("commit.committing_and_pushing")
        }
        CommitPending::Git(CommitAction::Push) => tr!("commit.pushing"),
    }
}

pub(super) struct CommitDraft {
    pub(super) id: Uuid,
    pub(super) workspace: PathBuf,
    invocation: Option<crate::git_commit::AgentInvocation>,
    pub(super) snapshot: crate::git_commit::Snapshot,
    pub(super) snapshot_loading: bool,
    pub(super) error: Option<String>,
}

impl CommitDraft {
    pub(super) fn can_commit(&self, include_unstaged: bool) -> bool {
        !self.snapshot_loading
            && (self.snapshot.has_staged || (include_unstaged && self.snapshot.has_unstaged))
    }
    pub(super) fn can_push(&self) -> bool {
        !self.snapshot_loading && self.snapshot.can_push
    }
    pub(super) fn displayed_counts(&self, include_unstaged: bool) -> (u64, u64) {
        if include_unstaged {
            (self.snapshot.additions, self.snapshot.deletions)
        } else {
            (
                self.snapshot.staged_additions,
                self.snapshot.staged_deletions,
            )
        }
    }
}

impl Michelle {
    pub(super) fn prepare_commit_draft(&mut self, cx: &mut Context<Self>) -> Option<Uuid> {
        if self.commit.operation.is_some() {
            return None;
        }
        let Some((workspace, provider, model, reasoning_effort)) =
            self.selected_session().and_then(|session| {
                Some((
                    self.workspace_path_for_session(session)?.to_path_buf(),
                    session.provider,
                    self.model_for_session(session).map(str::to_owned),
                    session.reasoning_effort.clone(),
                ))
            })
        else {
            self.show_toast(tr!("commit.no_task"));
            cx.notify();
            return None;
        };

        let invocation = self
            .provider_probe(provider)
            .and_then(|probe| probe.path.clone())
            .map(|binary| crate::git_commit::AgentInvocation {
                provider,
                binary,
                model,
                reasoning_effort,
            });
        let cached_branch = self
            .branches
            .visible_snapshot
            .as_ref()
            .filter(|(path, _)| path == &workspace)
            .map(|(_, snapshot)| snapshot);
        let snapshot = crate::git_commit::Snapshot {
            branch: cached_branch
                .and_then(|snapshot| snapshot.display_branch())
                .unwrap_or("HEAD")
                .to_owned(),
            additions: cached_branch.map_or(0, |snapshot| snapshot.additions),
            deletions: cached_branch.map_or(0, |snapshot| snapshot.deletions),
            ..Default::default()
        };
        let id = Uuid::new_v4();
        self.commit.draft = Some(CommitDraft {
            id,
            workspace,
            invocation,
            snapshot,
            snapshot_loading: true,
            error: None,
        });
        Some(id)
    }
    pub(super) fn load_commit_snapshot(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let Some(draft) = self.commit.draft.as_ref().filter(|draft| draft.id == id) else {
            return;
        };
        let workspace = draft.workspace.clone();
        let workspace_client = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match workspace_client.request(
                        michelle_client::WorkspaceOperation::InspectCommit {
                            cwd: workspace.clone(),
                        },
                    ) {
                        Ok(michelle_client::WorkspaceResult::CommitSnapshot { snapshot }) => {
                            Ok(snapshot)
                        }
                        Ok(_) => Err("the daemon returned an invalid Git response".to_owned()),
                        Err(error) => Err(error.to_string()),
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                let Some(dialog) = michelle
                    .commit
                    .draft
                    .as_mut()
                    .filter(|dialog| dialog.id == id)
                else {
                    return;
                };
                dialog.snapshot_loading = false;
                match result {
                    Ok(snapshot) => dialog.snapshot = snapshot,
                    Err(error) => dialog.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn perform_commit_action(
        &mut self,
        action: CommitAction,
        message: String,
        include_unstaged: bool,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.commit.operation.is_some() {
            return;
        }
        let Some(dialog) = self.commit.draft.as_ref() else {
            return;
        };
        let enabled = match action {
            CommitAction::Commit | CommitAction::CommitAndPush => {
                dialog.can_commit(include_unstaged)
            }
            CommitAction::Push => dialog.can_push(),
        };
        if !enabled {
            return;
        }

        let id = dialog.id;
        let workspace = dialog.workspace.clone();
        let invocation = dialog.invocation.clone();

        if action != CommitAction::Push && message.is_empty() {
            let Some(invocation) = invocation else {
                if let Some(dialog) = self.commit.draft.as_mut() {
                    dialog.error = Some(tr!("commit.agent_unavailable"));
                }
                cx.notify();
                return;
            };
            if let Some(draft) = self.commit.draft.as_mut() {
                draft.error = None;
            }
            if let Some(dialog) = self.commit_dialog.as_mut() {
                dialog
                    .message
                    .update(cx, |message, _| message.set_read_only(true));
            }
            self.commit.operation = Some(CommitOperationState {
                id,
                workspace: workspace.clone(),
                pending: CommitPending::Generating(action),
            });
            self.spawn_commit_message_generation(
                id,
                action,
                workspace,
                include_unstaged,
                invocation,
                window_handle,
                cx,
            );
            cx.notify();
            return;
        }

        if let Some(draft) = self.commit.draft.as_mut() {
            draft.error = None;
        }
        if let Some(dialog) = self.commit_dialog.as_mut() {
            dialog
                .message
                .update(cx, |message, _| message.set_read_only(true));
        }
        self.commit.operation = Some(CommitOperationState {
            id,
            workspace: workspace.clone(),
            pending: CommitPending::Git(action),
        });
        self.spawn_git_action(
            id,
            action,
            workspace,
            message,
            include_unstaged,
            window_handle,
            cx,
        );
        cx.notify();
    }

    fn spawn_commit_message_generation(
        &mut self,
        id: Uuid,
        action: CommitAction,
        workspace: PathBuf,
        include_unstaged: bool,
        invocation: crate::git_commit::AgentInvocation,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let workspace_client = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let generation_workspace = workspace.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    match workspace_client.request(
                        michelle_client::WorkspaceOperation::GenerateCommitMessage {
                            cwd: generation_workspace,
                            include_unstaged,
                            invocation,
                        },
                    ) {
                        Ok(michelle_client::WorkspaceResult::CommitMessage { message }) => {
                            Ok(message)
                        }
                        Ok(_) => {
                            Err("the daemon returned an invalid commit message response".into())
                        }
                        Err(error) => Err(error.to_string()),
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                let current =
                    michelle
                        .commit
                        .is_current(id, &workspace, CommitPending::Generating(action));
                if !current {
                    return;
                }
                match result {
                    Ok(message) => {
                        if let Some(operation) = michelle.commit.operation.as_mut() {
                            operation.pending = CommitPending::Git(action);
                        }
                        if let Some(dialog) = michelle
                            .commit_dialog
                            .as_mut()
                            .filter(|dialog| dialog.id == id)
                        {
                            dialog
                                .message
                                .update(cx, |input, cx| input.set_content(message.clone(), cx));
                        }
                        michelle.spawn_git_action(
                            id,
                            action,
                            workspace,
                            message,
                            include_unstaged,
                            window_handle,
                            cx,
                        );
                    }
                    Err(error) => {
                        michelle.commit.operation = None;
                        if let Some(dialog) = michelle
                            .commit_dialog
                            .as_mut()
                            .filter(|dialog| dialog.id == id)
                        {
                            if let Some(draft) = michelle
                                .commit
                                .draft
                                .as_mut()
                                .filter(|draft| draft.id == id)
                            {
                                draft.error = Some(error);
                            }
                            dialog
                                .message
                                .update(cx, |message, _| message.set_read_only(false));
                        } else {
                            michelle.show_toast(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn spawn_git_action(
        &mut self,
        id: Uuid,
        action: CommitAction,
        workspace: PathBuf,
        message: String,
        include_unstaged: bool,
        window_handle: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let workspace_client = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let operation_workspace = workspace.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let operation = match action {
                        CommitAction::Commit => michelle_client::WorkspaceOperation::Commit {
                            cwd: operation_workspace.clone(),
                            message,
                            include_unstaged,
                            push: false,
                        },
                        CommitAction::CommitAndPush => {
                            michelle_client::WorkspaceOperation::Commit {
                                cwd: operation_workspace.clone(),
                                message,
                                include_unstaged,
                                push: true,
                            }
                        }
                        CommitAction::Push => michelle_client::WorkspaceOperation::Push {
                            cwd: operation_workspace.clone(),
                        },
                    };
                    let result = match workspace_client.request(operation) {
                        Ok(michelle_client::WorkspaceResult::Ack) => Ok(()),
                        Ok(_) => Err(anyhow::anyhow!(
                            "the daemon returned an invalid Git response"
                        )),
                        Err(error) => Err(error),
                    };
                    let snapshot = result.as_ref().err().and_then(|_| {
                        match workspace_client.request(
                            michelle_client::WorkspaceOperation::InspectCommit {
                                cwd: operation_workspace.clone(),
                            },
                        ) {
                            Ok(michelle_client::WorkspaceResult::CommitSnapshot { snapshot }) => {
                                Some(snapshot)
                            }
                            _ => None,
                        }
                    });
                    (result.map_err(|error| error.to_string()), snapshot)
                })
                .await;
            let focus = michelle.update(cx, |michelle, cx| {
                let current =
                    michelle
                        .commit
                        .is_current(id, &workspace, CommitPending::Git(action));
                if !current {
                    return None;
                }
                let (result, refreshed_snapshot) = result;
                michelle.commit.operation = None;
                if michelle
                    .selected_workspace_path()
                    .is_some_and(|path| path == workspace)
                {
                    michelle.invalidate_workspace_queries(cx);
                } else {
                    michelle.branches.snapshots.invalidate(&workspace);
                }
                let focus = match result {
                    Ok(()) => {
                        let dialog_was_open = michelle
                            .commit_dialog
                            .as_ref()
                            .is_some_and(|dialog| dialog.id == id);
                        if dialog_was_open {
                            michelle.commit_dialog = None;
                            michelle.commit.draft = None;
                        }
                        michelle.show_success_toast(match action {
                            CommitAction::Commit => tr!("commit.committed"),
                            CommitAction::CommitAndPush => tr!("commit.committed_and_pushed"),
                            CommitAction::Push => tr!("commit.pushed"),
                        });
                        dialog_was_open.then(|| michelle.composer_focus(cx))
                    }
                    Err(error) => {
                        if let Some(dialog) = michelle
                            .commit_dialog
                            .as_mut()
                            .filter(|dialog| dialog.id == id)
                        {
                            if let Some(draft) = michelle
                                .commit
                                .draft
                                .as_mut()
                                .filter(|draft| draft.id == id)
                            {
                                draft.error = Some(error);
                                if let Some(snapshot) = refreshed_snapshot {
                                    draft.snapshot = snapshot;
                                    draft.snapshot_loading = false;
                                }
                            }
                            dialog
                                .message
                                .update(cx, |message, _| message.set_read_only(false));
                        } else {
                            michelle.show_toast(error);
                        }
                        None
                    }
                };
                cx.notify();
                focus
            });
            if let Ok(Some(focus)) = focus {
                let _ = window_handle.update(cx, |_, window, cx| window.focus(&focus, cx));
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismissing_draft_preserves_operation_and_rejects_stale_completions() {
        let id = Uuid::new_v4();
        let workspace = PathBuf::from("/project");
        let mut model = CommitModel {
            draft: Some(CommitDraft {
                id,
                workspace: workspace.clone(),
                invocation: None,
                snapshot: Default::default(),
                snapshot_loading: false,
                error: None,
            }),
            operation: Some(CommitOperationState {
                id,
                workspace: workspace.clone(),
                pending: CommitPending::Generating(CommitAction::Commit),
            }),
        };
        model.dismiss_draft();
        assert!(model.draft.is_none());
        assert!(model.is_current(
            id,
            &workspace,
            CommitPending::Generating(CommitAction::Commit)
        ));
        assert!(!model.is_current(
            Uuid::new_v4(),
            &workspace,
            CommitPending::Generating(CommitAction::Commit)
        ));
        assert!(!model.is_current(
            id,
            Path::new("/other"),
            CommitPending::Generating(CommitAction::Commit)
        ));
        assert!(!model.is_current(id, &workspace, CommitPending::Git(CommitAction::Commit)));
        model.operation.as_mut().unwrap().pending = CommitPending::Git(CommitAction::Commit);
        assert!(model.is_current(id, &workspace, CommitPending::Git(CommitAction::Commit)));
    }

    #[test]
    fn commit_availability_uses_loaded_snapshot_and_selected_option() {
        let mut draft = CommitDraft {
            id: Uuid::new_v4(),
            workspace: PathBuf::new(),
            invocation: None,
            snapshot: crate::git_commit::Snapshot {
                has_unstaged: true,
                can_push: true,
                ..Default::default()
            },
            snapshot_loading: true,
            error: None,
        };
        assert!(!draft.can_commit(true));
        assert!(!draft.can_push());
        draft.snapshot_loading = false;
        assert!(draft.can_commit(true));
        assert!(!draft.can_commit(false));
        assert!(draft.can_push());
        draft.snapshot.has_staged = true;
        assert!(draft.can_commit(false));
    }
}
