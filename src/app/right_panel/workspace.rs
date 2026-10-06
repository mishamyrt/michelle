//! Right panel workspace operations.
use super::*;

impl Michelle {
    /// Re-reads whichever workspace surface is on screen.
    pub(in crate::app) fn refresh_workspace_surfaces(&mut self, cx: &mut Context<Self>) {
        match self.active_right_panel_surface() {
            Some(RightPanelSurface::Diff) => self.refresh_right_panel_diff(cx),
            Some(RightPanelSurface::Files | RightPanelSurface::File(_)) => {
                self.refresh_right_panel_working_tree(cx)
            }
            _ => {}
        }
    }

    /// Re-walks the project's working tree.
    ///
    /// `read_dir` plus a `stat` per entry, recursively over expanded
    /// directories — filesystem I/O, so it runs on the background executor and
    /// the panel keeps drawing the previous listing until the result lands.
    /// Called when the tree's inputs change, never from a frame.
    pub(in crate::app) fn refresh_right_panel_working_tree(&mut self, cx: &mut Context<Self>) {
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.right_panel_model.working_tree.clear();
            return;
        };
        // The tree on disk moves under us, and the expanded set may just have
        // changed, so a cached listing is only good until something asks again.
        self.right_panel_model
            .working_trees
            .invalidate(&project_path);
        match self.right_panel_model.working_trees.read(&project_path) {
            Query::Ready(entries) => self.right_panel_model.working_tree = (*entries).clone(),
            Query::Pending => {}
            Query::Missing(token) => {
                let expanded = self.right_panel_ui.expanded_paths.clone();
                let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
                cx.spawn(async move |michelle, cx| {
                    let entries = cx
                        .background_executor()
                        .spawn({
                            let path = project_path.clone();
                            async move {
                                match workspace.request(
                                    michelle_client::WorkspaceOperation::ListTree {
                                        root: path,
                                        expanded_paths: expanded.into_iter().collect(),
                                    },
                                ) {
                                    Ok(michelle_client::WorkspaceResult::WorkingTree {
                                        entries,
                                    }) => entries
                                        .into_iter()
                                        .map(|entry| WorkingTreeEntry {
                                            file_icon: (!entry.is_dir)
                                                .then(|| file_icon_for_name(&entry.name)),
                                            relative_path: entry.relative_path,
                                            absolute_path: entry.absolute_path,
                                            name: entry.name,
                                            is_dir: entry.is_dir,
                                            expanded: entry.expanded,
                                            depth: entry.depth,
                                        })
                                        .collect(),
                                    Ok(_) | Err(_) => Vec::new(),
                                }
                            }
                        })
                        .await;
                    michelle
                        .update(cx, |michelle, cx| {
                            if michelle
                                .right_panel_model
                                .working_trees
                                .fulfill(token, entries.clone())
                                && michelle
                                    .selected_workspace_path()
                                    .is_some_and(|path| path == project_path)
                            {
                                michelle.right_panel_model.working_tree = entries;
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .detach();
            }
        }
    }
}
