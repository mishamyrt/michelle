//! Background command/file discovery and in-memory indexes for autocomplete.
use super::*;
use crate::composer_complete::{self, FILE_INDEX_CAP};

pub(in crate::app) struct ComposerSources {
    pub(in crate::app) slash_commands:
        QueryCache<(ProviderKind, PathBuf, Option<String>), Vec<SlashCommand>>,
    pub(in crate::app) slash_command_index: Rc<Vec<SlashCommand>>,
    pub(in crate::app) slash_command_index_key: Option<(ProviderKind, PathBuf, Option<String>)>,
    pub(in crate::app) slash_command_index_loading: bool,
    pub(in crate::app) mention_files: QueryCache<PathBuf, Vec<FileEntry>>,
    pub(in crate::app) mention_file_index: Rc<Vec<FileEntry>>,
    pub(in crate::app) mention_file_index_path: Option<PathBuf>,
    pub(in crate::app) mention_file_index_loading: bool,
    pub(in crate::app) stale: bool,
}

impl Michelle {
    /// Refresh the drawn command and file indexes for the selected session.
    ///
    /// A cache hit lands immediately; a miss starts discovery on the
    /// background executor and re-runs this when it arrives. Nothing here may
    /// touch the filesystem directly.
    pub(in crate::app) fn refresh_composer_sources(&mut self, cx: &mut Context<Self>) {
        let Some(project_path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.composer_model.sources.slash_command_index = Rc::new(Vec::new());
            self.composer_model.sources.slash_command_index_key = None;
            self.composer_model.sources.slash_command_index_loading = false;
            self.composer_model.sources.mention_file_index = Rc::new(Vec::new());
            self.composer_model.sources.mention_file_index_path = None;
            self.composer_model.sources.mention_file_index_loading = false;
            return;
        };
        let provider = self
            .selected_session()
            .map(|session| session.provider)
            .unwrap_or(self.state.last_provider);
        let reported = self
            .selected_session()
            .map(|session| session.available_commands.clone())
            .unwrap_or_default();
        let binary_override = self.state.provider_binary_overrides.get(&provider).cloned();

        let command_key = (provider, project_path.clone(), binary_override.clone());
        match self
            .composer_model
            .sources
            .slash_commands
            .read(&command_key)
        {
            Query::Ready(commands) => {
                self.composer_model.sources.slash_command_index = Rc::new(
                    composer_complete::merge_reported_commands(&commands, &reported),
                );
                self.composer_model.sources.slash_command_index_key = Some(command_key);
                self.composer_model.sources.slash_command_index_loading = false;
            }
            Query::Pending => {
                self.composer_model.sources.slash_command_index_loading = true;
                // A scan for this exact key is in flight; anything drawn
                // meanwhile must not be another provider's list.
                if self.composer_model.sources.slash_command_index_key.as_ref()
                    != Some(&command_key)
                {
                    self.composer_model.sources.slash_command_index = Rc::new(Vec::new());
                    self.composer_model.sources.slash_command_index_key = None;
                }
            }
            Query::Missing(token) => {
                self.composer_model.sources.slash_command_index_loading = true;
                if self.composer_model.sources.slash_command_index_key.as_ref()
                    != Some(&command_key)
                {
                    self.composer_model.sources.slash_command_index = Rc::new(Vec::new());
                    self.composer_model.sources.slash_command_index_key = None;
                }
                let path = project_path.clone();
                let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
                cx.spawn(async move |michelle, cx| {
                    let commands = cx
                        .background_executor()
                        .spawn(async move {
                            match workspace.request(
                                michelle_client::WorkspaceOperation::DiscoverSlashCommands {
                                    provider,
                                    project_root: path,
                                    binary_override,
                                },
                            ) {
                                Ok(michelle_client::WorkspaceResult::SlashCommands {
                                    commands,
                                }) => commands,
                                Ok(_) | Err(_) => Vec::new(),
                            }
                        })
                        .await;
                    michelle
                        .update(cx, |michelle, cx| {
                            if michelle
                                .composer_model
                                .sources
                                .slash_commands
                                .fulfill(token, commands)
                            {
                                michelle.refresh_composer_sources(cx);
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .detach();
            }
        }

        match self
            .composer_model
            .sources
            .mention_files
            .read(&project_path)
        {
            Query::Ready(files) => {
                self.composer_model.sources.mention_file_index = files.as_ref().clone().into();
                self.composer_model.sources.mention_file_index_path = Some(project_path);
                self.composer_model.sources.mention_file_index_loading = false;
            }
            Query::Pending => {
                self.composer_model.sources.mention_file_index_loading = true;
                if self.composer_model.sources.mention_file_index_path.as_ref()
                    != Some(&project_path)
                {
                    self.composer_model.sources.mention_file_index = Rc::new(Vec::new());
                    self.composer_model.sources.mention_file_index_path = None;
                }
            }
            Query::Missing(token) => {
                self.composer_model.sources.mention_file_index_loading = true;
                if self.composer_model.sources.mention_file_index_path.as_ref()
                    != Some(&project_path)
                {
                    self.composer_model.sources.mention_file_index = Rc::new(Vec::new());
                    self.composer_model.sources.mention_file_index_path = None;
                }
                let path = project_path.clone();
                let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
                cx.spawn(async move |michelle, cx| {
                    let files = cx
                        .background_executor()
                        .spawn(async move {
                            match workspace.request(
                                michelle_client::WorkspaceOperation::ListProjectFiles {
                                    root: path,
                                    cap: FILE_INDEX_CAP,
                                },
                            ) {
                                Ok(michelle_client::WorkspaceResult::ProjectFiles { entries }) => {
                                    entries
                                }
                                Ok(_) | Err(_) => Vec::new(),
                            }
                        })
                        .await;
                    michelle
                        .update(cx, |michelle, cx| {
                            if michelle
                                .composer_model
                                .sources
                                .mention_files
                                .fulfill(token, files)
                            {
                                michelle.refresh_composer_sources(cx);
                                cx.notify();
                            }
                        })
                        .ok();
                })
                .detach();
            }
        }
    }

    /// Invalidate and re-request both indexes for the selected workspace.
    pub(in crate::app) fn invalidate_composer_sources(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self
            .selected_workspace_path()
            .map(std::path::Path::to_path_buf)
        {
            let provider = self
                .selected_session()
                .map(|session| session.provider)
                .unwrap_or(self.state.last_provider);
            let binary_override = self.state.provider_binary_overrides.get(&provider).cloned();
            self.composer_model.sources.slash_commands.invalidate(&(
                provider,
                path.clone(),
                binary_override,
            ));
            self.composer_model.sources.mention_files.invalidate(&path);
        }
        self.refresh_composer_sources(cx);
    }
}
