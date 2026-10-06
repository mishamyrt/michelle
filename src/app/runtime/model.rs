//! Desktop runtime state and in-flight submission bookkeeping.
use super::*;

pub(in crate::app) struct RuntimeModel {
    /// Clock for the idle-session sweep, so the check costs one comparison per
    /// frame instead of a scan.
    pub(in crate::app) last_idle_session_sweep: Instant,

    pub(in crate::app) event_wake_tx: smol::channel::Sender<()>,
    pub(in crate::app) task_state_sync_tx: Sender<Result<RemoteTaskStateSnapshot, String>>,
    pub(in crate::app) task_state_sync_events: Receiver<Result<RemoteTaskStateSnapshot, String>>,
    pub(in crate::app) runtimes: HashMap<Uuid, SessionRuntime>,
    pub(in crate::app) runtime_attach_pending: HashSet<Uuid>,
    pub(in crate::app) runtime_attach_misses: HashMap<Uuid, u8>,
    pub(in crate::app) background_work: HashMap<Uuid, BackgroundWorkRegistry>,
    pub(in crate::app) last_background_work_tick: Instant,
    pub(in crate::app) submission_preparations: HashSet<Uuid>,
    pub(in crate::app) response_fork_preparations: HashMap<Uuid, usize>,
    pub(in crate::app) pending_queue_drains: Vec<Uuid>,
    pub(in crate::app) stream_state_dirty: bool,
    pub(in crate::app) last_stream_save: Instant,
}

pub(in crate::app) struct SessionRuntime {
    pub(in crate::app) driver: DriverHandle,
    /// Invalidates stale ApplyOptions responses when settings change again or
    /// this runtime is replaced while the RPC is in flight.
    pub(in crate::app) options_generation: u64,
    pub(in crate::app) events: Receiver<DriverEvent>,
    pub(in crate::app) pending_events: VecDeque<DriverEvent>,
    /// Presentation metadata for steering messages awaiting the provider's
    /// accepted/rejected acknowledgement, in transport order.
    pub(in crate::app) pending_steers: VecDeque<ComposerSubmission>,
    pub(in crate::app) stream_phase: Option<StreamPhase>,
    /// The parked-turn notification has fired for the turn in flight, so a
    /// wake that parks again does not repeat it. Cleared when the turn ends.
    pub(in crate::app) park_announced: bool,
    pub(in crate::app) stream_remeasure_pending: bool,
    pub(in crate::app) pending_permission: Option<PendingPermission>,
    pub(in crate::app) pending_user_input: Option<PendingUserInput>,
    pub(in crate::app) pending_computer_approval: Option<PendingComputerApproval>,
    /// Back-to-front stack of window previews captured during the active turn.
    pub(in crate::app) computer_use_previews: Vec<ComputerUsePreview>,
    pub(in crate::app) computer_session_grants: HashSet<String>,
    pub(in crate::app) last_driver_error: Option<String>,
    /// When this session last sent or received anything, for idle reaping.
    pub(in crate::app) last_active_at: Instant,
    /// Background-process snapshots are provider IPC. Keep the polling clock
    /// on the runtime so switching tasks never creates duplicate probes.
    pub(in crate::app) last_background_refresh_at: Instant,
}

/// Sessions between accepting a submission and handing it to a provider.
///
/// Worktree creation and the pre-turn checkpoint both run off the UI thread,
/// but neither operation has a safe interrupt contract. Keeping this separate
/// from [`SessionStatus`] lets the composer distinguish that non-cancellable
/// preparation window from a connecting provider that can already be stopped.
pub(in crate::app) struct PreparedSubmission {
    pub(in crate::app) workspace: SessionWorkspace,
    pub(in crate::app) checkpoint_warning: Option<String>,
    /// `None` reuses an already-live runtime. `Some` contains the result of a
    /// provider process start performed on the background executor.
    pub(in crate::app) driver: Option<anyhow::Result<PreparedDriver>>,
}

/// Everything needed to start a provider process, captured while the session
/// is still on the UI thread. `cwd` is replaced with the materialized
/// worktree path by the background preparation task.
pub(in crate::app) struct DriverStartRequest {
    pub(in crate::app) session_id: Uuid,
    pub(in crate::app) provider: ProviderKind,
    pub(in crate::app) options: DriverStartOptions,
    pub(in crate::app) event_wake: smol::channel::Sender<()>,
    pub(in crate::app) daemon: michelle_client::DaemonSupervisor,
}

/// A provider process that has started off-thread but is not installed into
/// Michelle's runtime map yet. Its event receiver safely buffers early events.
pub(in crate::app) struct PreparedDriver {
    pub(in crate::app) handle: DriverHandle,
    pub(in crate::app) events: Receiver<DriverEvent>,
}

pub(in crate::app) struct RemoteTaskStateSnapshot {
    pub(in crate::app) projects: Vec<Project>,
    pub(in crate::app) sessions: Vec<AgentSession>,
}

pub(in crate::app) fn signal_event_pump(wake: &smol::channel::Sender<()>) {
    let _ = wake.try_send(());
}

/// A file dropped onto the composer, staged as a chip until the next
/// submission carries it as an `@` mention.
/// Whether an untouched session's provider process may be released.
///
/// A session mid-turn is not idle however long it has been quiet: a slow tool
/// call, or an approval waiting on the user, must not have its agent pulled out
/// from under it.
pub(in crate::app) fn session_is_reapable(
    session: Option<&AgentSession>,
    idle_for: Duration,
    has_live_background_work: bool,
) -> bool {
    !has_live_background_work
        && idle_for >= IDLE_SESSION_TIMEOUT
        && session.is_none_or(|session| {
            session.active_turn_id().is_none()
                && matches!(session.status, SessionStatus::Idle | SessionStatus::Failed)
        })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum StreamPhase {
    Text,
    Reasoning,
    Activity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum StreamDeltaKind {
    Text,
    Reasoning,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum EventPumpSchedule {
    Idle,
    StreamFrame,
    BackgroundOutput(Duration),
}
