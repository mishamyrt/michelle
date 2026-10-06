//! Draft/attachment state and submission preparation for Composer.
use super::*;

pub(in crate::app) struct ComposerModel {
    pub(in crate::app) drafts: ComposerDrafts,
    pub(in crate::app) draft_store: ComposerDraftStore,
    pub(in crate::app) draft_save_generation: u64,
    pub(in crate::app) attachments: Vec<ComposerAttachment>,
    pub(in crate::app) sources: sources::ComposerSources,
}

#[derive(Clone, Debug)]
pub(in crate::app) struct ComposerAttachment {
    /// Materialized path on the daemon host. This is the only path sent to a
    /// provider or persisted with a task.
    pub(in crate::app) path: PathBuf,
    /// Ephemeral decoded client image used only for an immediate preview after
    /// upload. It is never persisted or sent to the daemon.
    pub(in crate::app) client_preview_image: Option<Arc<gpui::Image>>,
    /// What the submission sends: relative to the project root when the file
    /// is inside it, absolute otherwise, directories with a trailing slash.
    pub(in crate::app) mention: String,
    /// Basename drawn on the chip.
    pub(in crate::app) name: SharedString,
    pub(in crate::app) is_dir: bool,
    /// Whether the chip shows a thumbnail. Decided by extension at drop time
    /// so render never touches the filesystem.
    pub(in crate::app) is_image: bool,
    /// Daemon-issued durable reference retained by task persistence.
    pub(in crate::app) blob_reference: Option<String>,
}

/// One accepted composer submission. `prompt` preserves the composer and
/// transcript syntax; provider-specific command syntax resolves only at the
/// transport boundary. Presentation metadata keeps appended attachment
/// mentions out of the user bubble.
#[derive(Clone, Debug)]
pub(in crate::app) struct ComposerSubmission {
    pub(in crate::app) prompt: String,
    pub(in crate::app) display_content: Option<String>,
    pub(in crate::app) attachments: Vec<MessageAttachment>,
}

impl ComposerSubmission {
    pub(in crate::app) fn plain(prompt: String) -> Self {
        Self {
            prompt,
            display_content: None,
            attachments: Vec::new(),
        }
    }

    pub(in crate::app) fn into_queued_message(self) -> QueuedMessage {
        QueuedMessage::with_presentation(self.prompt, self.display_content, self.attachments)
    }

    pub(in crate::app) fn from_queued_message(message: QueuedMessage) -> Self {
        Self {
            prompt: message.content,
            display_content: message.display_content,
            attachments: message.attachments,
        }
    }

    /// Human-facing task text for titles and generated worktree names. An
    /// attachment-only submission uses basenames instead of its transport
    /// paths; providers still receive `prompt` unchanged.
    pub(in crate::app) fn human_prompt(&self) -> String {
        let visible = self
            .display_content
            .as_deref()
            .unwrap_or(&self.prompt)
            .trim();
        if !visible.is_empty() {
            return visible.to_owned();
        }
        if !self.attachments.is_empty() {
            return self
                .attachments
                .iter()
                .map(|attachment| attachment.name.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        }
        self.prompt.trim().to_owned()
    }
}

impl Michelle {
    /// The text and attachment presentation accepted from the composer. The
    /// stored prompt keeps its `@` mentions and visible command syntax, while
    /// sent-message UI uses `display_content` and retained attachment metadata.
    pub(in crate::app) fn submission_with_attachments(
        &mut self,
        prompt: &str,
        cx: &mut Context<Self>,
    ) -> Option<ComposerSubmission> {
        if self
            .composer_ui
            .input
            .read(cx)
            .has_pending_attachment_pastes()
        {
            return None;
        }
        if self.execute_local_composer_command(prompt, cx) {
            return None;
        }
        // Nothing installed or switched on can run this. Refuse before the
        // draft is consumed, so the text and its attachments survive until a
        // provider is available — every send route lands here, so `enter`,
        // the button, and steering are all covered by this one check.
        if self.model_picker_has_no_providers() {
            return None;
        }
        for attachment in &self.composer_model.attachments {
            if let (Some(reference), Some(image)) = (
                attachment.blob_reference.as_ref(),
                attachment.client_preview_image.as_ref(),
            ) {
                self.image_model
                    .retain_preview(reference.clone(), image.clone());
            }
        }
        let attachments = self
            .composer_model
            .attachments
            .drain(..)
            .map(MessageAttachment::from)
            .collect::<Vec<_>>();
        let mentions = attachments
            .iter()
            .map(|attachment| attachment.mention.clone())
            .collect::<Vec<_>>();
        let submission = merged_submission(prompt, &mentions)?;
        let display_content = (!attachments.is_empty()).then(|| prompt.trim().to_owned());
        self.discard_current_composer_draft(cx);
        Some(ComposerSubmission {
            prompt: submission,
            display_content,
            attachments,
        })
    }

