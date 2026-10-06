//! Right panel files operations.
use super::*;

pub(in crate::app) struct RightPanelFileEditor {
    pub(in crate::app) state: Entity<TextInput>,
    pub(in crate::app) disk_content: String,
    pub(in crate::app) writable: bool,
    pub(in crate::app) dirty: bool,
    /// A read is in flight on the background executor. Set from the moment the
    /// editor is created, because `render` may not touch the filesystem: until
    /// the first read lands the editor is empty and locked, and that means
    /// "not read yet", never "empty file".
    pub(in crate::app) reading: bool,
    /// Bumped whenever the editor's idea of the file changes, so a read that
    /// started earlier cannot apply over a newer truth — a save in particular,
    /// which makes any read already in flight describe the pre-save file.
    pub(in crate::app) read_epoch: u64,
}

impl Michelle {
    pub(in crate::app) fn right_panel_file_is_dirty(&self, relative_path: &str) -> bool {
        self.right_panel_model
            .file_editors
            .get(relative_path)
            .is_some_and(|editor| editor.dirty)
    }

    /// The file the active editor surface is showing, whether via a File tab
    /// or the Files browser's selection — regardless of whether the panel is
    /// currently visible, which is a per-caller decision: save works on a
    /// hidden panel, find does not.
    pub(in crate::app) fn visible_right_panel_file_path(&self) -> Option<String> {
        match self.active_right_panel_surface() {
            Some(RightPanelSurface::Files) => self.right_panel_ui.files_selected_path.clone(),
            Some(RightPanelSurface::File(path)) => Some(path.clone()),
            _ => None,
        }
    }

    pub(in crate::app) fn right_panel_surface_is_dirty(&self, surface: &RightPanelSurface) -> bool {
        match surface {
            RightPanelSurface::Files => self
                .right_panel_ui
                .files_selected_path
                .as_deref()
                .is_some_and(|path| self.right_panel_file_is_dirty(path)),
            RightPanelSurface::File(path) => self.right_panel_file_is_dirty(path),
            _ => false,
        }
    }

