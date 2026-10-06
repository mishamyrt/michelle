//! Provider detection, model discovery and CLI version probes.
use super::*;

pub(in crate::app) struct ProviderCatalog {
    pub(in crate::app) probes: Vec<ProviderProbe>,
    pub(in crate::app) probe_tx: Sender<ProviderProbe>,
    pub(in crate::app) probe_events: Receiver<ProviderProbe>,
    pub(in crate::app) model_discoveries: HashSet<ProviderKind>,
    pub(in crate::app) model_discoveries_pending: HashSet<ProviderKind>,
    pub(in crate::app) versions: HashMap<ProviderKind, Option<String>>,
    pub(in crate::app) version_tx: Sender<(ProviderKind, Option<String>)>,
    pub(in crate::app) version_events: Receiver<(ProviderKind, Option<String>)>,
    pub(in crate::app) version_probes_pending: HashSet<ProviderKind>,
    pub(in crate::app) detection_tx: Sender<ProviderProbe>,
    pub(in crate::app) detection_events: Receiver<ProviderProbe>,
    pub(in crate::app) detection_remaining: usize,
    pub(in crate::app) detection_checked_at: Option<Instant>,
}

impl Michelle {
    pub(in crate::app) fn provider_probe(&self, provider: ProviderKind) -> Option<&ProviderProbe> {
        self.settings
            .providers
            .probes
            .iter()
            .find(|probe| probe.provider == provider)
    }

    fn request_provider_model_discovery(&mut self, provider: ProviderKind) {
        if !provider.supports_model_discovery()
            || self
                .settings
                .providers
                .model_discoveries
                .contains(&provider)
        {
            return;
        }
        let Some(probe) = self
            .provider_probe(provider)
            .filter(|probe| probe.installed)
            .cloned()
        else {
            return;
        };
        self.settings.providers.model_discoveries.insert(provider);
        self.settings
            .providers
            .model_discoveries_pending
            .insert(provider);
        let provider_probe_tx = self.settings.providers.probe_tx.clone();
        let event_wake = self.sessions.runtime.event_wake_tx.clone();
        let daemon = self.daemon.client();
        let binary_override = self.state.provider_binary_overrides.get(&provider).cloned();
        if std::thread::Builder::new()
            .name(format!("michelle-{}-model-discovery", provider.id()))
            .spawn(move || {
                let discovered = match daemon.request(
                    Uuid::nil(),
                    Uuid::nil(),
                    michelle_client::Command::ProbeProvider {
                        provider,
                        binary_override,
                        discover_models: true,
                        probe_version: false,
                    },
                ) {
                    Ok(michelle_client::ResponsePayload::ProviderProbe { probe, .. }) => probe,
                    _ => probe,
                };
                if provider_probe_tx.send(discovered).is_ok() {
                    signal_event_pump(&event_wake);
                }
            })
            .is_err()
        {
            self.settings.providers.model_discoveries.remove(&provider);
            self.settings
                .providers
                .model_discoveries_pending
                .remove(&provider);
        }
    }

    /// Re-run one provider's model-owned catalog discovery, for selectors whose
    /// contents can change while Michelle stays open — models the user just
    /// authored in a provider's config, or DeepSeek's custom agent presets.
    /// The stale catalog stays on screen until the fresh probe lands, so an
    /// open menu never blanks into a loading state while it refreshes.
    pub(in crate::app) fn refresh_provider_model_discovery(&mut self, provider: ProviderKind) {
        if self
            .settings
            .providers
            .model_discoveries_pending
            .contains(&provider)
        {
            return;
        }
        self.settings.providers.model_discoveries.remove(&provider);
        self.request_provider_model_discovery(provider);
    }

