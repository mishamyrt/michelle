//! Workspace branch snapshots and background Git operations.
use super::*;

enum BranchOperation {
    Checkout(String),
    Create(String),
}

pub(in crate::app) struct BranchModel {
    pub(in crate::app) snapshots: QueryCache<PathBuf, Result<Option<BranchSnapshot>, String>>,
    pub(in crate::app) visible_snapshot: Option<(PathBuf, BranchSnapshot)>,
    pub(in crate::app) operation_pending: bool,
}

impl BranchModel {
    pub(in crate::app) fn new() -> Self {
        Self {
            snapshots: QueryCache::new(MAX_CACHED_WORKSPACES),
            visible_snapshot: None,
            operation_pending: false,
        }
    }
}

impl Michelle {
    pub(super) fn can_begin_branch_creation(&self) -> bool {
        !self.branches.operation_pending
            && self.selected_session().is_some_and(|session| {
                !session.is_busy()
                    && !matches!(session.workspace, SessionWorkspace::NewWorktree { .. })
            })
    }

    /// Read the selected workspace's cached Git branches, starting one
    /// background fetch on a miss. The previous selected-path snapshot remains
    /// drawable while an invalidation is being refreshed.
    pub(in crate::app) fn branch_snapshot_for_workspace(
        &mut self,
        workspace_path: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> Option<BranchSnapshot> {
        let workspace_path = workspace_path.to_path_buf();
        let fallback = self
            .branches
            .visible_snapshot
            .as_ref()
            .filter(|(path, _)| path == &workspace_path)
            .map(|(_, snapshot)| snapshot.clone());

        match self.branches.snapshots.read(&workspace_path) {
            Query::Ready(result) => match result.as_ref() {
                Ok(Some(snapshot)) => {
                    let snapshot = snapshot.clone();
                    self.branches.visible_snapshot = Some((workspace_path, snapshot.clone()));
                    Some(snapshot)
                }
                Ok(None) => {
                    if self
                        .branches
                        .visible_snapshot
                        .as_ref()
                        .is_some_and(|(path, _)| path == &workspace_path)
                    {
                        self.branches.visible_snapshot = None;
                    }
                    None
                }
                Err(_) => fallback,
            },
            Query::Pending => fallback,
            Query::Missing(token) => {
                let fetch_path = workspace_path.clone();
                let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
                cx.spawn(async move |michelle, cx| {
                    let result = cx
                        .background_executor()
                        .spawn({
                            let fetch_path = fetch_path.clone();
                            async move {
                                match workspace.request(
                                    michelle_client::WorkspaceOperation::InspectBranches {
                                        cwd: fetch_path.clone(),
                                    },
                                ) {
                                    Ok(michelle_client::WorkspaceResult::Branches { snapshot }) => {
                                        Ok(snapshot)
                                    }
                                    Ok(_) => {
                                        Err("the daemon returned an invalid branch response"
                                            .to_owned())
                                    }
                                    Err(error) => Err(error.to_string()),
                                }
                            }
                        })
                        .await;
                    let _ = michelle.update(cx, |michelle, cx| {
                        if !michelle.branches.snapshots.fulfill(token, result.clone()) {
                            return;
                        }
                        let selected = michelle
                            .selected_workspace_path()
                            .is_some_and(|path| path == fetch_path);
                        if selected {
                            match result {
                                Ok(Some(snapshot)) => {
                                    let mut persisted_branch_changed = false;
                                    if let Some(current) = snapshot.current.as_deref()
                                        && let Some(session) = michelle.selected_session_mut()
                                        && let SessionWorkspace::Worktree { branch, .. } =
                                            &mut session.workspace
                                        && branch != current
                                    {
                                        *branch = current.to_owned();
                                        persisted_branch_changed = true;
                                    }
                                    michelle.branches.visible_snapshot =
                                        Some((fetch_path, snapshot));
                                    if persisted_branch_changed {
                                        michelle.save();
                                    }
                                }
                                Ok(None) => michelle.branches.visible_snapshot = None,
                                Err(_) => {}
                            }
                            cx.notify();
                        }
                    });
                })
                .detach();
                fallback
            }
        }
    }

    pub(in crate::app) fn refresh_selected_branch_snapshot(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.branches.visible_snapshot = None;
            return;
        };
        self.branches.snapshots.invalidate(&path);
        cx.notify();
    }

