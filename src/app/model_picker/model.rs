//! Model projections and selection operations use the shared Settings catalog and PersistedState.
use super::*;

impl Michelle {
    fn remember_selected_model_traits(&mut self) {
        let Some((provider, model, reasoning_effort, service_tier, context_window)) =
            self.selected_session().and_then(|session| {
                Some((
                    session.provider,
                    self.model_for_session(session)?.to_owned(),
                    session.reasoning_effort.clone(),
                    session.service_tier.clone(),
                    session.context_window.clone(),
                ))
            })
        else {
            return;
        };
        self.state.remember_model_traits(
            provider,
            &model,
            reasoning_effort,
            service_tier,
            context_window,
        );
    }

    pub(in crate::app) fn choose_model(
        &mut self,
        provider: ProviderKind,
        model: String,
        cx: &mut Context<Self>,
    ) {
        let Some((session_id, provider_changed)) = self
            .selected_session()
            .filter(|session| {
                session.can_choose_model(provider)
                    && (session.provider != provider
                        || session.model.as_deref() != Some(model.as_str()))
            })
            .map(|session| (session.id, session.provider != provider))
        else {
            return;
        };

        self.remember_selected_model_traits();
        let (reasoning_effort, service_tier, context_window) =
            self.state.model_traits_for(provider, &model);
        if let Some(session) = self.selected_session_mut() {
            session.provider = provider;
            session.model = Some(model.clone());
            if provider_changed {
                session.agent_preset = None;
            }
            session.reasoning_effort.clone_from(&reasoning_effort);
            session.service_tier.clone_from(&service_tier);
            session.context_window.clone_from(&context_window);
            self.state.last_provider = provider;
            self.state.last_model = Some(model);
            self.state.last_reasoning_effort = reasoning_effort;
            self.state.last_service_tier = service_tier;
            self.state.last_context_window = context_window;
            // A different provider is a different binary and protocol; only a
            // model change within one provider can be applied in session.
            if provider_changed {
                self.reset_session_runtime(session_id);
                // A different provider is also a different command registry.
                self.refresh_composer_sources(cx);
            } else {
                self.apply_session_options(session_id, cx);
            }
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn toggle_favorite_model(
        &mut self,
        provider: ProviderKind,
        model: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self
            .state
            .favorite_models
            .iter()
            .position(|favorite| favorite.provider == provider && favorite.model == model)
        {
            self.state.favorite_models.remove(index);
        } else {
            self.state
                .favorite_models
                .push(FavoriteModel { provider, model });
        }
        let rows = self.current_model_picker_rows(cx);
        self.sync_model_picker_rows(&rows);
        self.save();
        cx.notify();
    }

    pub(in crate::app) fn can_configure_model_traits(&self) -> bool {
        self.selected_session()
            .and_then(|session| self.model_metadata_for_session(session))
            .is_some_and(|model| {
                !model.reasoning_efforts.is_empty()
                    || !model.service_tiers.is_empty()
                    || !model.context_windows.is_empty()
            })
    }

    pub(in crate::app) fn set_reasoning_effort(&mut self, effort: String, cx: &mut Context<Self>) {
        if let Some(session) = self.selected_session_mut()
            && session.reasoning_effort.as_deref() != Some(effort.as_str())
        {
            let session_id = session.id;
            session.reasoning_effort = Some(effort.clone());
            self.state.last_reasoning_effort = Some(effort);
            self.remember_selected_model_traits();
            self.apply_session_options(session_id, cx);
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn set_service_tier(&mut self, tier: String, cx: &mut Context<Self>) {
        if let Some(session) = self.selected_session_mut()
            && session.service_tier.as_deref() != Some(tier.as_str())
        {
            let session_id = session.id;
            session.service_tier = Some(tier.clone());
            self.state.last_service_tier = Some(tier);
            self.remember_selected_model_traits();
            self.apply_session_options(session_id, cx);
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn set_context_window(&mut self, window: String, cx: &mut Context<Self>) {
        if let Some(session) = self.selected_session_mut()
            && session.context_window.as_deref() != Some(window.as_str())
        {
            let session_id = session.id;
            session.context_window = Some(window.clone());
            self.state.last_context_window = Some(window);
            self.remember_selected_model_traits();
            self.apply_session_options(session_id, cx);
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn set_agent_preset(
        &mut self,
        agent_preset: String,
        cx: &mut Context<Self>,
    ) {
        let selectable = self
            .provider_probe(ProviderKind::DeepSeek)
            .is_some_and(|probe| {
                probe
                    .agent_presets
                    .iter()
                    .any(|preset| preset.id == agent_preset)
            });
        if !selectable {
            return;
        }
        if let Some(session) = self.selected_session_mut()
            && session.provider == ProviderKind::DeepSeek
            && !session.has_started()
            && !session.is_busy()
            && session.agent_preset.as_deref() != Some(agent_preset.as_str())
        {
            let session_id = session.id;
            session.agent_preset = Some(agent_preset);
            // A provider cursor makes a session started, so this is normally a
            // no-op. It also closes the narrow race where a blank runtime was
            // prepared but had not reported its native session yet.
            self.reset_session_runtime(session_id);
            self.save();
            cx.notify();
        }
    }

    /// Whether the model picker has no provider left to offer — nothing
    /// detected on this machine, or everything switched off — so the
    /// composer's trigger, the picker panel, and the send button all swap to
    /// their unavailable state.
    pub(in crate::app) fn model_picker_has_no_providers(&self) -> bool {
        let locked_provider = self
            .selected_session()
            .filter(|session| !session.messages.is_empty())
            .map(|session| session.provider);
        picker_has_no_providers(
            &self.settings.providers.probes,
            &self.state.disabled_providers,
            locked_provider,
            self.settings.providers.detection_checked_at.is_some(),
        )
    }
}
/// Installed providers enabled for new work, retaining a locked session's provider.
pub(in crate::app) fn picker_has_provider(
    probes: &[ProviderProbe],
    disabled_providers: &[ProviderKind],
    locked_provider: Option<ProviderKind>,
    kind: ProviderKind,
) -> bool {
    let installed = probes
        .iter()
        .any(|probe| probe.provider == kind && probe.installed);
    let switched_off = disabled_providers.contains(&kind) && locked_provider != Some(kind);
    installed && !switched_off
}

/// Whether the picker has nothing left to offer, so the composer's trigger
/// and the panel behind it both swap to their empty state.
///
/// `detection_settled` gates the whole answer. Every probe is seeded as "not
/// installed" and detection answers off the UI thread, so a pass that has
/// never completed means "not known yet", never "nothing here" — otherwise
/// the trigger would flash an empty state during every launch.
pub(in crate::app) fn picker_has_no_providers(
    probes: &[ProviderProbe],
    disabled_providers: &[ProviderKind],
    locked_provider: Option<ProviderKind>,
    detection_settled: bool,
) -> bool {
    detection_settled
        && !ProviderKind::ALL
            .into_iter()
            .any(|kind| picker_has_provider(probes, disabled_providers, locked_provider, kind))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) enum ModelPickerRow {
    Header(Option<ProviderKind>),
    Model {
        provider: ProviderKind,
        model: ProviderModel,
        in_favorites: bool,
    },
}

impl ModelPickerRow {
    pub(in crate::app) fn is_model(&self) -> bool {
        matches!(self, Self::Model { .. })
    }

    pub(in crate::app) fn same_model(&self, other: &Self) -> bool {
        matches!((self, other), (
            Self::Model { provider, model, .. },
            Self::Model { provider: other_provider, model: other_model, .. }
        ) if provider == other_provider && model.id == other_model.id)
    }

    pub(in crate::app) fn same_model_row(&self, other: &Self) -> bool {
        self.same_model(other)
            && matches!((self, other), (
            Self::Model { in_favorites, .. },
            Self::Model { in_favorites: other_favorites, .. }
        ) if in_favorites == other_favorites)
    }
}

/// One ordering for rendering and keyboard selection, including group headings.
pub(in crate::app) fn visible_picker_rows(
    probes: &[ProviderProbe],
    favorites: &[FavoriteModel],
    disabled_providers: &[ProviderKind],
    locked_provider: Option<ProviderKind>,
    normalized_query: &str,
) -> Vec<ModelPickerRow> {
    let models = ProviderKind::ALL
        .into_iter()
        .filter(|kind| picker_has_provider(probes, disabled_providers, locked_provider, *kind))
        .filter(|kind| locked_provider.is_none() || locked_provider == Some(*kind))
        .flat_map(|kind| {
            probes
                .iter()
                .filter(move |probe| probe.installed && probe.provider == kind)
                .flat_map(move |probe| probe.models.iter().map(move |model| (kind, model)))
        })
        .filter(|(kind, model)| {
            if normalized_query.is_empty() {
                return true;
            }
            let searchable = format!(
                "{} {} {} {}",
                model.name,
                model.id,
                kind.short_name(),
                model.sub_provider.as_deref().unwrap_or("")
            )
            .to_ascii_lowercase();
            normalized_query
                .split_whitespace()
                .all(|token| searchable.contains(token))
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for favorite in favorites {
        if let Some((provider, model)) = models
            .iter()
            .find(|(kind, model)| *kind == favorite.provider && model.id == favorite.model)
        {
            if rows.is_empty() {
                rows.push(ModelPickerRow::Header(None));
            }
            rows.push(ModelPickerRow::Model {
                provider: *provider,
                model: (*model).clone(),
                in_favorites: true,
            });
        }
    }
    let mut previous_provider = None;
    for (provider, model) in models {
        if previous_provider != Some(provider) {
            rows.push(ModelPickerRow::Header(Some(provider)));
            previous_provider = Some(provider);
        }
        rows.push(ModelPickerRow::Model {
            provider,
            model: model.clone(),
            in_favorites: false,
        });
    }
    rows
}

pub(in crate::app) fn next_model_picker_highlight(
    current: Option<usize>,
    rows: &[ModelPickerRow],
    key: &str,
) -> Option<usize> {
    let indexes = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.is_model().then_some(index))
        .collect::<Vec<_>>();
    let current =
        current.and_then(|index| indexes.iter().position(|candidate| *candidate == index));
    let next = match key {
        "home" => (!indexes.is_empty()).then_some(0),
        "end" => indexes.len().checked_sub(1),
        _ => crate::ui::primitives::navigation::next_picker_highlight(current, indexes.len(), key),
    }?;
    indexes.get(next).copied()
}
