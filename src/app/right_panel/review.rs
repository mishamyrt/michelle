//! Right panel review operations.
use super::*;

impl Michelle {
    pub(in crate::app) fn latest_review_turn_source(&self) -> Option<ReviewDiffSource> {
        let session = self.selected_session()?;
        session
            .turns
            .iter()
            .rev()
            .find(|turn| {
                turn.turn_count > 0
                    && turn
                        .checkpoint
                        .as_ref()
                        .is_some_and(|checkpoint| checkpoint.status == CheckpointStatus::Ready)
            })
            .map(|turn| ReviewDiffSource::LastTurn {
                session_id: session.id,
                turn_id: turn.id,
                turn_count: turn.turn_count,
            })
    }

    /// Captures one stable Git range and turns it into render-ready rows. Git,
    /// patch parsing, and syntax tokenization all stay off the UI thread; the
    /// generation check prevents an old source or session from landing late.
    pub(in crate::app) fn refresh_right_panel_diff(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.state.selected_session else {
            self.right_panel_ui.diff_selection.clear();
            self.right_panel_model.diff_snapshot = None;
            self.right_panel_model.diff_loading = false;
            self.right_panel_model.diff_error = Some(tr!("diff.unavailable"));
            return;
        };
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.right_panel_ui.diff_selection.clear();
            self.right_panel_model.diff_snapshot = None;
            self.right_panel_model.diff_loading = false;
            self.right_panel_model.diff_error = Some(tr!("diff.unavailable"));
            return;
        };

