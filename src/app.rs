use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use crossbeam_channel::{Receiver, Sender, unbounded};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClipboardEntry, ClipboardItem, Context, Div,
    Entity, ExternalPaths, FocusHandle, Focusable, FontWeight, Hsla, IntoElement, KeyDownEvent,
    ListAlignment, ListOffset, ListState, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, ObjectFit, PathPromptOptions, Pixels, Render, ScrollHandle,
    SharedString, Stateful, StyleRefinement, TextRun, WeakEntity, Window, WindowBounds, canvas,
    div, ease_out_quint, fill, font, img, linear_color_stop, linear_gradient, list, point,
    prelude::*, pulsating_between, px, rgb,
};
use uuid::Uuid;

use crate::checkpoint;
use crate::composer_complete::{FileEntry, SlashCommand};
use crate::computer_use::{
    ComputerPermissions, ComputerTarget, ComputerUsePhase, ComputerUseState,
    PendingComputerApproval,
};
use crate::driver::{self, DriverHandle, DriverStartOptions, SessionOptions};
use crate::git_branch::BranchSnapshot;
use crate::input::{ComposerAttachmentPaste, ComposerEvent, ComposerInput, InputEvent, TextInput};
use crate::md;
use crate::model::{
    ActivityItem, ActivityKind, AgentSession, BackgroundWorkEvent, BackgroundWorkItem,
    BackgroundWorkKey, BackgroundWorkKind, BackgroundWorkStatus, Checkpoint, CheckpointStatus,
    ContextUsage, DriverEvent, FavoriteModel, Message, MessageAttachment, MessageRole,
    PendingPermission, Project, ProviderKind, ProviderModel, ProviderProbe, ProviderResumeCursor,
    ProviderSessionHistory, ProviderSessionSummary, QueuedMessage, ReasoningBlock, RuntimeMode,
    SessionStatus, SessionWorkspace, TranscriptBlock, TurnStatus, UserInputAnswer,
    UserInputQuestion, compact_path, unix_time, unix_time_millis,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::md::render::{
    Ctx as MarkdownCtx, MarkdownView, Metrics as MarkdownMetrics, Palette as MarkdownPalette,
    TranscriptSelection,
};
use crate::ui::menu::{
    ConfirmEntry, ContextMenuHandle, DismissMenu, MenuAlign, MenuItem, SelectNextEntry,
    SelectNextTab, SelectPreviousEntry, SelectPreviousTab, context_menu, dropdown_menu, popover,
};
use crate::ui::scrollbar::{self, ScrollbarState};
use crate::ui::tooltip::Tooltip;

use self::presentation::{
    ProjectNameSelector, activity_icon, activity_noun, localized_session_title, provider_color,
    provider_mark, status_color,
};
use crate::persistence::{
    ComposerDraftStore, ComposerDrafts, DEFAULT_RIGHT_PANEL_WIDTH, DEFAULT_SIDEBAR_WIDTH,
    PersistedState, PersistedWindowState, SidebarOrdering, StateStore,
};
use crate::query::{Query, QueryCache};
use crate::review_diff::{Snapshot as ReviewDiffSnapshot, Source as ReviewDiffSource};
use crate::terminal::TerminalView;
use crate::theme::{Theme, ThemeDefinition, ThemePreference, sp};
use crate::ui::text_field::TextField;
use crate::ui::{
    MenuChip, TOOLBAR_HEIGHT, contain_scroll, file_icon, icon, icon_button, motion, toggle_switch,
};
use crate::{
    CancelTaskSwitch, CancelTurn, CloseFind, CloseWindow, ConfirmTaskSwitch, CopySelection,
    FindNext, FindPrevious, FocusComposer, NavigateBack, NavigateForward, NewProject, NewSession,
    OpenFind, OpenFindReplace, OpenResumePicker, OpenSettings, ReplaceAllMatches, SaveFile,
    SelectFirstTask, SelectLastTask, SwitchTaskBackward, SwitchTaskForward, ToggleCommandPalette,
    ToggleFindCaseSensitive, ToggleFindRegex, ToggleFindWholeWord, ToggleFpsCounter,
    ToggleModelPicker, ToggleModelTraits, ToggleRightPanel, ToggleSidebar, ToggleUsagePanel,
};

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_CLEARANCE: f32 = 96.0;
#[cfg(not(target_os = "macos"))]
const TRAFFIC_LIGHT_CLEARANCE: f32 = 8.0;
const CONTENT_MAX_WIDTH: f32 = 720.0;
/// Menu-registry id of the composer's model picker, shared by its render site
/// and the primary-modifier `/` toggle action.
const SIDEBAR_MIN_WIDTH: f32 = 180.0;
const SIDEBAR_MAX_WIDTH: f32 = 420.0;
const RIGHT_PANEL_MIN_WIDTH: f32 = 280.0;
const RIGHT_PANEL_MAX_WIDTH: f32 = 1000.0;
const DEFAULT_FILE_TREE_WIDTH: f32 = 184.0;
const FILE_TREE_MIN_WIDTH: f32 = 140.0;
const FILE_TREE_MAX_WIDTH: f32 = 360.0;
const FILE_EDITOR_MIN_WIDTH: f32 = 140.0;
const FILE_EDITOR_INITIAL_WIDTH: f32 = 500.0;
const REVIEW_INITIAL_WIDTH: f32 = 820.0;
const MAIN_PANEL_MIN_WIDTH: f32 = 360.0;
const FOLLOWUP_TURN_TOP_GAP: f32 = 48.0;
const NAVIGATION_RAIL_WIDTH: f32 = 44.0;
const NAVIGATION_RAIL_LEFT: f32 = 16.0;
const NAVIGATION_RAIL_CONTENT_GAP: f32 = 16.0;
const NAVIGATION_RAIL_VIEWPORT_HEIGHT_RATIO: f32 = 0.80;
const NAVIGATION_RAIL_TICK_WIDTH: f32 = 32.0;
const NAVIGATION_RAIL_TICK_HEIGHT: f32 = 2.0;
const NAVIGATION_RAIL_TICK_GAP: f32 = 10.0;
const NAVIGATION_RAIL_INACTIVE_OPACITY: f32 = 0.45;
const NAVIGATION_RAIL_TURN_HEIGHT: f32 = NAVIGATION_RAIL_TICK_HEIGHT + NAVIGATION_RAIL_TICK_GAP;
const NAVIGATION_RAIL_FADE_HEIGHT: f32 = 20.0;
const NAVIGATION_RAIL_ANIMATION_DURATION: Duration = Duration::from_millis(300);
const ESCAPE_STOP_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(3);
/// Presentation pacing only. The app sleeps until a provider or background
/// result wakes it, then uses this cadence while streamed chunks remain.
/// Chunks queue for a full interval and fold into one drain → one notify →
/// one remeasure, so the per-commit parse/flatten/highlight work runs at ~8 Hz
/// regardless of the provider's chunk rate, and the veil dissolve spans the
/// gap so streamed text still reads as continuous.
const STREAM_FRAME_INTERVAL: Duration = Duration::from_millis(120);
/// How long a session may sit untouched before its provider process is released.
/// Codex and Pi stay resident between turns, so without this an afternoon of
/// abandoned tasks is an afternoon of idle agent processes.
const IDLE_SESSION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const IDLE_SESSION_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
const BACKGROUND_WORK_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const BACKGROUND_WORK_TICK_INTERVAL: Duration = Duration::from_secs(1);
const PLAN_USAGE_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(30);
const STREAM_SAVE_INTERVAL: Duration = Duration::from_secs(1);
/// Zed keeps status toasts on screen for ten seconds, pausing the countdown
/// while the pointer is over the toast so a long message remains readable.
const TASK_NOTIFICATION_TAG_PREFIX: &str = "michelle-task:";

pub(crate) fn task_notification_tag(session_id: Uuid) -> String {
    format!("{TASK_NOTIFICATION_TAG_PREFIX}{session_id}")
}

pub(crate) fn task_id_from_notification_tag(tag: &str) -> Option<Uuid> {
    tag.strip_prefix(TASK_NOTIFICATION_TAG_PREFIX)?.parse().ok()
}

/// Source bytes of parsed messages kept across session switches.
///
/// Measured at ~17x expansion into parsed structures, plus flattened text and
/// shaped runs on top, so this is bounded by source size rather than entry
/// count — one long message costs far more than several short ones. 512 KB
/// holds several sessions' transcripts for a few MB of structures.
const MAX_CACHED_MESSAGE_SOURCE_BYTES: usize = 512 * 1024;
/// Projects whose workspace lookups are remembered — branch, diff listing,
/// working tree. A window rarely has more than a handful open, and the diff
/// and tree caches are invalidated on every refresh, so they hold one entry in
/// practice. 8 is generous and caps the tree cache, the only large one, at a
/// few hundred KB.
const MAX_CACHED_WORKSPACES: usize = 8;
const STREAM_REMEASURE_TAIL_ROWS: usize = 3;
/// Top-level markdown blocks the live reasoning peek renders, counted from
/// the tail. The peek is a 400 px viewport pinned to the newest thought, so
/// this only bounds how far a mid-stream scrollback reaches — the full trace
/// renders once the turn settles. 48 blocks is far more than the viewport
/// shows and keeps a long think from costing O(document) per pulse tick.
const LIVE_REASONING_TAIL_BLOCKS: usize = 48;
/// Source bytes the live reasoning peek keeps parsed, counted from the tail.
/// Markdown cost is O(rendered source) per pulse tick regardless of block
/// shape — a wall-of-text think is one giant paragraph and a bulleted think
/// one giant list, so the block cap above bounds neither. Six KB is several
/// viewports of scrollback; the full trace renders once the turn settles.
const LIVE_REASONING_WINDOW_TARGET: usize = 6 * 1024;
/// Slide hysteresis: the window re-anchors (and the peek reparses from a
/// fresh view) only once the tail outgrows this. Fast reasoning can append
/// several KB per commit, so the gap to the target is deliberately wide —
/// a slide costs a full window rebuild, and sliding every commit would pay
/// it at commit rate.
const LIVE_REASONING_WINDOW_MAX: usize = 18 * 1024;

pub struct Michelle {
    session_ui: sessions::interactions::SessionUi,
    shell_ui: shell::ShellUi,
    shell_model: shell::model::ShellModel,
    notifications: notifications::model::NotificationsModel,
    notifications_ui: notifications::NotificationsUi,
    goal_model: goal_dialog::model::GoalModel,
    goal_ui: goal_dialog::GoalUi,
    image_model: image_preview::model::ImageModel,
    image_ui: image_preview::ImageUi,

    right_panel_model: right_panel::model::RightPanelModel,
    right_panel_ui: right_panel::RightPanelUi,
    transcript_model: transcript_view::model::TranscriptModel,
    transcript_ui: transcript_view::TranscriptUi,
    sidebar_model: sidebar::model::SidebarModel,
    sidebar_ui: sidebar::SidebarUi,
    composer_model: composer::model::ComposerModel,
    composer_ui: composer::ComposerUi,
    sessions: sessions::model::SessionModel,
    model_picker_ui: model_picker::ModelPickerUi,
    branch_picker_ui: branch_picker::BranchPickerUi,
    branches: branch_picker::model::BranchModel,
    project_picker_ui: project_picker::ProjectPickerUi,
    project_picker: project_picker::model::ProjectPickerModel,
    /// Owns the headless provider process for exactly as long as the desktop
    /// app entity. Debug builds can replace it independently after a rebuild;
    /// all live driver handles below are lightweight RPC proxies.
    daemon: michelle_client::DaemonSupervisor,
    settings: settings::model::SettingsModel,
    settings_ui: settings::SettingsUi,
    state: PersistedState,
    store: StateStore,
    command_palette: command_palette::CommandPaletteUi,
    task_switcher: task_switcher::TaskSwitcherUi,
    usage: usage_page::model::UsageModel,
    usage_ui: usage_page::UsageUi,
    /// Window-modal Git commit/push UI. Its repository snapshot is filled
    /// off-thread; frames only read this in-memory value.
    commit_dialog: Option<commit_dialog::CommitDialogState>,
    /// Commit-message generation and Git mutation outlive the modal that
    /// started them. Keeping the operation on the app also lets every
    /// Environment surface reflect and gate the same in-flight action.
    commit: commit_dialog::model::CommitModel,
    skills: skills_page::model::SkillsModel,
    skills_ui: skills_page::SkillsUi,
}

mod activity_diff;
mod autocomplete;
mod background_work;
mod branch_picker;
mod command_palette;
mod commit_dialog;
mod components;
mod composer;
use composer::model::{ComposerAttachment, ComposerSubmission};
mod file_search;
mod goal_dialog;
mod image_preview;
mod model_picker;
mod notifications;
#[cfg(test)]
use notifications::model::paused_toast_duration;
pub(crate) mod presentation;
mod project_picker;
mod render;
mod right_panel;
use right_panel::RightPanelSurface;
use right_panel::files::RightPanelFileEditor;
use right_panel::session_state::RightPanelSessionState;
mod runtime;
mod sessions;
use presentation::single_line_label;
use runtime::model::{
    DriverStartRequest, EventPumpSchedule, PreparedDriver, PreparedSubmission,
    RemoteTaskStateSnapshot, SessionRuntime, StreamDeltaKind, StreamPhase, session_is_reapable,
    signal_event_pump,
};
use sessions::activation::SessionNavigation;
use sessions::checkpoints::PendingCheckpointCapture;
mod settings;
use settings::SettingsPage;
mod shell;
mod sidebar;
mod skills_page;
mod startup;
use sessions::interactions::{
    ComputerUsePreview, EscapeStopConfirmation, EscapeStopPress, EscapeStopTarget, PendingUserInput,
};
use shell::*;
mod streaming;
mod task_switcher;
mod toolbar;
mod transcript_search;
mod transcript_view;
mod usage_meter;
mod usage_page;
use usage_page::UsageViewMode;
mod window_chrome;

pub use autocomplete::init as init_composer_autocomplete;
use background_work::{
    BackgroundWorkRegistry, work_kind_icon, work_status_color, work_status_label,
};
pub use command_palette::init as init_command_palette;
pub use commit_dialog::init as init_commit_dialog_keys;
use components::*;
pub use goal_dialog::init as init_goal_dialog_keys;
pub use image_preview::init as init_image_preview_keys;
use project_picker::ProjectPickerSite;
pub use settings::init as init_settings_keys;
use sidebar::SidebarGroup;
pub use sidebar::init as init_sidebar_keys;
pub use skills_page::init as init_skills_keys;
use streaming::*;
use transcript_view::ConversationNavigationRail;
use transcript_view::model::*;
use transcript_view::{MessageEdit, TranscriptAnchor, UserMessageScrollViewport};

#[cfg(test)]
mod tests;
