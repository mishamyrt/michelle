use crate::model::{ActivityKind, AgentSession, SessionStatus};
use crate::theme::Theme;
use gpui::Hsla;

pub fn status_color(theme: &Theme, status: SessionStatus) -> Hsla {
    match status {
        SessionStatus::Idle => theme.text_ghost,
        SessionStatus::Connecting | SessionStatus::Working => theme.text,
        SessionStatus::Background => theme.text_secondary,
        SessionStatus::Waiting => theme.warning,
        SessionStatus::Failed => theme.danger,
    }
}

pub fn activity_icon(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Reasoning => "sparkles",
        ActivityKind::Command => "terminal",
        ActivityKind::FileChange => "pencil",
        ActivityKind::FileRead => "doc",
        ActivityKind::FileSearch => "magnifyingglass",
        ActivityKind::FileList => "folder",
        ActivityKind::Search => "magnifyingglass",
        ActivityKind::Plan => "list.bullet",
        ActivityKind::Tool => "wrench.and.screwdriver",
    }
}

pub fn activity_noun(kind: ActivityKind) -> (String, String) {
    match kind {
        ActivityKind::Reasoning => (tr!("activity.thought"), tr!("activity.thoughts")),
        ActivityKind::Command => (tr!("activity.command"), tr!("activity.commands")),
        ActivityKind::FileChange => (tr!("activity.file_edit"), tr!("activity.file_edits")),
        ActivityKind::FileRead => (tr!("activity.file_read"), tr!("activity.file_reads")),
        ActivityKind::FileSearch => (
            tr!("activity.right_panel_ui.file_search"),
            tr!("activity.file_searches"),
        ),
        ActivityKind::FileList => (tr!("activity.file_list"), tr!("activity.file_lists")),
        ActivityKind::Search => (tr!("activity.search"), tr!("activity.searches")),
        ActivityKind::Plan => (tr!("activity.plan_step"), tr!("activity.plan_steps")),
        ActivityKind::Tool => (tr!("activity.tool_call"), tr!("activity.tool_calls")),
    }
}

pub fn localized_session_title(session: &AgentSession) -> String {
    let title = session.display_title();
    if title == AgentSession::DEFAULT_TITLE {
        tr!("session.new_task")
    } else {
        title.to_owned()
    }
}
