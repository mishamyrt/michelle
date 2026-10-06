//! Follow-up queue operations and coordination with runtime/checkpoint completion.
use super::*;

impl Michelle {
    pub(in crate::app) fn defer_queue_drain(&mut self, session_id: Uuid) {
        if !self
            .sessions
            .runtime
            .pending_queue_drains
            .contains(&session_id)
        {
            self.sessions.runtime.pending_queue_drains.push(session_id);
        }
    }

    pub(in crate::app) fn enqueue_follow_up_submission(
        &mut self,
        session_id: Uuid,
        mut submission: ComposerSubmission,
        cx: &mut Context<Self>,
    ) {
        submission.prompt = submission.prompt.trim().to_owned();
        if submission.prompt.is_empty() {
            return;
        }
        if let Some(session) = self.state.session_mut(session_id) {
            session
                .queued_messages
                .push(submission.into_queued_message());
            session.updated_at = unix_time();
        }
        self.save();
        cx.notify();
    }

    pub(in crate::app) fn remove_queued_message(
        &mut self,
        session_id: Uuid,
        message_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if let Some(session) = self.state.session_mut(session_id) {
            session
                .queued_messages
                .retain(|message| message.id != message_id);
        }
        self.save();
        cx.notify();
    }

    /// Pop a queued message back into the composer so the user can edit and
    /// resubmit it.
    pub(in crate::app) fn edit_queued_message(
        &mut self,
        session_id: Uuid,
        message_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = self.state.session_mut(session_id).and_then(|session| {
            let index = session
                .queued_messages
                .iter()
                .position(|message| message.id == message_id)?;
            Some(session.queued_messages.remove(index))
        }) else {
            return;
        };
        self.restore_composer_submission(ComposerSubmission::from_queued_message(message), cx);
        let focus_handle = self.composer_focus(cx);
        window.focus(&focus_handle, cx);
        self.save();
        cx.notify();
    }

    /// Deliver a queued follow-up into the running turn right away instead of
    /// waiting for the turn to settle. Falls through the same paths as a
    /// composer steer: an idle session starts a fresh turn, an unsteerable
    /// one re-queues the message.
    pub(in crate::app) fn steer_queued_message(
        &mut self,
        session_id: Uuid,
        message_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = self.state.session_mut(session_id).and_then(|session| {
            let index = session
                .queued_messages
                .iter()
                .position(|message| message.id == message_id)?;
            Some(session.queued_messages.remove(index))
        }) else {
            return;
        };
        self.save();
        self.steer_composer_submission(ComposerSubmission::from_queued_message(message), cx);
    }

    /// Activate the same action as the oldest queued row's Steer control.
    /// When that control is unavailable, leave the queue untouched rather
    /// than removing and re-queueing its first message at the back.
    pub(in crate::app) fn steer_oldest_queued_message(&mut self, cx: &mut Context<Self>) {
        let Some((session_id, message_id)) = self.selected_session().and_then(|session| {
            if !self.session_can_steer(session) {
                return None;
            }
            Some((session.id, session.queued_messages.first()?.id))
        }) else {
            return;
        };
        self.steer_queued_message(session_id, message_id, cx);
    }

    /// Start the next queued follow-up as a fresh turn. Only called once a
    /// settled turn has been fully closed, so the session is Idle.
    pub(in crate::app) fn drain_queued_message(
        &mut self,
        session_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if self
            .sessions
            .runtime
            .response_fork_preparations
            .contains_key(&session_id)
        {
            return;
        }
        let Some(session) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
        else {
            return;
        };
        if session.is_busy()
            || session.queued_messages.is_empty()
            || self.ending_checkpoint_pending(session_id)
            // Messages parked behind a goal-initiated provider start stay
            // queued until that runtime installs.
            || self.goal_model.runtime_starts.contains(&session_id)
        {
            return;
        }
        let Some(message) = self
            .state
            .session_mut(session_id)
            .map(|session| session.queued_messages.remove(0))
        else {
            return;
        };
        self.submit_submission_for_session(
            session_id,
            ComposerSubmission::from_queued_message(message),
            cx,
        );
    }
}