    pub(in crate::app) fn execute_local_composer_command(
        &mut self,
        prompt: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        self.execute_resume_composer_command(prompt, cx)
            || self.execute_fast_mode_toggle(prompt, cx)
            || self.execute_goal_composer_command(prompt, cx)
    }

    fn execute_resume_composer_command(&mut self, prompt: &str, cx: &mut Context<Self>) -> bool {
        if !crate::composer_complete::is_resume_submission(prompt) {
            return false;
        }
        self.composer_ui
            .input
            .update(cx, |input, cx| input.clear(cx));
        // Submission notifications already hold this entity mutably. Dispatch
        // after that effect returns so the window action can safely re-enter
        // Michelle and move focus into the Resume picker.
        cx.defer(|cx| cx.dispatch_action(&OpenResumePicker));
        true
    }

    /// Bridge Codex's native `/goal` command without starting a turn. Reads run
    /// against the session's cached goal; mutations go to the app-server and
    /// echo back as `GoalUpdated` events.
    fn execute_goal_composer_command(&mut self, prompt: &str, cx: &mut Context<Self>) -> bool {
        use crate::composer_complete::GoalCommand;
        use crate::model::{GoalOperation, ThreadGoalStatus};
        let Some((session_id, command, current_goal)) =
            self.selected_session().and_then(|session| {
                let command = crate::composer_complete::parse_goal_submission(
                    session.provider,
                    prompt,
                    &self.composer_model.sources.slash_command_index,
                )?;
                Some((session.id, command, session.thread_goal.clone()))
            })
        else {
            return false;
        };
        match command {
            GoalCommand::Show | GoalCommand::Edit => {
                self.request_goal_dialog(session_id, None, false, cx);
            }
            GoalCommand::Pause => {
                self.dispatch_goal_operation(
                    session_id,
                    GoalOperation::Set {
                        objective: None,
                        status: Some(ThreadGoalStatus::Paused),
                        replace: false,
                    },
                    cx,
                );
            }
            GoalCommand::Resume => {
                self.dispatch_goal_operation(
                    session_id,
                    GoalOperation::Set {
                        objective: None,
                        status: Some(ThreadGoalStatus::Active),
                        replace: false,
                    },
                    cx,
                );
            }
            GoalCommand::Clear => {
                self.dispatch_goal_operation(session_id, GoalOperation::Clear, cx);
            }
            GoalCommand::Set(objective) => match &current_goal {
                // Replacing unfinished work needs a look at what it replaces;
                // the dialog carries the confirmation.
                Some(goal) if !goal.status.is_terminal() => {
                    self.request_goal_dialog(session_id, Some(objective), true, cx);
                }
                Some(_) | None => {
                    self.dispatch_goal_operation(
                        session_id,
                        GoalOperation::Set {
                            objective: Some(objective),
                            status: Some(ThreadGoalStatus::Active),
                            replace: current_goal.is_some(),
                        },
                        cx,
                    );
                }
            },
        }
        self.composer_ui
            .input
            .update(cx, |input, cx| input.clear(cx));
        cx.notify();
        true
    }