        self.right_panel_model.diff_generation =
            self.right_panel_model.diff_generation.wrapping_add(1);
        let generation = self.right_panel_model.diff_generation;
        let source = self.right_panel_model.diff_source;
        let had_snapshot = self.right_panel_model.diff_snapshot.is_some();
        let previous_directories = self
            .right_panel_model
            .diff_snapshot
            .as_ref()
            .map_or_else(HashSet::new, |snapshot| {
                review_diff_directory_paths(&snapshot.files)
            });
        let selected_path = self.right_panel_ui.diff_selected_file.and_then(|index| {
            self.right_panel_model
                .diff_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.files.get(index))
                .map(|file| file.path.clone())
        });
        self.right_panel_model.diff_loading = true;
        self.right_panel_model.diff_error = None;
        cx.notify();

        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let project_path = project_path.clone();
                    async move {
                        match workspace.request(
                            michelle_client::WorkspaceOperation::CollectReviewDiff {
                                cwd: project_path,
                                source: crate::review_diff::wire_source(source),
                            },
                        )? {
                            michelle_client::WorkspaceResult::ReviewDiff { data } => {
                                Ok(crate::review_diff::parse_collected(
                                    source,
                                    &data.numstat,
                                    &data.patch,
                                    data.complete_context,
                                ))
                            }
                            _ => anyhow::bail!("the daemon returned an invalid diff response"),
                        }
                    }
                })
                .await;
            michelle
                .update(cx, |michelle, cx| {
                    let still_current = michelle.state.selected_session == Some(session_id)
                        && michelle.right_panel_model.diff_generation == generation
                        && michelle.right_panel_model.diff_source == source
                        && michelle
                            .selected_workspace_path()
                            .is_some_and(|path| path == project_path);
                    if !still_current {
                        return;
                    }

                    michelle.right_panel_model.diff_loading = false;
                    match result {
                        Ok(snapshot) => {
                            michelle.right_panel_ui.diff_selection.clear();
                            let directories = review_diff_directory_paths(&snapshot.files);
                            if had_snapshot {
                                michelle
                                    .right_panel_ui
                                    .diff_expanded_paths
                                    .retain(|path| directories.contains(path));
                                michelle
                                    .right_panel_ui
                                    .diff_expanded_paths
                                    .extend(directories.difference(&previous_directories).cloned());
                            } else {
                                michelle.right_panel_ui.diff_expanded_paths = directories;
                            }
                            michelle.right_panel_ui.diff_selected_file = selected_path
                                .as_deref()
                                .and_then(|path| {
                                    snapshot.files.iter().position(|file| file.path == path)
                                })
                                .or_else(|| (!snapshot.files.is_empty()).then_some(0));
                            let line_count = snapshot.lines.len();
                            michelle.right_panel_model.diff_snapshot = Some(Arc::new(snapshot));
                            michelle.right_panel_model.diff_error = None;
                            michelle.right_panel_ui.diff_list_state.reset(line_count);
                            michelle.sync_right_panel_diff_tree_rows(cx);
                        }
                        Err(error) => {
                            let message = error.to_string();
                            if michelle.right_panel_model.diff_snapshot.is_some() {
                                michelle.show_toast(tr!("diff.refresh_failed", error = message));
                            } else {
                                michelle.right_panel_model.diff_error = Some(message);
                            }
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub(in crate::app) fn set_right_panel_diff_source(
        &mut self,
        source: ReviewDiffSource,
        cx: &mut Context<Self>,
    ) {
        if self.right_panel_model.diff_source != source {
            self.right_panel_ui.diff_selection.clear();
            self.right_panel_model.diff_source = source;
            self.right_panel_model.diff_snapshot = None;
            self.right_panel_model.diff_error = None;
            self.right_panel_ui.diff_selected_file = None;
            self.right_panel_ui.diff_expanded_paths.clear();
            self.right_panel_ui.diff_tree_cursor = None;
            self.right_panel_ui.diff_tree_rows.borrow_mut().clear();
            self.right_panel_ui.diff_tree_list_state.reset(0);
            self.right_panel_ui.diff_list_state.reset(0);
        }
        self.open_right_panel_surface(RightPanelSurface::Diff, cx);
    }
}

pub(super) fn review_diff_directory_paths(files: &[crate::review_diff::File]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for file in files {
        let parts = file.path.split('/').collect::<Vec<_>>();
        let mut path = String::new();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            paths.insert(path.clone());
        }
    }
    paths
}

pub(super) fn review_diff_tree_rows(
    files: &[crate::review_diff::File],
    expanded_paths: &HashSet<String>,
    filter: &str,
) -> Vec<ReviewDiffTreeRow> {
    let filter = filter.trim().to_ascii_lowercase();
    let filtering = !filter.is_empty();
    let mut indexes = files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            filtering
                .then(|| file.path.to_ascii_lowercase().contains(&filter))
                .unwrap_or(true)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indexes.sort_by_key(|index| files[*index].path.to_ascii_lowercase());

    let mut rows = Vec::new();
    let mut emitted_directories = HashSet::new();
    for file_index in indexes {
        let parts = files[file_index].path.split('/').collect::<Vec<_>>();
        let mut directory = String::new();
        let mut visible = true;
        for (depth, part) in parts.iter().take(parts.len().saturating_sub(1)).enumerate() {
            if !directory.is_empty() {
                directory.push('/');
            }
            directory.push_str(part);
            let expanded = filtering || expanded_paths.contains(&directory);
            if emitted_directories.insert(directory.clone()) && visible {
                rows.push(ReviewDiffTreeRow::Directory {
                    path: directory.clone(),
                    name: (*part).to_owned(),
                    depth,
                    expanded,
                });
            }
            if !expanded {
                visible = false;
                break;
            }
        }
        if visible {
            rows.push(ReviewDiffTreeRow::File {
                file_index,
                depth: parts.len().saturating_sub(1),
            });
        }
    }
    rows
}

impl Michelle {
    pub(in crate::app) fn expand_right_panel_diff_gap(
        &mut self,
        line_index: usize,
        direction: crate::review_diff::ExpansionDirection,
        cx: &mut Context<Self>,
    ) {
        let expansion = self
            .right_panel_model
            .diff_snapshot
            .as_mut()
            .and_then(|snapshot| Arc::make_mut(snapshot).expand_gap(line_index, direction));
        let Some(expansion) = expansion else {
            return;
        };
        self.right_panel_ui
            .diff_list_state
            .splice(line_index..line_index + 1, expansion.replacement_count);
        cx.notify();
    }
}
