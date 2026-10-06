//! Historical usage data and scan lifecycle, independent of page interaction state.
use super::*;

const USAGE_RESCAN_AFTER: Duration = Duration::from_secs(120);

#[derive(Default)]
pub(in crate::app) struct UsageModel {
    pub(in crate::app) history: Option<UsageHistory>,
    pub(in crate::app) pending_for: Option<UsageWindow>,
    pub(in crate::app) generation: u64,
    scanned_at: Option<Instant>,
    error: Option<String>,
}

impl UsageModel {
    fn begin_scan(&mut self, window: UsageWindow, force: bool) -> Option<u64> {
        let satisfied = self
            .history
            .as_ref()
            .is_some_and(|history| history.window == window)
            && self
                .scanned_at
                .is_some_and(|scanned| scanned.elapsed() < USAGE_RESCAN_AFTER);
        // Even a forced refresh coalesces with the same in-flight scan.
        if self.pending_for == Some(window) || (!force && satisfied) {
            return None;
        }
        self.pending_for = Some(window);
        self.generation += 1;
        Some(self.generation)
    }

    fn finish_scan(&mut self, generation: u64, result: anyhow::Result<UsageHistory>) -> bool {
        if self.generation != generation {
            return false;
        }
        self.pending_for = None;
        match result {
            Ok(history) => {
                self.scanned_at = Some(Instant::now());
                self.history = Some(history);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }
}

impl Michelle {
    /// Start a background transcript scan unless a current-enough snapshot
    /// (or an in-flight scan for the same window) already covers it. `force`
    /// is the refresh button. Results from superseded scans are discarded by
    /// generation, so a window change mid-scan cannot land stale data.
    pub(in crate::app) fn ensure_usage_history(&mut self, force: bool, cx: &mut Context<Self>) {
        let window = self.effective_usage_window();
        let Some(generation) = self.usage.begin_scan(window, force) else {
            return;
        };
        let daemon = self.daemon.client();
        let project_roots: Vec<PathBuf> = self
            .state
            .projects
            .iter()
            .map(|project| project.path.clone())
            .collect();
        cx.spawn(async move |this, cx| {
            let history = cx
                .background_executor()
                .spawn(async move {
                    match daemon.request(
                        Uuid::nil(),
                        Uuid::nil(),
                        michelle_client::Command::LoadUsageHistory {
                            window,
                            project_roots,
                        },
                    )? {
                        michelle_client::ResponsePayload::UsageHistory { history } => Ok(history),
                        _ => anyhow::bail!("the daemon returned an invalid usage response"),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.usage.finish_scan(generation, history) {
                    return;
                }
                // The day axis may have changed length after a scan.
                this.usage_ui.chart_hover = None;
                if let Some(error) = this.usage.error.clone() {
                    this.show_toast(error);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_coalesce_and_superseded_results_leave_current_request_intact() {
        let mut model = UsageModel::default();
        let first_window = UsageWindow::TrailingDays(30);
        let second_window = UsageWindow::TrailingDays(7);
        let first = model.begin_scan(first_window, false).unwrap();
        assert_eq!(model.begin_scan(first_window, true), None);
        let second = model.begin_scan(second_window, false).unwrap();
        assert!(!model.finish_scan(first, Err(anyhow::anyhow!("obsolete"))));
        assert_eq!(model.pending_for, Some(second_window));
        assert!(model.error.is_none());
        assert!(model.finish_scan(second, Err(anyhow::anyhow!("current"))));
        assert_eq!(model.pending_for, None);
        assert_eq!(model.error.as_deref(), Some("current"));
        assert!(model.begin_scan(second_window, false).is_some());
    }
}
