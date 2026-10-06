//! Session operations for cancellation, permission responses and provider questions.
use super::*;

impl Michelle {
    pub(in crate::app) fn set_runtime_mode(&mut self, mode: RuntimeMode, cx: &mut Context<Self>) {
        let Some((session_id, session_changed)) = self
            .selected_session()
            .map(|session| (session.id, session.runtime_mode != mode))
        else {
            return;
        };
        let remembered_changed = self.state.last_runtime_mode != mode;
        if session_changed {
            self.selected_session_mut()
                .expect("selected session still exists")
                .runtime_mode = mode;
            self.apply_session_options(session_id, cx);
        }
        if session_changed || remembered_changed {
            self.state.last_runtime_mode = mode;
            self.save();
            cx.notify();
        }
    }

    pub(in crate::app) fn cancel_turn(&mut self, cx: &mut Context<Self>) {
        self.session_ui.escape_stop_confirmation.clear();
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        // Worktree/checkpoint preparation has no safe interrupt contract. The
        // composer deliberately shows a spinner rather than Stop until the
        // provider runtime exists, and the keyboard action follows the same
        // boundary.
        if self
            .sessions
            .runtime
            .submission_preparations
            .contains(&session_id)
        {
            return;
        }
        let retain_runtime = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .is_some_and(|session| retain_runtime_after_cancel(session.provider))
            || self.session_has_live_detached_work(session_id);
        // Goal operations queued behind a starting runtime would set the
        // objective after this stop and begin pursuing it; the user asked to
        // stop, so they leave with the turn.
        self.goal_model.pending_operations.remove(&session_id);
        let mut runtime = self.sessions.runtime.runtimes.remove(&session_id);
        if let Some(runtime) = runtime.as_ref() {
            runtime.driver.cancel();
            if retain_runtime {
                // A detached process keeps Codex's app-server resident, but
                // Computer Use descendants still belong to the cancelled turn.
                runtime.driver.cancel_computer_use();
            }
        }
        // Do not leave already-received text in the smoothing queue: once the
        // message is marked complete, a later delta would otherwise create a
        // second assistant bubble. Show the received portion immediately.
        // Buffered turn-completion events also must not start queued
        // follow-ups: the user asked to stop, not to continue.
        let mut keep_runtime = true;
        if let Some(runtime) = runtime.as_mut() {
            Self::collect_runtime_events(runtime);
            while let Some(event) = runtime.pending_events.pop_front() {
                keep_runtime &= self.handle_driver_event(session_id, runtime, event, false, cx);
                if !keep_runtime {
                    break;
                }
            }
        }
        self.sessions
            .runtime
            .pending_queue_drains
            .retain(|id| *id != session_id);
        let has_active_turn = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .and_then(AgentSession::active_turn_id)
            .is_some();
        let previous_kinds = has_active_turn
            .then(|| self.snapshot_selected_transcript_rows(session_id))
            .flatten();
        self.finish_streaming_assistant(session_id);
        self.complete_turn_blocks(session_id);
        self.settle_foreground_work(session_id, BackgroundWorkStatus::Stopped);
        if let Some(runtime) = runtime.as_mut() {
            runtime.stream_phase = None;
            runtime.pending_permission = None;
            runtime.pending_user_input = None;
            runtime.pending_computer_approval = None;
            runtime.computer_use_previews.clear();
        }
        if has_active_turn {
            let needs_fallback = !self.turn_has_assistant_message(session_id);
            if let Some(session) = self.state.session_mut(session_id) {
                session.status = SessionStatus::Idle;
                if needs_fallback {
                    session.push_message(MessageRole::Assistant, tr!("session.stopped"));
                }
                session.finish_active_turn(TurnStatus::Interrupted);
            }
        }
        if has_active_turn {
            self.capture_latest_turn_checkpoint_for(session_id);
            self.start_pending_checkpoint_captures(cx);
        }
        if let Some(previous_kinds) = previous_kinds.as_deref() {
            self.splice_active_transcript_rows_after_visibility_change(previous_kinds);
        }
        // A provider runtime owns its Michelle JavaScript REPL and Computer Use
        // descendants. Normally Stop closes that process tree and the next
        // prompt resumes the same provider thread with a fresh runtime. A
        // detached process or subagent is the exception: its provider must
        // remain resident so Michelle can keep observing and stopping it.
        if retain_runtime && keep_runtime {
            if let Some(runtime) = runtime.take() {
                self.sessions.runtime.runtimes.insert(session_id, runtime);
            }
        } else if let Some(runtime) = runtime {
            runtime.driver.close();
        }
        self.remeasure_transcript_tail();
        self.save();
        cx.notify();
    }

