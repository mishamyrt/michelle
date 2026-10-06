//! Account usage snapshots and refresh lifecycle, shared by Settings and the meter.
use super::*;

pub(in crate::app) struct PlanUsageState {
    pub(in crate::app) snapshots: HashMap<ProviderKind, crate::usage::PlanUsage>,
    pub(in crate::app) errors: HashMap<ProviderKind, String>,
    pub(in crate::app) tx: Sender<(
        ProviderKind,
        Result<Option<crate::usage::PlanUsage>, String>,
    )>,
    pub(in crate::app) events: Receiver<(
        ProviderKind,
        Result<Option<crate::usage::PlanUsage>, String>,
    )>,
    pub(in crate::app) pending: HashSet<ProviderKind>,
    pub(in crate::app) unconfigured: HashSet<ProviderKind>,
    pub(in crate::app) checked_at: HashMap<ProviderKind, Instant>,
    pub(in crate::app) stale: HashSet<ProviderKind>,
}

/// Providers with an account-level plan fetcher. Codex additionally refreshes
/// live from its own stream notifications.
pub(in crate::app) const PLAN_USAGE_PROVIDERS: [ProviderKind; 4] = [
    ProviderKind::Claude,
    ProviderKind::Codex,
    ProviderKind::OpenCode,
    ProviderKind::Grok,
];

/// Refresh cadences for the plan snapshots. Quota moves only when turns run,
/// so idle refreshes stay rare; a settled turn or a just-opened panel asks
/// sooner. Grok's fetch spawns a probe process, so its idle cadence is wider.
const PLAN_USAGE_REFRESH: Duration = Duration::from_secs(300);
const PLAN_USAGE_REFRESH_GROK: Duration = Duration::from_secs(600);
const PLAN_USAGE_REFRESH_STALE: Duration = Duration::from_secs(30);
const PLAN_USAGE_RETRY: Duration = Duration::from_secs(90);

impl Michelle {
    /// Start background fetches of any plan meters whose snapshot is due.
    /// The slow maintenance clock and explicit panel-open requests call this;
    /// guards keep it to one in-flight fetch per provider.
    pub(in crate::app) fn maybe_refresh_plan_usage(&mut self, cx: &mut Context<Self>) {
        // Disabling a provider only stops it backing new sessions; a session
        // already locked to it keeps running, and while one is selected its
        // usage panel still owes the account meters. Without this the panel
        // would show its loading skeleton forever: fetchable provider, no
        // snapshot, and no fetch ever allowed to start.
        let selected_provider = self.selected_session().map(|session| session.provider);
        for provider in PLAN_USAGE_PROVIDERS {
            if self.settings.plan.pending.contains(&provider)
                || (!self.provider_enabled(provider) && selected_provider != Some(provider))
            {
                continue;
            }
            let interval = if self.settings.plan.errors.contains_key(&provider) {
                PLAN_USAGE_RETRY
            } else if self.settings.plan.stale.contains(&provider) {
                PLAN_USAGE_REFRESH_STALE
            } else if provider == ProviderKind::Grok {
                PLAN_USAGE_REFRESH_GROK
            } else {
                PLAN_USAGE_REFRESH
            };
            if self
                .settings
                .plan
                .checked_at
                .get(&provider)
                .is_some_and(|checked| checked.elapsed() < interval)
            {
                continue;
            }
            self.settings.plan.pending.insert(provider);
            let tx = self.settings.plan.tx.clone();
            let event_wake = self.sessions.runtime.event_wake_tx.clone();
            let claude_version = self
                .settings
                .providers
                .versions
                .get(&ProviderKind::Claude)
                .cloned()
                .flatten();
            let binary_override = self.state.provider_binary_overrides.get(&provider).cloned();
            let daemon = self.daemon.client();
            cx.background_executor()
                .spawn(async move {
                    let result = match daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::FetchPlanUsage {
                            provider,
                            binary_override,
                            cli_version: claude_version,
                        },
                    ) {
                        Ok(michelle_client::ResponsePayload::PlanUsage { usage }) => Ok(usage),
                        Ok(_) => Err(anyhow::anyhow!(
                            "the daemon returned an invalid plan usage response"
                        )),
                        Err(error) => Err(error),
                    };
                    if tx
                        .send((provider, result.map_err(|error| format!("{error:#}"))))
                        .is_ok()
                    {
                        signal_event_pump(&event_wake);
                    }
                })
                .detach();
        }
    }

    pub(in crate::app) fn drain_plan_usage_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok((provider, result)) = self.settings.plan.events.try_recv() {
            self.settings.plan.pending.remove(&provider);
            self.settings.plan.stale.remove(&provider);
            self.settings
                .plan
                .checked_at
                .insert(provider, Instant::now());
            match result {
                Ok(Some(usage)) => {
                    changed |= self.settings.plan.snapshots.get(&provider) != Some(&usage)
                        || self.settings.plan.errors.contains_key(&provider)
                        || self.settings.plan.unconfigured.contains(&provider);
                    self.settings.plan.snapshots.insert(provider, usage);
                    self.settings.plan.errors.remove(&provider);
                    self.settings.plan.unconfigured.remove(&provider);
                }
                Ok(None) => {
                    let had_usage = self.settings.plan.snapshots.remove(&provider).is_some();
                    let had_error = self.settings.plan.errors.remove(&provider).is_some();
                    let newly_unconfigured = self.settings.plan.unconfigured.insert(provider);
                    changed |= had_usage || had_error || newly_unconfigured;
                }
                Err(error) => {
                    let was_unconfigured = self.settings.plan.unconfigured.remove(&provider);
                    changed |= self.settings.plan.errors.get(&provider) != Some(&error)
                        || was_unconfigured;
                    // Keep any previous snapshot; stale meters with reset
                    // times still self-correct visually.
                    self.settings.plan.errors.insert(provider, error);
                }
            }
        }
        changed
    }
}
