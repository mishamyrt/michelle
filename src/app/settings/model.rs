//! Settings operations and transient state. Persisted values remain in PersistedState.
use super::*;

pub(in crate::app) struct SettingsModel {
    pub(in crate::app) daemon_hostname: String,
    pub(in crate::app) themes: Rc<Vec<ThemeDefinition>>,
    pub(in crate::app) daemon_reconfigure_pending: bool,
    pub(in crate::app) providers: providers::ProviderCatalog,
    pub(in crate::app) permissions: permissions::PermissionState,
    pub(in crate::app) plan: plan_usage::PlanUsageState,
}

impl Michelle {
    pub(super) fn daemon_exposure_from_fields(
        &self,
        cx: &App,
    ) -> Result<michelle_client::DaemonExposureSettings, String> {
        let port = self
            .settings_ui
            .daemon_port_input
            .read(cx)
            .content()
            .trim()
            .parse::<u16>()
            .map_err(|_| tr!("daemon.invalid_port"))?;
        if port == 0 {
            return Err(tr!("daemon.invalid_port"));
        }
        let origins = self
            .settings_ui
            .daemon_origins_input
            .read(cx)
            .content()
            .to_owned();
        let mut settings = self.state.daemon_exposure.clone();
        settings.port = port;
        settings
            .with_allowed_origins_text(&origins)
            .and_then(michelle_client::DaemonExposureSettings::validate)
            .map_err(|error| error.to_string())
    }

    pub(super) fn daemon_exposure_fields_dirty(&self, cx: &App) -> bool {
        self.daemon_exposure_from_fields(cx)
            .map(|settings| {
                settings.port != self.state.daemon_exposure.port
                    || settings.allowed_origins != self.state.daemon_exposure.allowed_origins
            })
            .unwrap_or(true)
    }