    pub(in crate::app) fn respond_permission(
        &mut self,
        request_id: String,
        option_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        if let Some(runtime) = self.sessions.runtime.runtimes.get_mut(&session_id) {
            runtime.driver.respond(request_id, option_id);
            runtime.pending_permission = None;
        }
        if let Some(session) = self.selected_session_mut() {
            session.status = SessionStatus::Working;
        }
        cx.notify();
    }

    pub(in crate::app) fn sync_user_input_answer(&mut self, cx: &mut Context<Self>) {
        let answer = self
            .selected_runtime()
            .and_then(|runtime| runtime.pending_user_input.as_ref())
            .and_then(|pending| {
                pending
                    .current_question()
                    .map(|question| (pending, question))
            })
            .and_then(|(pending, question)| pending.custom_answers.get(&question.id))
            .cloned()
            .unwrap_or_default();
        self.session_ui
            .user_input_answer
            .update(cx, |input, cx| input.set_content(answer, cx));
    }

    pub(in crate::app) fn update_user_input_custom_answer(
        &mut self,
        answer: impl AsRef<str>,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let Some(pending) = self
            .sessions
            .runtime
            .runtimes
            .get_mut(&session_id)
            .and_then(|runtime| runtime.pending_user_input.as_mut())
        else {
            return;
        };
        let Some(question_id) = pending
            .current_question()
            .map(|question| question.id.clone())
        else {
            return;
        };
        let answer = answer.as_ref().to_owned();
        if answer.trim().is_empty() {
            pending.custom_answers.remove(&question_id);
        } else {
            pending.custom_answers.insert(question_id.clone(), answer);
            pending.selections.remove(&question_id);
        }
        cx.notify();
    }

    pub(in crate::app) fn submit_user_input_custom_answer(
        &mut self,
        answer: String,
        cx: &mut Context<Self>,
    ) {
        if answer.trim().is_empty() {
            return;
        }
        self.update_user_input_custom_answer(answer, cx);
        self.advance_user_input(cx);
    }