    /// Ask every installed CLI for its version, one short-lived subprocess per
    /// provider on its own thread. Answers land in `provider_versions` through
    /// the drain loop; render reads only that map.
    fn request_provider_version_probes(&mut self) {
        let targets = self
            .settings
            .providers
            .probes
            .iter()
            .filter(|probe| probe.installed)
            .map(|probe| probe.provider)
            .collect::<Vec<_>>();
        for provider in targets {
            if !self
                .settings
                .providers
                .version_probes_pending
                .insert(provider)
            {
                continue;
            }
            let provider_version_tx = self.settings.providers.version_tx.clone();
            let event_wake = self.sessions.runtime.event_wake_tx.clone();
            let daemon = self.daemon.client();
            let binary_override = self.state.provider_binary_overrides.get(&provider).cloned();
            if std::thread::Builder::new()
                .name(format!("michelle-{}-version-probe", provider.id()))
                .spawn(move || {
                    let version = match daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::ProbeProvider {
                            provider,
                            binary_override,
                            discover_models: false,
                            probe_version: true,
                        },
                    ) {
                        Ok(michelle_client::ResponsePayload::ProviderProbe { version, .. }) => {
                            version
                        }
                        _ => None,
                    };
                    if provider_version_tx.send((provider, version)).is_ok() {
                        signal_event_pump(&event_wake);
                    }
                })
                .is_err()
            {
                self.settings
                    .providers
                    .version_probes_pending
                    .remove(&provider);
            }
        }
    }

    pub(in crate::app) fn drain_provider_version_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok((provider, version)) = self.settings.providers.version_events.try_recv() {
            self.settings
                .providers
                .version_probes_pending
                .remove(&provider);
            self.settings.providers.versions.insert(provider, version);
            changed = true;
        }
        changed
    }

    /// Re-detect provider CLIs off-thread — every provider for the Providers
    /// page's refresh, or one whose binary override just changed. Also re-runs
    /// model discovery and version probes for whatever the detection finds
    /// installed.
    pub(in crate::app) fn refresh_provider_detection(&mut self, scope: Option<ProviderKind>) {
        if self.settings.providers.detection_remaining > 0 {
            return;
        }
        let providers = match scope {
            Some(provider) => vec![provider],
            None => ProviderKind::ALL.to_vec(),
        };
        self.settings.providers.detection_remaining = providers.len();
        let overrides = self.state.provider_binary_overrides.clone();
        let provider_detection_tx = self.settings.providers.detection_tx.clone();
        let event_wake = self.sessions.runtime.event_wake_tx.clone();
        let detect_providers = providers.clone();
        let daemon = self.daemon.client();
        if std::thread::Builder::new()
            .name("michelle-provider-detection".into())
            .spawn(move || {
                for provider in detect_providers {
                    let response = daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::ProbeProvider {
                            provider,
                            binary_override: overrides.get(&provider).cloned(),
                            discover_models: false,
                            probe_version: false,
                        },
                    );
                    let probe = match response {
                        Ok(michelle_client::ResponsePayload::ProviderProbe { probe, .. }) => probe,
                        _ => ProviderProbe {
                            provider,
                            installed: false,
                            path: None,
                            models: crate::model_catalog::fallback_models(provider),
                            agent_presets: crate::model_catalog::fallback_agent_presets(provider),
                        },
                    };
                    if provider_detection_tx.send(probe).is_ok() {
                        signal_event_pump(&event_wake);
                    }
                }
            })
            .is_err()
        {
            self.settings.providers.detection_remaining = 0;
            return;
        }
        // A refresh means "re-check everything about these providers":
        // clearing the per-launch guard lets each one's catalog discovery run
        // again as its detection lands below.
        for provider in providers {
            self.settings.providers.model_discoveries.remove(&provider);
        }
    }

    pub(in crate::app) fn drain_provider_detection_events(&mut self) -> bool {
        let mut changed = false;
        let mut installed_providers = Vec::new();
        while let Ok(probe) = self.settings.providers.detection_events.try_recv() {
            let provider = probe.provider;
            let installed = probe.installed;
            self.settings.providers.detection_remaining = self
                .settings
                .providers
                .detection_remaining
                .saturating_sub(1);
            if self.settings.providers.detection_remaining == 0 {
                self.settings.providers.detection_checked_at = Some(Instant::now());
            }
            if let Some(existing) = self
                .settings
                .providers
                .probes
                .iter_mut()
                .find(|existing| existing.provider == provider)
            {
                if self
                    .settings
                    .providers
                    .model_discoveries_pending
                    .contains(&provider)
                {
                    // A manual refresh may overlap an older live discovery.
                    // Keep that newer catalog while still accepting PATH
                    // detection from this response.
                    existing.installed = probe.installed;
                    existing.path = probe.path;
                } else {
                    *existing = probe;
                }
            } else {
                self.settings.providers.probes.push(probe);
            }
            if installed {
                installed_providers.push(provider);
            } else {
                self.settings.providers.versions.remove(&provider);
            }
            changed = true;
        }
        for provider in installed_providers {
            self.request_provider_model_discovery(provider);
        }
        if changed {
            self.request_provider_version_probes();
        }
        changed
    }

    pub(in crate::app) fn drain_provider_probe_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok(probe) = self.settings.providers.probe_events.try_recv() {
            self.settings
                .providers
                .model_discoveries_pending
                .remove(&probe.provider);
            if let Some(existing) = self
                .settings
                .providers
                .probes
                .iter_mut()
                .find(|existing| existing.provider == probe.provider)
            {
                *existing = probe;
            } else {
                self.settings.providers.probes.push(probe);
            }
            changed = true;
        }
        changed
    }

    /// Whether the provider can back a new session: installed and not switched
    /// off in the Providers settings.
    pub(in crate::app) fn provider_enabled(&self, provider: ProviderKind) -> bool {
        !self.state.disabled_providers.contains(&provider)
            && self
                .provider_probe(provider)
                .is_some_and(|probe| probe.installed)
    }
}