    pub(super) fn set_daemon_exposure_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !enabled {
            self.settings_ui.daemon_token_revealed = false;
        }
        let settings = if enabled {
            match self.daemon_exposure_from_fields(cx) {
                Ok(mut settings) => {
                    settings.enabled = true;
                    settings
                }
                Err(error) => {
                    self.show_toast(tr!("daemon.invalid_settings", error = error));
                    return;
                }
            }
        } else {
            let mut settings = self.state.daemon_exposure.clone();
            settings.enabled = false;
            settings
        };
        self.apply_daemon_exposure(settings, cx);
    }

    pub(in crate::app) fn apply_daemon_exposure_fields(&mut self, cx: &mut Context<Self>) {
        let settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        self.apply_daemon_exposure(settings, cx);
    }

    pub(super) fn regenerate_daemon_token(&mut self, cx: &mut Context<Self>) {
        let mut settings = match self.daemon_exposure_from_fields(cx) {
            Ok(settings) => settings,
            Err(error) => {
                self.show_toast(tr!("daemon.invalid_settings", error = error));
                return;
            }
        };
        settings.token = michelle_client::DaemonExposureSettings::new_token();
        self.settings_ui.daemon_token_revealed = false;
        self.apply_daemon_exposure(settings, cx);
    }

    pub(super) fn apply_daemon_exposure(
        &mut self,
        settings: michelle_client::DaemonExposureSettings,
        cx: &mut Context<Self>,
    ) {
        if self.settings.daemon_reconfigure_pending || settings == self.state.daemon_exposure {
            return;
        }
        if self.daemon.is_remote() {
            self.show_toast(tr!("daemon.external_description"));
            return;
        }
        if self
            .state
            .sessions
            .iter()
            .any(|session| !matches!(session.status, SessionStatus::Idle | SessionStatus::Failed))
        {
            self.show_toast(tr!("daemon.stop_active_tasks"));
            return;
        }

        let needs_restart = self.state.daemon_exposure.enabled || settings.enabled;
        if !needs_restart {
            self.state.daemon_exposure = settings;
            self.save();
            cx.notify();
            return;
        }

        self.settings.daemon_reconfigure_pending = true;
        let daemon = self.daemon.clone();
        let applied = settings.clone();
        let restart = cx
            .background_executor()
            .spawn(async move { daemon.reconfigure(settings) });
        cx.spawn(async move |this, cx| {
            let result = restart.await;
            let _ = this.update(cx, |this, cx| {
                this.settings.daemon_reconfigure_pending = false;
                match result {
                    Ok(()) => {
                        this.state.daemon_exposure = applied.clone();
                        this.sessions.runtime.runtimes.clear();
                        this.settings_ui.daemon_port_input.update(cx, |input, cx| {
                            input.set_content(applied.port.to_string(), cx)
                        });
                        this.settings_ui
                            .daemon_origins_input
                            .update(cx, |input, cx| {
                                input.set_content(applied.allowed_origins_text(), cx)
                            });
                        this.save();
                        this.show_success_toast(tr!("daemon.settings_applied"));
                    }
                    Err(error) => {
                        this.show_toast(tr!("daemon.restart_failed", error = error.to_string()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn set_render_math(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.state.render_math == enabled {
            return;
        }
        self.state.render_math = enabled;
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    pub(super) fn set_ui_font_size(
        &mut self,
        size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let size = michelle_client::persistence::sanitized_ui_font_size(size);
        if self.state.ui_font_size == size {
            return;
        }
        self.state.ui_font_size = size;
        // Chrome is authored in `sp` rems; the rem size is the setting.
        window.set_rem_size(px(size));
        self.remeasure_font_sized_surfaces();
        self.save();
        window.refresh();
        cx.notify();
    }

    pub(super) fn set_code_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = michelle_client::persistence::sanitized_code_font_size(size);
        if self.state.code_font_size == size {
            return;
        }
        self.state.code_font_size = size;
        self.remeasure_font_sized_surfaces();
        self.save();
        cx.notify();
    }

    /// Drop every cached row height that a font size participates in. The
    /// virtualized lists remember measured heights, so a stale entry would
    /// misplace scroll anchors until the row happened to remeasure. The
    /// sidebar list keeps its uniform row height and needs no reset.
    pub(super) fn remeasure_font_sized_surfaces(&self) {
        self.reset_transcript_rows(self.transcript_row_count());
        let line_count = self
            .right_panel_model
            .diff_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.lines.len());
        self.right_panel_ui.diff_list_state.reset(line_count);
        self.right_panel_ui
            .diff_tree_list_state
            .reset(self.right_panel_ui.diff_tree_rows.borrow().len());
        self.skills_ui
            .list_state
            .reset(self.skills_ui.rows.borrow().len());
    }

    /// Commit the binary override edit for the expanded provider: empty means
    /// detect from PATH. Re-detects that provider and refreshes every catalog
    /// keyed by the executable path.
    pub(in crate::app) fn apply_provider_path_override(&mut self, cx: &mut Context<Self>) {
        let Some(provider) = self.settings_ui.expanded_provider else {
            return;
        };
        let text = self
            .settings_ui
            .provider_path_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let current = self
            .state
            .provider_binary_overrides
            .get(&provider)
            .cloned()
            .unwrap_or_default();
        if text == current {
            return;
        }
        if text.is_empty() {
            self.state.provider_binary_overrides.remove(&provider);
        } else {
            self.state.provider_binary_overrides.insert(provider, text);
        }
        self.save();
        self.refresh_provider_detection(Some(provider));
        self.refresh_composer_sources(cx);
        cx.notify();
    }

    /// Providers switched off here stop offering models to new sessions;
    /// sessions already locked to them keep working.
    pub(super) fn set_provider_enabled(
        &mut self,
        provider: ProviderKind,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if enabled {
            self.state
                .disabled_providers
                .retain(|kind| *kind != provider);
        } else if !self.state.disabled_providers.contains(&provider) {
            self.state.disabled_providers.push(provider);
        }
        if !enabled
            && let Some(fallback) = ProviderKind::ALL
                .into_iter()
                .find(|kind| self.provider_enabled(*kind))
        {
            // New work must land somewhere usable: move the new-session
            // default and any unstarted drafts off the switched-off provider.
            // The remembered model belongs to the old provider, so it resets
            // with it.
            if self.state.last_provider == provider {
                self.state.last_provider = fallback;
                self.state.last_model = None;
                self.state.last_reasoning_effort = None;
                self.state.last_service_tier = None;
                self.state.last_context_window = None;
            }
            let draft_ids = self
                .state
                .sessions
                .iter()
                .filter(|session| session.provider == provider && !session.has_started())
                .map(|session| session.id)
                .collect::<Vec<_>>();
            for id in draft_ids {
                if let Some(session) = self.state.session_mut(id) {
                    session.provider = fallback;
                    session.model = None;
                    session.reasoning_effort = None;
                    session.service_tier = None;
                    session.context_window = None;
                }
            }
        }
        self.save();
        cx.notify();
    }

    pub(super) fn selected_theme(&self) -> &ThemeDefinition {
        self.settings
            .themes
            .iter()
            .find(|theme| theme.file == self.state.theme_name)
            .unwrap_or(&self.settings.themes[0])
    }

    pub(in crate::app) fn apply_theme(&self, window: &mut Window, cx: &mut App) {
        crate::theme::apply_theme_preference(self.state.theme, self.selected_theme(), window, cx);
    }

    pub(in crate::app) fn load_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx
            .background_executor()
            .spawn(async { crate::theme::load_themes() });
        cx.spawn_in(window, async move |this, cx| {
            let themes = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.themes = Rc::new(themes);
                if this.settings.themes.len() > 1 {
                    this.apply_theme(window, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn set_theme(
        &mut self,
        file: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.theme_name == file {
            return;
        }
        self.state.theme_name = file;
        self.apply_theme(window, cx);
        self.save();
        cx.notify();
    }

    pub(super) fn set_color_scheme(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.theme == preference {
            return;
        }
        self.state.theme = preference;
        self.apply_theme(window, cx);
        self.save();
        cx.notify();
    }

    pub(super) fn set_language(
        &mut self,
        language: crate::i18n::AppLanguage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.language == language {
            return;
        }

        self.state.language = language;
        crate::i18n::set_language(language);

        self.composer_ui.input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.do_anything"), cx)
        });
        self.model_picker_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_models"), cx)
        });
        self.branch_picker_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_branches"), cx)
        });
        self.branch_picker_ui.create_input.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.new_branch_name"), cx)
        });
        self.project_picker_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.search_projects"), cx)
        });
        self.settings_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("settings.search"), cx)
        });
        self.skills_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("skills.search"), cx)
        });
        self.settings_ui
            .provider_path_input
            .update(cx, |input, cx| {
                input.set_placeholder(tr!("input.detected_automatically"), cx)
            });
        self.usage_ui.project_filter.update(cx, |input, cx| {
            input.set_placeholder(tr!("input.filter_projects"), cx)
        });
        self.refresh_command_palette_localized_text(cx);
        self.refresh_file_search_localized_text(cx);
        self.refresh_transcript_search_localized_text(cx);
        for terminal in self.right_panel_model.terminals.values() {
            terminal.update(cx, |terminal, cx| terminal.refresh_localized_text(cx));
        }
        for probe in &mut self.settings.providers.probes {
            probe.models = crate::model_catalog::fallback_models(probe.provider);
        }
        self.refresh_provider_detection(None);
        self.invalidate_composer_sources(cx);

        crate::set_app_menus(cx);
        self.save();
        window.refresh();
        cx.notify();
    }
}