    pub(in crate::app) fn select_user_input_option(
        &mut self,
        label: String,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let Some(pending) = self
            .sessions
            .runtime
            .runtimes
            .get_mut(&session_id)
            .and_then(|runtime| runtime.pending_user_input.as_mut())
        else {
            return;
        };
        let Some((question_id, multi_select)) = pending
            .current_question()
            .map(|question| (question.id.clone(), question.multi_select))
        else {
            return;
        };
        let selected = pending.selections.entry(question_id.clone()).or_default();
        if multi_select {
            if let Some(index) = selected.iter().position(|answer| answer == &label) {
                selected.remove(index);
            } else {
                selected.push(label);
            }
        } else {
            selected.clear();
            selected.push(label);
        }
        if selected.is_empty() {
            pending.selections.remove(&question_id);
        }
        pending.custom_answers.remove(&question_id);
        self.session_ui
            .user_input_answer
            .update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    pub(in crate::app) fn previous_user_input(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let Some(pending) = self
            .sessions
            .runtime
            .runtimes
            .get_mut(&session_id)
            .and_then(|runtime| runtime.pending_user_input.as_mut())
        else {
            return;
        };
        if pending.question_index == 0 {
            return;
        }
        pending.question_index -= 1;
        self.sync_user_input_answer(cx);
        cx.notify();
    }

    pub(in crate::app) fn advance_user_input(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let should_submit = {
            let Some(pending) = self
                .sessions
                .runtime
                .runtimes
                .get_mut(&session_id)
                .and_then(|runtime| runtime.pending_user_input.as_mut())
            else {
                return;
            };
            let Some(question) = pending.current_question() else {
                return;
            };
            let answered = pending
                .custom_answers
                .get(&question.id)
                .is_some_and(|answer| !answer.trim().is_empty())
                || pending
                    .selections
                    .get(&question.id)
                    .is_some_and(|answers| !answers.is_empty());
            if !answered {
                return;
            }
            if pending.question_index + 1 < pending.questions.len() {
                pending.question_index += 1;
                false
            } else {
                true
            }
        };

        if should_submit {
            let Some(runtime) = self.sessions.runtime.runtimes.get_mut(&session_id) else {
                return;
            };
            let Some(pending) = runtime.pending_user_input.take() else {
                return;
            };
            let answers = pending.answers();
            runtime
                .driver
                .respond_user_input(pending.request_id, answers);
            if let Some(session) = self.state.session_mut(session_id) {
                session.status = SessionStatus::Working;
            }
            self.session_ui
                .user_input_answer
                .update(cx, |input, cx| input.clear(cx));
        } else {
            self.sync_user_input_answer(cx);
        }
        cx.notify();
    }

    pub(in crate::app) fn respond_computer_permission(
        &mut self,
        decision: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.state.selected_session else {
            return;
        };
        let Some(mut runtime) = self.sessions.runtime.runtimes.remove(&session_id) else {
            return;
        };
        let Some(pending) = runtime.pending_computer_approval.take() else {
            self.sessions.runtime.runtimes.insert(session_id, runtime);
            return;
        };

        if decision == "deny" {
            runtime.driver.reject_computer_tool(
                pending.request,
                "The user denied control of this app.".into(),
            );
        } else {
            let key = pending.target.grant_key();
            runtime.computer_session_grants.insert(key);
            if decision == "always" && pending.target.persistable() {
                let grant = crate::computer_use::ComputerAppGrant {
                    bundle_id: pending.target.bundle_id.clone(),
                    app_name: pending.target.app_name.clone(),
                };
                if !self
                    .state
                    .computer_use_allowed_apps
                    .iter()
                    .any(|existing| existing.key() == grant.key())
                {
                    self.state.computer_use_allowed_apps.push(grant);
                    self.save();
                }
            }
            runtime.driver.run_computer_tool(pending.request);
        }
        if let Some(session) = self.state.session_mut(session_id) {
            session.status = SessionStatus::Working;
        }
        self.sessions.runtime.runtimes.insert(session_id, runtime);
        cx.notify();
    }

    pub(in crate::app) fn bring_computer_use_to_front(
        &mut self,
        window_id: u64,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self
            .state
            .selected_session
            .and_then(|session_id| self.sessions.runtime.runtimes.get_mut(&session_id))
            && let Some(index) = runtime.computer_use_previews.iter().position(|preview| {
                preview
                    .target
                    .as_ref()
                    .is_some_and(|target| target.window_id == window_id)
            })
        {
            let preview = runtime.computer_use_previews.remove(index);
            runtime.computer_use_previews.push(preview);
        }
        cx.notify();
    }

    pub(in crate::app) fn dismiss_computer_use(&mut self, window_id: u64, cx: &mut Context<Self>) {
        if let Some(runtime) = self
            .state
            .selected_session
            .and_then(|session_id| self.sessions.runtime.runtimes.get_mut(&session_id))
        {
            if let Some(preview) = runtime.computer_use_previews.iter_mut().find(|preview| {
                preview
                    .target
                    .as_ref()
                    .is_some_and(|target| target.window_id == window_id)
            }) {
                // Keep the hidden entry until the turn ends so the next
                // screenshot cannot reopen a preview the user just closed.
                preview.visible = false;
                preview.decode_task = None;
                preview.frames = Default::default();
            }
        }
        cx.notify();
    }
}

pub(in crate::app) struct SessionUi {
    pub(in crate::app) user_input_answer: Entity<TextInput>,
    /// First Escape press for the current turn. A matching second press stops
    /// the response; otherwise this returns to the ordinary Stop icon after a
    /// short timeout.
    pub(in crate::app) escape_stop_confirmation: EscapeStopConfirmation,
    /// Window-relative PiP position, independent of incoming preview frames.
    pub(in crate::app) computer_use_preview_position: Option<gpui::Point<Pixels>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct EscapeStopTarget {
    pub(in crate::app) session_id: Uuid,
    pub(in crate::app) turn_id: Option<Uuid>,
}

impl EscapeStopTarget {
    pub(in crate::app) fn for_session(session: &AgentSession) -> Self {
        Self {
            session_id: session.id,
            turn_id: session.active_turn_id(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum EscapeStopPress {
    Arm(EscapeStopArm),
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct EscapeStopArm {
    pub(in crate::app) target: EscapeStopTarget,
    pub(in crate::app) expires_at: Instant,
}

#[derive(Default)]
pub(in crate::app) struct EscapeStopConfirmation {
    pub(in crate::app) arm: Option<EscapeStopArm>,
}

impl EscapeStopConfirmation {
    pub(in crate::app) fn press(
        &mut self,
        target: EscapeStopTarget,
        now: Instant,
    ) -> EscapeStopPress {
        if self
            .arm
            .is_some_and(|arm| arm.target == target && now < arm.expires_at)
        {
            self.arm = None;
            EscapeStopPress::Stop
        } else {
            let arm = EscapeStopArm {
                target,
                expires_at: now + ESCAPE_STOP_CONFIRMATION_TIMEOUT,
            };
            self.arm = Some(arm);
            EscapeStopPress::Arm(arm)
        }
    }

    pub(in crate::app) fn is_armed_for(&self, target: EscapeStopTarget, now: Instant) -> bool {
        self.arm
            .is_some_and(|arm| arm.target == target && now < arm.expires_at)
    }

    pub(in crate::app) fn expire(&mut self, arm: EscapeStopArm) -> bool {
        if self.arm != Some(arm) {
            return false;
        }
        self.arm = None;
        true
    }

    pub(in crate::app) fn clear(&mut self) {
        self.arm = None;
    }
}

#[derive(Clone)]
pub(in crate::app) struct PendingUserInput {
    pub(in crate::app) request_id: String,
    pub(in crate::app) questions: Vec<UserInputQuestion>,
    pub(in crate::app) question_index: usize,
    pub(in crate::app) selections: HashMap<String, Vec<String>>,
    pub(in crate::app) custom_answers: HashMap<String, String>,
}

impl PendingUserInput {
    pub(in crate::app) fn new(request_id: String, questions: Vec<UserInputQuestion>) -> Self {
        Self {
            request_id,
            questions,
            question_index: 0,
            selections: HashMap::new(),
            custom_answers: HashMap::new(),
        }
    }

    pub(in crate::app) fn current_question(&self) -> Option<&UserInputQuestion> {
        self.questions.get(self.question_index)
    }

    pub(in crate::app) fn answers(&self) -> Vec<UserInputAnswer> {
        self.questions
            .iter()
            .map(|question| {
                let custom = self
                    .custom_answers
                    .get(&question.id)
                    .map(|answer| answer.trim())
                    .filter(|answer| !answer.is_empty());
                UserInputAnswer {
                    question_id: question.id.clone(),
                    answers: custom.map_or_else(
                        || {
                            self.selections
                                .get(&question.id)
                                .cloned()
                                .unwrap_or_default()
                        },
                        |answer| vec![answer.to_owned()],
                    ),
                }
            })
            .collect()
    }
}

pub(in crate::app) struct ComputerUsePreview {
    pub(in crate::app) target: Option<ComputerTarget>,
    pub(in crate::app) phase: ComputerUsePhase,
    pub(in crate::app) visible: bool,
    pub(in crate::app) frames:
        crate::computer_use::PreviewFrames<crate::computer_use::PreviewImage>,
    pub(in crate::app) decode_task: Option<gpui::Task<()>>,
}