    /// Reads a file into its editor off the UI thread.
    ///
    /// One `read_to_string` of an arbitrarily large file — hundreds of frames
    /// for a big one — so it never runs in a frame. The editor keeps whatever
    /// it is already showing until the read lands.
    ///
    /// The result is applied only if the same session is still selected and the
    /// editor is still the one that asked, so a read started before a project
    /// or session switch cannot write another workspace's text into the view.
    pub(in crate::app) fn read_right_panel_file_into_editor(
        &mut self,
        relative_path: String,
        cx: &mut Context<Self>,
    ) {
        let project_path = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf);
        let (Some(project_path), Some(session_id)) = (project_path, self.state.selected_session)
        else {
            // Nothing to read from. Say so in the editor rather than leaving it
            // looking like an empty file.
            if let Some(editor) = self.right_panel_model.file_editors.get_mut(&relative_path) {
                editor.reading = false;
                editor.disk_content = tr!("files.no_project_is_open");
                editor.writable = false;
                let state = editor.state.clone();
                let content = editor.disk_content.clone();
                state.update(cx, |state, cx| state.set_content(content, cx));
            }
            return;
        };
        let Some(editor) = self.right_panel_model.file_editors.get_mut(&relative_path) else {
            return;
        };
        // A second asker would only duplicate the read and race to apply it.
        if editor.reading {
            return;
        }
        editor.reading = true;
        editor.read_epoch += 1;
        let epoch = editor.read_epoch;
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());

        cx.spawn(async move |michelle, cx| {
            let read = cx
                .background_executor()
                .spawn({
                    let project_path = project_path.clone();
                    let relative_path = relative_path.clone();
                    async move { read_right_panel_file(&workspace, &project_path, &relative_path) }
                })
                .await;
            michelle
                .update(cx, |michelle, cx| {
                    if michelle.state.selected_session != Some(session_id)
                        || michelle
                            .selected_workspace_path()
                            .is_none_or(|path| path != project_path)
                    {
                        // The editor moved into another session's stored state, or
                        // the project changed. Clear the flag so a later reload can
                        // ask again, and drop the text.
                        if let Some(editor) = michelle
                            .right_panel_model
                            .file_editors
                            .get_mut(&relative_path)
                        {
                            editor.reading = false;
                        }
                        return;
                    }
                    let (content, writable) = read;
                    let Some(editor) = michelle
                        .right_panel_model
                        .file_editors
                        .get_mut(&relative_path)
                    else {
                        return;
                    };
                    // A save landed while the read was in flight, so this text
                    // describes the file as it was before that save.
                    if editor.read_epoch != epoch {
                        return;
                    }
                    editor.reading = false;
                    // An edit landed while the read was in flight; the user's text
                    // wins over the copy on disk.
                    if editor.dirty {
                        return;
                    }
                    if editor.disk_content == content && editor.writable == writable {
                        return;
                    }
                    editor.disk_content = content.clone();
                    editor.writable = writable;
                    editor.dirty = false;
                    let state = editor.state.clone();
                    state.update(cx, |state, cx| {
                        state.set_read_only(!writable);
                        state.set_content(content, cx);
                    });
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// Picks up an external edit to a file the user has not modified here.
    ///
    /// Reaches the filesystem, so it queues a background read rather than
    /// blocking; the editor keeps showing its current text until that lands.
    fn reload_right_panel_file_if_clean(&mut self, relative_path: &str, cx: &mut Context<Self>) {
        if self
            .right_panel_model
            .file_editors
            .get(relative_path)
            .is_none_or(|editor| editor.dirty)
        {
            return;
        }
        self.read_right_panel_file_into_editor(relative_path.to_owned(), cx);
    }

    pub(in crate::app) fn reload_clean_right_panel_file_editors(&mut self, cx: &mut Context<Self>) {
        let paths = self
            .right_panel_model
            .file_editors
            .iter()
            .filter(|(_, editor)| !editor.dirty)
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        for path in paths {
            self.reload_right_panel_file_if_clean(&path, cx);
        }
    }
}

impl Michelle {
    pub(in crate::app) fn ensure_right_panel_file_editor(
        &mut self,
        relative_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<TextInput>, bool, bool) {
        if let Some(editor) = self.right_panel_model.file_editors.get(relative_path) {
            return (editor.state.clone(), editor.writable, editor.dirty);
        }

        // Reached from `render`, so the file cannot be read here. The editor
        // starts empty and locked, and `read_right_panel_file_into_editor`
        // fills it in from the background executor a frame or two later.
        let language = file_highlighter_language(relative_path);
        let state = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line()
                .syntax(Some(language))
                .read_only(true)
        });

        self.right_panel_model.file_editors.insert(
            relative_path.to_owned(),
            RightPanelFileEditor {
                state: state.clone(),
                disk_content: String::new(),
                writable: false,
                dirty: false,
                reading: false,
                read_epoch: 0,
            },
        );

        // Dirty tracking follows content edits. Observing raw notifies would
        // also fire for caret blinks and selection drags, cloning the whole
        // file's text for each one.
        let subscribed_path = relative_path.to_owned();
        cx.subscribe(
            &state,
            move |this: &mut Self, state, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Edited) {
                    return;
                }
                let value = state.read(cx).content().to_owned();
                if let Some(editor) = this
                    .right_panel_model
                    .file_editors
                    .get_mut(subscribed_path.as_str())
                {
                    let dirty = editor.writable && value != editor.disk_content;
                    if editor.dirty != dirty {
                        editor.dirty = dirty;
                        cx.notify();
                    }
                }
                // Any content change — typing, a replace, a reload from disk —
                // moves the text out from under an open find's match list.
                this.refresh_file_search_for_edit(subscribed_path.as_str(), cx);
            },
        )
        .detach();

        let focused_path = relative_path.to_owned();
        cx.subscribe(&state, move |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Focus) {
                this.reload_right_panel_file_if_clean(focused_path.as_str(), cx);
            }
        })
        .detach();

        self.read_right_panel_file_into_editor(relative_path.to_owned(), cx);
        (state, false, false)
    }

    pub(in crate::app) fn save_right_panel_file(
        &mut self,
        relative_path: String,
        cx: &mut Context<Self>,
    ) {
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            return;
        };
        let Some(editor) = self.right_panel_model.file_editors.get(&relative_path) else {
            return;
        };
        if !editor.writable {
            self.show_toast(if editor.reading {
                tr!("files.could_not_save_opening", path = relative_path)
            } else {
                tr!("files.could_not_save_read_only", path = relative_path)
            });
            cx.notify();
            return;
        }

        let content = editor.state.read(cx).content().to_owned();
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let epoch =
            if let Some(editor) = self.right_panel_model.file_editors.get_mut(&relative_path) {
                editor.reading = false;
                editor.read_epoch += 1;
                editor.read_epoch
            } else {
                return;
            };
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let project_path = project_path.clone();
                    let relative_path = relative_path.clone();
                    let content = content.clone();
                    async move {
                        match workspace.request(
                            michelle_client::WorkspaceOperation::WriteTextFile {
                                root: project_path,
                                relative_path: PathBuf::from(relative_path),
                                content,
                            },
                        )? {
                            michelle_client::WorkspaceResult::Ack => Ok(()),
                            _ => anyhow::bail!("the daemon returned an invalid file response"),
                        }
                    }
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                if michelle.state.selected_session != Some(session_id)
                    || michelle
                        .selected_workspace_path()
                        .is_none_or(|path| path != project_path)
                {
                    return;
                }
                match result {
                    Ok(()) => {
                        if let Some(editor) = michelle
                            .right_panel_model
                            .file_editors
                            .get_mut(&relative_path)
                            && editor.read_epoch == epoch
                        {
                            let current = editor.state.read(cx).content();
                            editor.disk_content = content.clone();
                            editor.dirty = current != content;
                        }
                    }
                    Err(error) => michelle.show_toast(tr!(
                        "files.could_not_save",
                        path = relative_path,
                        error = error.to_string()
                    )),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// Reads a file for the editor, returning its text and whether it can be saved.
///
/// One unbounded `read_to_string`, so callers keep it off the UI thread; the
/// only caller is [`Michelle::read_right_panel_file_into_editor`].
fn read_right_panel_file(
    workspace: &michelle_client::WorkspaceClient,
    project_path: &Path,
    relative_path: &str,
) -> (String, bool) {
    match workspace.request(michelle_client::WorkspaceOperation::ReadTextFile {
        root: project_path.to_path_buf(),
        relative_path: PathBuf::from(relative_path),
    }) {
        Ok(michelle_client::WorkspaceResult::TextFile { content }) => (content, true),
        Ok(_) => (
            tr!(
                "files.unable_to_edit",
                error = "the daemon returned an invalid file response"
            ),
            false,
        ),
        Err(error) => (
            tr!("files.unable_to_edit", error = error.to_string()),
            false,
        ),
    }
}

impl Michelle {
    pub(in crate::app) fn open_transcript_link(
        &mut self,
        target: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        match transcript_link_route(target, self.selected_workspace_path()) {
            TranscriptLinkRoute::ProjectFile(relative_path) => {
                self.open_right_panel_surface(RightPanelSurface::Files, cx);
                self.open_right_panel_file(relative_path, cx);
            }
            TranscriptLinkRoute::Finder(path) => {
                if self.daemon.is_remote() {
                    self.show_toast(tr!("errors.remote_host_path"));
                    cx.notify();
                } else {
                    crate::platform::reveal_in_file_manager(&path, cx);
                }
            }
            TranscriptLinkRoute::External => return false,
        }
        true
    }

    /// Open a path a tool reported, from an activity in the transcript.
    ///
    /// Providers name a changed file however they like — absolute, or relative
    /// to the session's workspace — so resolve it before routing. Inside the
    /// workspace it opens in the file viewer; anywhere else it goes to the file
    /// manager, the same split a file link in the transcript takes.
    pub(in crate::app) fn open_activity_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let path = Path::new(path.trim());
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(workspace) = self.selected_workspace_path() {
            workspace.join(path)
        } else {
            return;
        };
        self.open_transcript_link(&resolved.to_string_lossy(), cx);
    }
}
