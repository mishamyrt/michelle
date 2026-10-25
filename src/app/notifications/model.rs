//! Notification lifetime, hover pauses and copied-control feedback.
use super::*;

const DEFAULT_TOAST_DURATION: Duration = Duration::from_secs(5);
const MINIMUM_TOAST_RESUME_DURATION: Duration = Duration::from_millis(800);
#[derive(Debug)]
pub(in crate::app) struct ToastState {
    pub(in crate::app) message: String,
    pub(in crate::app) tone: ToastTone,
    pub(in crate::app) id: u64,
    pub(in crate::app) timer_generation: u64,
    pub(in crate::app) duration_remaining: Duration,
    pub(in crate::app) timer_started: Option<Instant>,
    pub(in crate::app) hovered: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ToastTone {
    Alert,
    Success,
}

pub(in crate::app) fn paused_toast_duration(remaining: Duration, elapsed: Duration) -> Duration {
    remaining
        .saturating_sub(elapsed)
        .max(MINIMUM_TOAST_RESUME_DURATION)
}

pub(in crate::app) struct NotificationsModel {
    /// Successful background turns the user has not opened yet.
    pub(in crate::app) unseen_completions: HashSet<Uuid>,
    pub(in crate::app) toast: Option<ToastState>,
    pub(in crate::app) toast_generation: u64,
    pub(in crate::app) copied_control_feedback: HashMap<String, u64>,
    pub(in crate::app) copied_control_generation: u64,
}

impl NotificationsModel {
    pub(in crate::app) fn new(startup_toast: Option<String>) -> Self {
        Self {
            unseen_completions: HashSet::new(),
            toast: startup_toast.map(|message| ToastState {
                message,
                tone: ToastTone::Alert,
                id: 0,
                timer_generation: 0,
                duration_remaining: DEFAULT_TOAST_DURATION,
                timer_started: None,
                hovered: false,
            }),
            toast_generation: 0,
            copied_control_feedback: HashMap::new(),
            copied_control_generation: 0,
        }
    }

    pub(in crate::app) fn record_session_completion(
        &mut self,
        session_id: Uuid,
        selected_session: Option<Uuid>,
        success: bool,
    ) {
        if success && selected_session != Some(session_id) {
            self.unseen_completions.insert(session_id);
        } else {
            self.clear_session_completion(session_id);
        }
    }

    pub(in crate::app) fn clear_session_completion(&mut self, session_id: Uuid) {
        self.unseen_completions.remove(&session_id);
    }
}

impl Michelle {
    pub(in crate::app) fn show_toast(&mut self, message: impl Into<String>) {
        self.show_toast_with_tone(message, ToastTone::Alert);
    }

    pub(in crate::app) fn show_success_toast(&mut self, message: impl Into<String>) {
        self.show_toast_with_tone(message, ToastTone::Success);
    }

    fn show_toast_with_tone(&mut self, message: impl Into<String>, tone: ToastTone) {
        self.notifications_ui.dismiss_focus = None;
        self.notifications_ui
            .selection
            .selection
            .borrow_mut()
            .clear();
        self.notifications_ui
            .selection
            .registry
            .borrow_mut()
            .clear();
        self.notifications.toast_generation = self.notifications.toast_generation.wrapping_add(1);
        self.notifications.toast = Some(ToastState {
            message: message.into(),
            tone,
            id: self.notifications.toast_generation,
            timer_generation: self.notifications.toast_generation,
            duration_remaining: DEFAULT_TOAST_DURATION,
            timer_started: None,
            hovered: false,
        });
    }

    pub(in crate::app) fn hide_toast(&mut self) {
        if self.notifications.toast.take().is_some() {
            self.notifications_ui.dismiss_focus = None;
            self.notifications_ui
                .selection
                .selection
                .borrow_mut()
                .clear();
            self.notifications_ui
                .selection
                .registry
                .borrow_mut()
                .clear();
            // Detached timers are deliberately cheap, but their generation
            // must stop them from dismissing a newer toast.
            self.notifications.toast_generation =
                self.notifications.toast_generation.wrapping_add(1);
        }
    }

    pub(in crate::app) fn start_toast_dismiss_timer(&mut self, cx: &mut Context<Self>) {
        let Some(toast) = self.notifications.toast.as_mut() else {
            return;
        };
        if toast.hovered || toast.timer_started.is_some() {
            return;
        }

        let duration = toast.duration_remaining;
        let generation = toast.timer_generation;
        toast.timer_started = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .notifications
                    .toast
                    .as_ref()
                    .is_some_and(|toast| toast.timer_generation == generation && !toast.hovered)
                {
                    this.hide_toast();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(in crate::app) fn set_toast_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let Some(toast) = self.notifications.toast.as_ref() else {
            return;
        };
        if toast.hovered == hovered {
            return;
        }

        self.notifications.toast_generation = self.notifications.toast_generation.wrapping_add(1);
        let generation = self.notifications.toast_generation;
        let toast = self
            .notifications
            .toast
            .as_mut()
            .expect("toast checked above");
        toast.timer_generation = generation;
        toast.hovered = hovered;
        if hovered {
            if let Some(started) = toast.timer_started.take() {
                toast.duration_remaining =
                    paused_toast_duration(toast.duration_remaining, started.elapsed());
            }
        } else {
            toast.timer_started = None;
            self.start_toast_dismiss_timer(cx);
        }
    }
    pub(in crate::app) fn control_was_copied(&self, control_id: &str) -> bool {
        self.notifications
            .copied_control_feedback
            .contains_key(control_id)
    }

    pub(in crate::app) fn show_control_copied(
        &mut self,
        control_id: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        let control_id = control_id.into();
        self.notifications.copied_control_generation =
            self.notifications.copied_control_generation.wrapping_add(1);
        let generation = self.notifications.copied_control_generation;
        self.notifications
            .copied_control_feedback
            .insert(control_id.clone(), generation);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                if this.notifications.copied_control_feedback.get(&control_id) == Some(&generation)
                {
                    this.notifications
                        .copied_control_feedback
                        .remove(&control_id);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_completions_remain_until_the_corresponding_session_is_opened() {
        let mut notifications = NotificationsModel::new(None);
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let selected = Some(Uuid::from_u128(3));
        notifications.record_session_completion(first, selected, true);
        notifications.record_session_completion(second, selected, true);
        // Duplicate completion reports do not create multiple badges.
        notifications.record_session_completion(first, selected, true);
        assert_eq!(
            notifications.unseen_completions,
            HashSet::from([first, second])
        );

        notifications.clear_session_completion(first);
        assert_eq!(notifications.unseen_completions, HashSet::from([second]));
        notifications.clear_session_completion(second);
        assert!(notifications.unseen_completions.is_empty());
    }

    #[test]
    fn visible_or_failed_turns_do_not_leave_a_success_badge() {
        let mut notifications = NotificationsModel::new(None);
        let session = Uuid::from_u128(1);
        notifications.record_session_completion(session, Some(session), true);
        assert!(notifications.unseen_completions.is_empty());
        notifications.record_session_completion(session, None, false);
        assert!(notifications.unseen_completions.is_empty());

        notifications.record_session_completion(session, None, true);
        notifications.record_session_completion(session, None, false);
        assert!(notifications.unseen_completions.is_empty());
        notifications.record_session_completion(session, None, true);
        notifications.clear_session_completion(session);
        assert!(notifications.unseen_completions.is_empty());
    }
}
