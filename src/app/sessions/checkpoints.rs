//! Background checkpoint capture and cached ref existence for transcript actions.
use super::*;

pub(in crate::app) struct CheckpointModel {
    pub(in crate::app) refs: RefCell<HashMap<(Uuid, usize), bool>>,
    pub(in crate::app) generation: Cell<u64>,
    pub(in crate::app) prefetch: Cell<Option<(Uuid, u64)>>,
    pub(in crate::app) pending: Vec<PendingCheckpointCapture>,
    pub(in crate::app) in_flight: HashSet<(Uuid, usize)>,
}

/// A turn whose checkpoint still has to be captured.
pub(in crate::app) struct PendingCheckpointCapture {
    pub(in crate::app) session_id: Uuid,
    pub(in crate::app) turn_count: usize,
    pub(in crate::app) project_path: PathBuf,
}

impl Michelle {
    fn checkpoint_capture_pending(&self, session_id: Uuid, turn_count: usize) -> bool {
        self.sessions
            .checkpoints
            .in_flight
            .contains(&(session_id, turn_count))
            || self
                .sessions
                .checkpoints
                .pending
                .iter()
                .any(|capture| capture.session_id == session_id && capture.turn_count == turn_count)
    }

    pub(in crate::app) fn ending_checkpoint_pending(&self, session_id: Uuid) -> bool {
        self.state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .and_then(|session| session.turns.last())
            .filter(|turn| turn.status != TurnStatus::Running)
            .is_some_and(|turn| self.checkpoint_capture_pending(session_id, turn.turn_count))
    }

    /// Queues the newest finished turn's checkpoint for capture.
    ///
    /// Bookkeeping only. The capture itself is upwards of ten `git`
    /// invocations, one of them a `git add -A` over the whole worktree, and the
    /// hottest caller is the driver-event drain that shares the UI thread with
    /// rendering — so the work belongs to
    /// [`Self::start_pending_checkpoint_captures`], which every caller that
    /// holds a `Context` runs straight after queueing.
    pub(in crate::app) fn capture_latest_turn_checkpoint_for(&mut self, session_id: Uuid) {
        let Some((session, turn_count)) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .and_then(|session| {
                session
                    .turns
                    .last()
                    .filter(|turn| turn.status != TurnStatus::Running)
                    .map(|turn| (session, turn.turn_count))
            })
        else {
            return;
        };
        if self.checkpoint_capture_pending(session_id, turn_count) {
            return;
        }
        let Some(project_path) = self
            .workspace_path_for_session(session)
            .map(std::path::Path::to_path_buf)
        else {
            return;
        };
        self.sessions
            .checkpoints
            .pending
            .push(PendingCheckpointCapture {
                session_id,
                turn_count,
                project_path,
            });
    }

