//! Goal operation state and session intentions.
use super::*;

pub(in crate::app) struct GoalModel {
    /// Goal operations accepted before the session's runtime exists. Goals
    /// attach to the provider thread, not to any turn, so `/goal` on a fresh
    /// task starts the provider and these drain once it installs.
    pub(in crate::app) pending_operations: HashMap<Uuid, Vec<crate::model::GoalOperation>>,
    /// Sessions whose runtime is being started by a goal operation rather
    /// than a submission. Submissions queue behind this instead of racing a
    /// second provider process into existence.
    pub(in crate::app) runtime_starts: HashSet<Uuid>,
    /// When each session's goal accounting was last reported. The chip adds
    /// the wall clock since then while an active goal's turn runs, so elapsed
    /// pursuit time ticks live the way the Codex CLI shows it.
    pub(in crate::app) observed_at: HashMap<Uuid, Instant>,
}

impl Michelle {
    /// Hand a goal operation to the session's runtime, starting one first
    /// when none exists yet. Goals attach to the provider thread, not to any
    /// turn — the Codex CLI opens its thread at launch, so `/goal` works
    /// there before the first message. Michelle starts providers lazily, so the
    /// goal path starts the runtime itself and the queued operations drain
    /// the moment it installs.
    pub(in crate::app) fn dispatch_goal_operation(
        &mut self,
        session_id: Uuid,
        operation: GoalOperation,
        cx: &mut Context<Self>,
    ) {
        self.record_goal_submission(session_id, &operation, cx);
        self.begin_goal_pursuit_turn(session_id, &operation, cx);
        if let Some(runtime) = self.sessions.runtime.runtimes.get(&session_id) {
            runtime.driver.goal(operation);
            return;
        }
        self.goal_model
            .pending_operations
            .entry(session_id)
            .or_default()
            .push(operation);
        self.start_goal_runtime(session_id, cx);
    }

    /// A submitted objective leaves a persistent transcript record — the
    /// centered pill a system message renders as — the way a submission
    /// leaves its user message. Pushed before the pursuit turn exists so it
    /// stays turn-less and survives an unwound pursuit.
    fn record_goal_submission(
        &mut self,
        session_id: Uuid,
        operation: &GoalOperation,
        cx: &mut Context<Self>,
    ) {
        let GoalOperation::Set {
            objective: Some(objective),
            ..
        } = operation
        else {
            return;
        };
        let Some(session) = self.state.session_mut(session_id) else {
            return;
        };
        session.set_title_from_prompt(objective);
        let notice = tr!("goal.set_notice", objective = notice_objective(objective));
        session.push_message(MessageRole::System, notice);
        session.updated_at = crate::model::unix_time();
        self.state.mark_session_dirty(session_id);
        cx.notify();
    }

    /// Activating a goal on an idle thread makes Codex pursue it right away
    /// (`apply_external_goal_set` → `continue_if_idle`), so begin its turn
    /// optimistically — exactly how a submission's turn begins at accept —
    /// instead of leaving the empty-task page up until the provider's start
    /// report arrives seconds later. The provider's `turn/started` confirms
    /// the turn; errors and a watchdog unwind an unconfirmed one.
    fn begin_goal_pursuit_turn(
        &mut self,
        session_id: Uuid,
        operation: &GoalOperation,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            operation,
            GoalOperation::Set {
                status: Some(ThreadGoalStatus::Active),
                ..
            }
        ) {
            return;
        }
        let Some(session) = self
            .state
            .session_mut(session_id)
            .filter(|session| session.active_turn_id().is_none() && !session.status.is_busy())
        else {
            return;
        };
        let turn_id = session.begin_provider_turn();
        session.status = SessionStatus::Connecting;
        self.state.mark_session_dirty(session_id);
        cx.notify();
        // Continuation can legitimately never come — an inherited deferral,
        // or a goal feature disabled provider-side. Do not let the working
        // indicator outlive that silence.
        cx.spawn(async move |michelle, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(30))
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                let stale = michelle
                    .state
                    .sessions
                    .iter()
                    .find(|session| session.id == session_id)
                    .is_some_and(|session| {
                        session.active_turn_id() == Some(turn_id)
                            && session.active_turn_is_unconfirmed_pursuit()
                    });
                if stale {
                    michelle.unwind_unconfirmed_pursuit_turn(session_id);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Remove an optimistic pursuit turn whose provider start never came,
    /// returning the session to rest. Confirmed turns and submissions are
    /// never touched: only a running provider turn without a user message
    /// and without a provider start report qualifies.
    pub(in crate::app) fn unwind_unconfirmed_pursuit_turn(&mut self, session_id: Uuid) {
        let Some(session) = self
            .state
            .session_mut(session_id)
            .filter(|session| session.active_turn_is_unconfirmed_pursuit())
        else {
            return;
        };
        if let Some(turn_id) = session.active_turn_id() {
            session.unwind_unstarted_turn(turn_id);
        }
        if session.status.is_busy() {
            session.status = SessionStatus::Idle;
        }
        self.state.mark_session_dirty(session_id);
    }

    /// Flush operations accepted before the runtime existed. Called after
    /// any runtime install so the goal lands on the thread that was started
    /// for it — whether the goal path or a racing submission started it.
    pub(in crate::app) fn drain_pending_goal_operations(&mut self, session_id: Uuid) {
        let Some(operations) = self.goal_model.pending_operations.remove(&session_id) else {
            return;
        };
        if let Some(runtime) = self.sessions.runtime.runtimes.get(&session_id) {
            for operation in operations {
                runtime.driver.goal(operation);
            }
        }
    }
}
/// Saving an objective keeps a resumable status but restarts a finished one,
/// mirroring Codex's `/goal edit` semantics.
pub(in crate::app) fn edited_goal_status(status: ThreadGoalStatus) -> ThreadGoalStatus {
    if status.is_terminal() {
        ThreadGoalStatus::Active
    } else {
        status
    }
}

impl Michelle {
    pub(in crate::app) fn save_goal_objective(
        &mut self,
        session_id: Uuid,
        objective: String,
        replace: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if objective.is_empty() {
            return false;
        }
        let current_status = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .and_then(|session| session.thread_goal.as_ref())
            .map(|goal| goal.status);
        let (status, replace) = match current_status {
            // Editing keeps a resumable status; finished goals restart.
            Some(status) if !replace => (edited_goal_status(status), false),
            // Replacing always pursues the new objective from scratch. A goal
            // that vanished since the dialog opened degrades to a plain set.
            Some(_) => (ThreadGoalStatus::Active, true),
            None => (ThreadGoalStatus::Active, false),
        };
        self.dispatch_goal_operation(
            session_id,
            GoalOperation::Set {
                objective: Some(objective),
                status: Some(status),
                replace,
            },
            cx,
        );
        true
    }
}