    fn execute_fast_mode_toggle(&mut self, prompt: &str, cx: &mut Context<Self>) -> bool {
        let Some(next_tier) = self.selected_session().and_then(|session| {
            if !crate::composer_complete::is_fast_mode_toggle_submission(
                session.provider,
                prompt,
                &self.composer_model.sources.slash_command_index,
            ) {
                return None;
            }
            let model = self.model_metadata_for_session(session)?;
            crate::composer_complete::toggled_fast_service_tier(
                session.service_tier.as_deref(),
                &model.service_tiers,
            )
        }) else {
            return false;
        };
        let enabled = next_tier != "default";
        // Clearing emits an Edited event. Apply the tier afterward so any
        // draft refresh caused by that event cannot repaint the old choice.
        self.composer_ui
            .input
            .update(cx, |input, cx| input.clear(cx));
        self.set_service_tier(next_tier, cx);
        self.show_success_toast(tr!(if enabled {
            "commands.fast_enabled"
        } else {
            "commands.fast_disabled"
        }));
        true
    }

    pub(in crate::app) fn restore_composer_submission(
        &mut self,
        submission: ComposerSubmission,
        cx: &mut Context<Self>,
    ) {
        self.composer_model.attachments = submission
            .attachments
            .into_iter()
            .map(ComposerAttachment::from)
            .collect();
        let content = submission.display_content.unwrap_or(submission.prompt);
        self.composer_ui
            .input
            .update(cx, |input, cx| input.set_content(content, cx));
        self.schedule_composer_draft_save(cx);
        cx.notify();
    }
}
/// The prompt a submission sends: the typed text plus one `@` mention per
/// staged attachment, appended at the end the way T3 Code appends dropped
/// files. `None` means there is nothing to send.
pub(in crate::app) fn merged_submission(prompt: &str, mentions: &[String]) -> Option<String> {
    let mentions = mentions
        .iter()
        .map(|mention| format!("@{mention}"))
        .collect::<Vec<_>>()
        .join(" ");
    let prompt = prompt.trim();
    match (prompt.is_empty(), mentions.is_empty()) {
        (true, true) => None,
        (false, true) => Some(prompt.to_owned()),
        (true, false) => Some(mentions),
        (false, false) => Some(format!("{prompt} {mentions}")),
    }
}

impl Michelle {
    pub(in crate::app) fn handle_composer_event(
        &mut self,
        event: &ComposerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            ComposerEvent::Submit(prompt) => {
                if let Some(session_id) = self.selected_session().and_then(|session| {
                    self.sessions
                        .runtime
                        .response_fork_preparations
                        .contains_key(&session.id)
                        .then_some(session.id)
                }) {
                    self.defer_restore_composer_after_fork(session_id, prompt.clone(), cx);
                } else if let Some(submission) = self.submission_with_attachments(prompt, cx) {
                    self.submit_composer_submission(submission, cx);
                }
            }
            ComposerEvent::SubmitSteer(prompt) => {
                if let Some(session_id) = self.selected_session().and_then(|session| {
                    self.sessions
                        .runtime
                        .response_fork_preparations
                        .contains_key(&session.id)
                        .then_some(session.id)
                }) {
                    self.defer_restore_composer_after_fork(session_id, prompt.clone(), cx);
                } else if let Some(submission) = self.submission_with_attachments(prompt, cx) {
                    self.steer_composer_submission(submission, cx);
                }
            }
            ComposerEvent::SteerQueued => {
                // Staged attachments make this a real draft even when
                // the text field is empty. Preserve the shortcut's
                // previous no-op behavior until that draft is sent or
                // cleared.
                if self.composer_model.attachments.is_empty() {
                    self.steer_oldest_queued_message(cx);
                }
            }
            ComposerEvent::Edited => {
                self.schedule_composer_draft_save(cx);
                cx.notify();
            }
            ComposerEvent::Focus => {}
            ComposerEvent::BackspaceOnEmpty => {
                if self.composer_model.attachments.pop().is_some() {
                    self.schedule_composer_draft_save(cx);
                    cx.notify();
                }
            }
        }
    }
}