    /// Runs queued turn checkpoints on the background executor.
    ///
    /// A capture lands a frame or many later, and the turn it belongs to may be
    /// gone by then, so the result is matched back by turn count rather than
    /// position. Nothing on screen waits for it: the transcript's rewind
    /// affordance appears when `invalidate_checkpoint_refs` prompts the next
    /// prefetch to notice the new ref.
    pub(in crate::app) fn start_pending_checkpoint_captures(&mut self, cx: &mut Context<Self>) {
        for request in std::mem::take(&mut self.sessions.checkpoints.pending) {
            let PendingCheckpointCapture {
                session_id,
                turn_count,
                project_path,
            } = request;
            if !self
                .sessions
                .checkpoints
                .in_flight
                .insert((session_id, turn_count))
            {
                continue;
            }
            let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
            cx.spawn(async move |michelle, cx| {
                let captured = cx
                    .background_executor()
                    .spawn({
                        let project_path = project_path.clone();
                        async move {
                            match workspace.request(
                                michelle_client::WorkspaceOperation::CaptureTurn {
                                    cwd: project_path,
                                    session_id,
                                    turn_count,
                                },
                            )? {
                                michelle_client::WorkspaceResult::Checkpoint { checkpoint } => {
                                    Ok(checkpoint)
                                }
                                _ => anyhow::bail!(
                                    "the daemon returned an invalid checkpoint response"
                                ),
                            }
                        }
                    })
                    .await;
                michelle
                    .update(cx, |michelle, cx| {
                        michelle
                            .sessions
                            .checkpoints
                            .in_flight
                            .remove(&(session_id, turn_count));
                        let selected = michelle.state.selected_session == Some(session_id);
                        if selected {
                            michelle.sync_transcript_rows();
                        }
                        let previous_kinds = if selected {
                            michelle.transcript_model.row_kinds.borrow().clone()
                        } else {
                            Vec::new()
                        };
                        let checkpoint = match captured {
                            Ok(checkpoint) => checkpoint,
                            Err(error) => {
                                michelle.show_toast(tr!(
                                    "errors.capture_turn_checkpoint",
                                    error = error
                                ));
                                Checkpoint {
                                    turn_count,
                                    git_ref: checkpoint::checkpoint_ref(session_id, turn_count),
                                    status: CheckpointStatus::Error,
                                    files: Vec::new(),
                                    additions: 0,
                                    deletions: 0,
                                    created_at: unix_time(),
                                }
                            }
                        };
                        michelle.invalidate_checkpoint_refs();
                        let mut attached_turn_id = None;
                        if let Some(session) = michelle.state.session_mut(session_id)
                            && let Some(turn) = session
                                .turns
                                .iter_mut()
                                .find(|turn| turn.turn_count == turn_count)
                        {
                            turn.checkpoint = Some(checkpoint);
                            attached_turn_id = Some(turn.id);
                        }
                        if let Some(turn_id) = attached_turn_id
                            && selected
                        {
                            // Reconcile a standalone card by row identity, then
                            // remeasure the terminal response when the card is
                            // hosted inline before its footer.
                            michelle
                                .splice_transcript_rows_after_visibility_change(&previous_kinds);
                            michelle.remeasure_changed_files(turn_id);
                        }
                        let resume_queue = michelle
                            .sessions
                            .runtime
                            .pending_queue_drains
                            .contains(&session_id);
                        if resume_queue {
                            michelle
                                .sessions
                                .runtime
                                .pending_queue_drains
                                .retain(|id| *id != session_id);
                            michelle.drain_queued_message(session_id, cx);
                        }
                        cx.notify();
                        if attached_turn_id.is_some() {
                            // Let the new transcript row paint before SQLite work.
                            // Without this save, a checkpoint that lands after the
                            // turn's final stream save can disappear on relaunch.
                            cx.spawn(async move |michelle, cx| {
                                cx.background_executor().timer(STREAM_FRAME_INTERVAL).await;
                                let _ = michelle.update(cx, |michelle, _| michelle.save());
                            })
                            .detach();
                        }
                    })
                    .ok();
            })
            .detach();
        }
    }

    /// Forget cached checkpoint-ref existence after refs changed. The next
    /// transcript frame schedules a fresh background prefetch.
    pub(in crate::app) fn invalidate_checkpoint_refs(&self) {
        self.sessions.checkpoints.refs.borrow_mut().clear();
        self.sessions
            .checkpoints
            .generation
            .set(self.sessions.checkpoints.generation.get().wrapping_add(1));
    }

    /// Resolve the selected session's checkpoint refs on the background
    /// executor — one `git for-each-ref` per session per invalidation — and
    /// cache which retained turn counts have one. The rewind affordance
    /// appears once the result lands and notifies.
    pub(in crate::app) fn prefetch_checkpoint_refs(&self, cx: &mut Context<Self>) {
        let Some(session) = self.selected_session() else {
            return;
        };
        let generation = self.sessions.checkpoints.generation.get();
        if self.sessions.checkpoints.prefetch.get() == Some((session.id, generation)) {
            return;
        }
        let Some(project_path) = self
            .workspace_path_for_session(session)
            .map(std::path::Path::to_path_buf)
        else {
            return;
        };
        let session_id = session.id;
        let retained_turn_counts = session
            .turns
            .iter()
            .map(|turn| turn.turn_count.saturating_sub(1))
            .collect::<Vec<_>>();
        self.sessions
            .checkpoints
            .prefetch
            .set(Some((session_id, generation)));
        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |this, cx| {
            let existing = cx
                .background_executor()
                .spawn(async move {
                    match workspace.request(michelle_client::WorkspaceOperation::SessionTurnRefs {
                        cwd: project_path,
                        session_id,
                    }) {
                        Ok(michelle_client::WorkspaceResult::TurnRefs { turn_counts }) => {
                            turn_counts.into_iter().collect::<HashSet<_>>()
                        }
                        Ok(_) | Err(_) => HashSet::new(),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.sessions.checkpoints.generation.get() != generation {
                    return;
                }
                let mut cache = this.sessions.checkpoints.refs.borrow_mut();
                for turn_count in retained_turn_counts {
                    cache.insert((session_id, turn_count), existing.contains(&turn_count));
                }
                drop(cache);
                cx.notify();
            });
        })
        .detach();
    }
}