    /// Select an existing branch. A planned worktree remembers it as the base
    /// ref without touching the ordinary checkout; concrete workspaces run a
    /// real `git switch` on the background executor.
    ///
    /// `true` asks the caller to dismiss the picker after this entity update
    /// ends. Closing sooner runs the toggle observer, which re-enters `Michelle`
    /// and double-leases the entity.
    pub(in crate::app) fn choose_workspace_branch(
        &mut self,
        branch: String,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(session) = self.selected_session() else {
            return false;
        };
        if session.is_busy() || self.branches.operation_pending {
            return false;
        }
        if matches!(session.workspace, SessionWorkspace::NewWorktree { .. }) {
            let changed = self.selected_session_mut().is_some_and(|session| {
                let SessionWorkspace::NewWorktree { base_branch } = &mut session.workspace else {
                    return false;
                };
                if base_branch.as_deref() == Some(branch.as_str()) {
                    return false;
                }
                *base_branch = Some(branch);
                true
            });
            if changed {
                self.save();
                cx.notify();
            }
            return true;
        }

        let Some(path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            return false;
        };
        if self
            .branches
            .visible_snapshot
            .as_ref()
            .filter(|(snapshot_path, _)| snapshot_path == &path)
            .and_then(|(_, snapshot)| snapshot.current.as_deref())
            == Some(branch.as_str())
        {
            return true;
        }
        self.start_branch_operation(path, BranchOperation::Checkout(branch), cx);
        true
    }

    fn start_branch_operation(
        &mut self,
        path: PathBuf,
        operation: BranchOperation,
        cx: &mut Context<Self>,
    ) {
        if self.branches.operation_pending {
            return;
        }
        self.branches.operation_pending = true;
        cx.notify();
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    async move {
                        let (branch, create) = match operation {
                            BranchOperation::Checkout(branch) => (branch, false),
                            BranchOperation::Create(branch) => (branch, true),
                        };
                        match workspace.request(
                            michelle_client::WorkspaceOperation::CheckoutBranch {
                                cwd: path,
                                branch,
                                create,
                            },
                        )? {
                            michelle_client::WorkspaceResult::BranchChanged { snapshot } => {
                                Ok(snapshot)
                            }
                            _ => anyhow::bail!("the daemon returned an invalid branch response"),
                        }
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                michelle.branches.operation_pending = false;
                match result {
                    Ok(snapshot) => {
                        let current = snapshot.current.clone();
                        michelle.branches.visible_snapshot = Some((path.clone(), snapshot));
                        michelle.branches.snapshots.invalidate(&path);
                        let selected_path = michelle
                            .selected_workspace_path()
                            .map(std::path::Path::to_path_buf);
                        if selected_path.as_ref() == Some(&path) {
                            if let Some(current) = current
                                && let Some(session) = michelle.selected_session_mut()
                                && let SessionWorkspace::Worktree { branch, .. } =
                                    &mut session.workspace
                            {
                                *branch = current;
                            }
                            michelle.invalidate_workspace_queries(cx);
                            michelle.reload_clean_right_panel_file_editors(cx);
                            michelle.save();
                        }
                    }
                    Err(error) => {
                        michelle.show_toast(tr!("errors.change_branch", error = error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn create_workspace_branch(
        &mut self,
        branch: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.branches.operation_pending || branch.is_empty() {
            return false;
        }
        let Some(path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            return false;
        };
        self.start_branch_operation(path, BranchOperation::Create(branch), cx);
        true
    }
}
/// Branches matching the search, with the selected branch pinned first and
/// every other row sorted by name. Disabled worktree-owned rows stay in the
/// result; the UI needs to explain why Git cannot switch to them.
pub(in crate::app) fn visible_branch_entries(
    branches: &[crate::git_branch::BranchEntry],
    selected_branch: &str,
    normalized_query: &str,
) -> Vec<crate::git_branch::BranchEntry> {
    let normalized_query = normalized_query.to_ascii_lowercase();
    let mut visible = branches
        .iter()
        .filter(|branch| {
            normalized_query
                .split_whitespace()
                .all(|token| branch.name.to_ascii_lowercase().contains(token))
        })
        .cloned()
        .collect::<Vec<_>>();
    visible.sort_by(|left, right| {
        let left_selected = left.name == selected_branch;
        let right_selected = right.name == selected_branch;
        right_selected
            .cmp(&left_selected)
            .then_with(|| left.name.cmp(&right.name))
    });
    visible
}
