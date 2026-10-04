use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Utc};
use gpui::{ClickEvent, FontFeatures, KeyBinding, KeyboardButton, actions};
use michelle_client::persistence::SidebarProjectGroup;

use super::*;

actions!(michelle_sidebar, [CancelSessionRename, FocusSidebar]);

const SESSION_RENAME_PARENT_CONTEXT: &str = "SessionRename";
const SESSION_RENAME_FIELD_CONTEXT: &str = "SessionRename > TextInput";

/// Sidebar shortcuts, including Escape overrides for navigation and renaming.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-shift-e", FocusSidebar, None),
        KeyBinding::new(
            "escape",
            FocusComposer,
            Some("SidebarNavigation && !TextInput"),
        ),
        KeyBinding::new(
            "escape",
            CancelSessionRename,
            Some(SESSION_RENAME_FIELD_CONTEXT),
        ),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SessionDateGroup {
    Today,
    Yesterday,
    ThisWeek,
    ThisMonth,
    ThisYear,
    More,
}

impl SessionDateGroup {
    const ALL: [Self; 6] = [
        Self::Today,
        Self::Yesterday,
        Self::ThisWeek,
        Self::ThisMonth,
        Self::ThisYear,
        Self::More,
    ];

    fn index(self) -> usize {
        match self {
            Self::Today => 0,
            Self::Yesterday => 1,
            Self::ThisWeek => 2,
            Self::ThisMonth => 3,
            Self::ThisYear => 4,
            Self::More => 5,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Today => tr!("sidebar.today"),
            Self::Yesterday => tr!("sidebar.yesterday"),
            Self::ThisWeek => tr!("sidebar.this_week"),
            Self::ThisMonth => tr!("sidebar.this_month"),
            Self::ThisYear => tr!("sidebar.this_year"),
            Self::More => tr!("sidebar.more"),
        }
    }
}

/// Stable identity for a collapsible sidebar section. Keeping both variants in
/// one set preserves each view's disclosure state when the user switches
/// between Project and Updated grouping.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SidebarGroup {
    Updated(SessionDateGroup),
    Project(Uuid),
    Projectless,
}

impl SidebarGroup {
    fn element_key(self) -> SharedString {
        match self {
            Self::Updated(group) => format!("updated-{}", group.index()).into(),
            Self::Project(project_id) => format!("project-{project_id}").into(),
            Self::Projectless => "projectless".into(),
        }
    }

    fn mix_fingerprint(self, fingerprint: u64) -> u64 {
        match self {
            Self::Updated(group) => mix(fingerprint, group.index() as u64 + 1),
            Self::Project(project_id) => mix_uuid(mix(fingerprint, 0x100), project_id),
            Self::Projectless => mix(fingerprint, 0x200),
        }
    }
}

fn sidebar_grouping_label(grouping: SidebarGrouping) -> String {
    match grouping {
        SidebarGrouping::Project => tr!("sidebar.grouping_project"),
        SidebarGrouping::Updated => tr!("sidebar.grouping_updated"),
    }
}

fn sidebar_ordering_label(ordering: SidebarOrdering) -> String {
    match ordering {
        SidebarOrdering::Newest => tr!("sidebar.ordering_newest"),
        SidebarOrdering::Oldest => tr!("sidebar.ordering_oldest"),
        SidebarOrdering::Manual => tr!("sidebar.ordering_manual"),
    }
}

fn session_date_group(timestamp: u64, today: NaiveDate) -> SessionDateGroup {
    let session_date = i64::try_from(timestamp)
        .ok()
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
        .unwrap_or(today);
    session_date_group_for_dates(session_date, today)
}

fn session_date_group_for_dates(session_date: NaiveDate, today: NaiveDate) -> SessionDateGroup {
    if session_date >= today {
        return SessionDateGroup::Today;
    }

    if today.pred_opt() == Some(session_date) {
        return SessionDateGroup::Yesterday;
    }

    let week_start = today
        .checked_sub_days(Days::new(today.weekday().num_days_from_monday().into()))
        .unwrap_or(today);
    if session_date >= week_start {
        return SessionDateGroup::ThisWeek;
    }

    if session_date.year() == today.year() && session_date.month() == today.month() {
        return SessionDateGroup::ThisMonth;
    }

    if session_date.year() == today.year() {
        return SessionDateGroup::ThisYear;
    }

    SessionDateGroup::More
}

// Paint the focus ring without changing the row's layout or measured height.
fn focus_ring(color: Hsla) -> gpui::BoxShadow {
    gpui::BoxShadow::new(px(0.0), px(0.0), color)
        .spread_radius(px(1.0))
        .inset()
}

fn session_group_header(theme: &Theme) -> Div {
    div()
        .h(px(SIDEBAR_GROUP_HEADER_HEIGHT))
        .px(px(8.0))
        .flex()
        .items_center()
        .text_size(sp(13.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_tertiary)
}

fn sidebar_collection_rename_target(id: Option<Uuid>, event: &ClickEvent) -> Option<Uuid> {
    match event {
        ClickEvent::Keyboard(event) if event.button == KeyboardButton::Enter => id,
        _ => None,
    }
}

fn append_sidebar_group_rows(
    rows: &mut Vec<SidebarRow>,
    group: SidebarGroup,
    sessions: &[Uuid],
    collapsed: bool,
    show_more: bool,
) {
    if sessions.is_empty() && !show_more {
        return;
    }

    rows.push(SidebarRow::Header(group));
    if !collapsed {
        rows.extend(sessions.iter().copied().map(SidebarRow::Session));
        if show_more {
            rows.push(SidebarRow::ShowMore(group));
        }
    }
    rows.push(SidebarRow::GroupSpacer);
}

/// Height of a session card plus the separation reserved beneath it in the
/// virtualized sidebar list. Keep the gap inside the list row so measured and
/// estimated heights stay identical for off-screen sessions.
const SIDEBAR_SESSION_CARD_HEIGHT: f32 = 51.0;
const SIDEBAR_SESSION_ROW_GAP: f32 = 1.0;
const SIDEBAR_SESSION_ROW_HEIGHT: f32 = SIDEBAR_SESSION_CARD_HEIGHT + SIDEBAR_SESSION_ROW_GAP;
const SIDEBAR_ACTION_ROW_HEIGHT: f32 = 32.0;
const SIDEBAR_SEARCH_BOTTOM_GAP: f32 = 10.0;
const SIDEBAR_GROUP_HEADER_HEIGHT: f32 = 32.0;
const SIDEBAR_COLLECTION_HEADER_HEIGHT: f32 = 32.0;
const SIDEBAR_GROUP_HEADER_BOTTOM_GAP: f32 = 1.0;
const SIDEBAR_SHOW_MORE_ROW_HEIGHT: f32 = 30.0;
const SIDEBAR_GROUP_SPACER_HEIGHT: f32 = 6.0;
const SIDEBAR_GROUP_CHILD_PADDING: f32 = 34.0;
const SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS: u64 = 3 * 24 * 60 * 60;
const SIDEBAR_PROJECT_REVEAL_BATCH: usize = 30;

static TABULAR_NUMBERS: LazyLock<FontFeatures> =
    LazyLock::new(|| FontFeatures(Arc::new(vec![("tnum".into(), 1)])));

/// The session row's trailing time: how long the live turn has been working,
/// or how long ago the agent last replied. A session that has never replied
/// shows nothing.
pub(super) fn session_time_label(session: &AgentSession, now: u64) -> Option<String> {
    if session.status == SessionStatus::Background {
        return Some(tr!("sidebar.status_background"));
    }
    if session.is_busy()
        && let Some(turn) = session
            .turns
            .last()
            .filter(|turn| turn.status == TurnStatus::Running)
    {
        return Some(tr!(
            "sidebar.working",
            elapsed = format_working_elapsed(now.saturating_sub(turn.started_at))
        ));
    }
    session
        .last_reply_at
        .map(|last_reply_at| format_time_ago(now.saturating_sub(last_reply_at)))
}

/// Recency for sidebar ordering and date groups. A submitted turn promotes the
/// task immediately, while metadata edits such as a rename do not; a task with
/// no turns stays anchored to when it was created.
fn sidebar_session_timestamp(session: &AgentSession) -> u64 {
    session.last_reply_at.unwrap_or(session.created_at)
}

fn sort_sidebar_sessions(
    sessions: &mut Vec<&AgentSession>,
    ordering: SidebarOrdering,
    manual_order: &[Uuid],
) {
    match ordering {
        SidebarOrdering::Newest => {
            sessions.sort_by_key(|session| std::cmp::Reverse(sidebar_session_timestamp(session)))
        }
        SidebarOrdering::Oldest => {
            sessions.sort_by_key(|session| sidebar_session_timestamp(session))
        }
        SidebarOrdering::Manual => {
            let ranks = manual_order
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i))
                .collect::<HashMap<_, _>>();
            sessions.sort_by_key(|session| {
                (
                    ranks.get(&session.id).copied(),
                    std::cmp::Reverse(session.created_at),
                    session.id,
                )
            });
        }
    }
}

fn move_sidebar_item(
    order: &mut Vec<Uuid>,
    source: Uuid,
    target: Uuid,
    after_target: bool,
) -> bool {
    if source == target {
        return false;
    }
    let Some(from) = order.iter().position(|id| *id == source) else {
        return false;
    };
    let Some(target_index) = order.iter().position(|id| *id == target) else {
        return false;
    };
    let to = target_index - usize::from(from < target_index) + usize::from(after_target);
    if from == to {
        return false;
    }
    order.remove(from);
    order.insert(to, source);
    true
}

fn project_sidebar_groups(
    sessions: &[&AgentSession],
    projectless_project_ids: &HashSet<Uuid>,
) -> Vec<(SidebarGroup, Vec<Uuid>)> {
    let mut groups: Vec<(SidebarGroup, Vec<Uuid>)> = Vec::new();
    let mut indexes = HashMap::new();
    let mut projectless_sessions = Vec::new();
    for session in sessions {
        if projectless_project_ids.contains(&session.project_id) {
            projectless_sessions.push(session.id);
            continue;
        }
        let index = *indexes.entry(session.project_id).or_insert_with(|| {
            let index = groups.len();
            groups.push((SidebarGroup::Project(session.project_id), Vec::new()));
            index
        });
        groups[index].1.push(session.id);
    }
    if !projectless_sessions.is_empty() {
        groups.push((SidebarGroup::Projectless, projectless_sessions));
    }
    groups
}

fn sidebar_project_timestamps(state: &PersistedState) -> HashMap<Uuid, u64> {
    let mut timestamps: HashMap<Uuid, u64> = HashMap::new();
    for session in state
        .sessions
        .iter()
        .filter(|session| session.has_started())
    {
        let timestamp = sidebar_session_timestamp(session);
        timestamps
            .entry(session.project_id)
            .and_modify(|current| {
                *current = if state.sidebar_ordering == SidebarOrdering::Oldest {
                    (*current).min(timestamp)
                } else {
                    (*current).max(timestamp)
                };
            })
            .or_insert(timestamp);
    }
    for project in &state.projects {
        timestamps.entry(project.id).or_insert(project.created_at);
    }
    timestamps
}

fn sidebar_project_sort_key(
    ordering: SidebarOrdering,
    timestamp: u64,
    project: Uuid,
) -> (u64, Uuid) {
    (
        if ordering == SidebarOrdering::Newest {
            u64::MAX - timestamp
        } else {
            timestamp
        },
        project,
    )
}

fn visible_project_sessions(
    sessions: &[Uuid],
    session_timestamps: &HashMap<Uuid, u64>,
    recent_cutoff: u64,
    revealed_older_sessions: usize,
) -> (Vec<Uuid>, bool) {
    let mut visible = Vec::with_capacity(sessions.len());
    let mut older_seen = 0usize;
    for session_id in sessions {
        let recent = session_timestamps
            .get(session_id)
            .is_some_and(|timestamp| *timestamp >= recent_cutoff);
        if recent || older_seen < revealed_older_sessions {
            visible.push(*session_id);
        }
        if !recent {
            older_seen = older_seen.saturating_add(1);
        }
    }
    (visible, older_seen > revealed_older_sessions)
}

fn sidebar_project_is_projectless(project: &Project, projectless_root: Option<&Path>) -> bool {
    projectless_root.is_some_and(|root| project.path.starts_with(root))
}

fn persisted_sidebar_branch_label(workspace: &SessionWorkspace) -> Option<&str> {
    match workspace {
        SessionWorkspace::Local => None,
        SessionWorkspace::NewWorktree { base_branch } => base_branch.as_deref(),
        SessionWorkspace::Worktree { branch, .. } => Some(branch.as_str()),
    }
    .filter(|branch| !branch.is_empty())
}

/// Compact "how long ago" for the sidebar: "just now", then one coarse unit —
/// "5m", "3h", "420d". Days are the largest unit so a glance still reads as a
/// count rather than a date.
pub(super) fn format_time_ago(seconds: u64) -> String {
    match seconds {
        0..=59 => tr!("sidebar.just_now"),
        60..=3_599 => tr!("sidebar.minutes_ago", count = seconds / 60),
        3_600..=86_399 => tr!("sidebar.hours_ago", count = seconds / 3_600),
        _ => tr!("sidebar.days_ago", count = seconds / 86_400),
    }
}

/// One row of the virtualized sidebar session history.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SidebarRow {
    /// Opens the window-wide command palette and scrolls with history.
    Search,
    /// User collection, or the built-in trailing Projects collection.
    Collection(Option<Uuid>),
    /// Group header; the first row also carries the sidebar actions.
    Header(SidebarGroup),
    /// A started session.
    Session(Uuid),
    /// Reveals the next batch of older sessions in a project section.
    ShowMore(SidebarGroup),
    /// Spacing between date groups.
    GroupSpacer,
}

fn sidebar_working_headers(
    state: &PersistedState,
    today: NaiveDate,
    projectless_root: Option<&Path>,
) -> HashSet<SidebarRow> {
    let projectless_projects = state
        .projects
        .iter()
        .filter(|project| sidebar_project_is_projectless(project, projectless_root))
        .map(|project| project.id)
        .collect::<HashSet<_>>();
    let mut collections = HashMap::new();
    for collection in &state.sidebar_project_groups {
        for project in &collection.projects {
            collections.entry(*project).or_insert(collection.id);
        }
    }
    let mut headers = HashSet::new();
    for session in state.sessions.iter().filter(|session| {
        session.has_started()
            && matches!(
                session.status,
                SessionStatus::Connecting | SessionStatus::Working
            )
    }) {
        let group = match state.sidebar_grouping {
            SidebarGrouping::Updated => SidebarGroup::Updated(session_date_group(
                sidebar_session_timestamp(session),
                today,
            )),
            SidebarGrouping::Project => {
                let projectless = projectless_projects.contains(&session.project_id);
                let collection = (!projectless)
                    .then(|| collections.get(&session.project_id).copied())
                    .flatten();
                headers.insert(SidebarRow::Collection(collection));
                if projectless {
                    SidebarGroup::Projectless
                } else {
                    SidebarGroup::Project(session.project_id)
                }
            }
        };
        headers.insert(SidebarRow::Header(group));
    }
    headers
}

fn append_sidebar_project_collections(
    rows: &mut Vec<SidebarRow>,
    sections: Vec<Vec<SidebarRow>>,
    groups: &[SidebarProjectGroup],
    projects_collapsed: bool,
) {
    let mut membership = HashMap::new();
    for (index, group) in groups.iter().enumerate() {
        for project in &group.projects {
            membership.entry(*project).or_insert(index);
        }
    }
    let mut buckets = vec![Vec::new(); groups.len() + 1];
    for section in sections {
        let index = match section.first() {
            Some(SidebarRow::Header(SidebarGroup::Project(id))) => {
                membership.get(id).copied().unwrap_or(groups.len())
            }
            _ => groups.len(),
        };
        buckets[index].extend(
            section
                .into_iter()
                .filter(|row| *row != SidebarRow::GroupSpacer),
        );
    }
    for (index, mut bucket) in buckets.into_iter().enumerate() {
        let group = groups.get(index);
        rows.push(SidebarRow::Collection(group.map(|group| group.id)));
        if !group.map_or(projects_collapsed, |group| group.collapsed) {
            rows.append(&mut bucket);
        }
        rows.push(SidebarRow::GroupSpacer);
    }
}

/// Only peers may be reordered: projects, or sessions within one section.
fn sidebar_reorder_siblings(rows: &[SidebarRow], row: SidebarRow) -> Vec<SidebarRow> {
    match row {
        SidebarRow::Collection(Some(_)) => rows
            .iter()
            .copied()
            .filter(|row| matches!(row, SidebarRow::Collection(Some(_))))
            .collect(),
        SidebarRow::Header(SidebarGroup::Project(_)) => rows
            .iter()
            .copied()
            .filter(|row| matches!(row, SidebarRow::Header(SidebarGroup::Project(_))))
            .collect(),
        SidebarRow::Session(_) => {
            let Some(index) = rows.iter().position(|candidate| *candidate == row) else {
                return Vec::new();
            };
            let start = rows[..index]
                .iter()
                .rposition(|row| matches!(row, SidebarRow::Header(_)))
                .map_or(0, |i| i + 1);
            rows[start..]
                .iter()
                .copied()
                .take_while(|row| !matches!(row, SidebarRow::Header(_) | SidebarRow::Collection(_)))
                .filter(|row| matches!(row, SidebarRow::Session(_)))
                .collect()
        }
        _ => Vec::new(),
    }
}

#[derive(Clone)]
struct SidebarDrag {
    row: SidebarRow,
    label: SharedString,
    michelle: WeakEntity<Michelle>,
    preview: Rc<RefCell<Option<SidebarDragPreview>>>,
}

/// A drag changes only this snapshot; the persisted order changes on drop.
pub(super) struct SidebarDragPreview {
    row: SidebarRow,
    siblings: Vec<SidebarRow>,
    rows: Rc<Vec<SidebarRow>>,
    offsets: Vec<Pixels>,
    ordering: SidebarOrdering,
    timestamps: HashMap<Uuid, u64>,
    destination: Option<(Option<Uuid>, Option<Uuid>)>,
    bounds: Bounds<Pixels>,
    cursor_offset: gpui::Point<Pixels>,
    collapsed: bool,
    working: bool,
}

fn sidebar_row_offsets(rows: &[SidebarRow]) -> Vec<Pixels> {
    let mut offsets = Vec::with_capacity(rows.len() + 1);
    offsets.push(Pixels::ZERO);
    for row in rows {
        offsets.push(*offsets.last().unwrap() + sidebar_row_height(*row));
    }
    offsets
}

const SIDEBAR_REORDER_DURATION: Duration = Duration::from_millis(160);

/// Keep layout at the new positions and ease visible rows from their old ones.
pub(super) struct SidebarReorderAnimation {
    offsets: HashMap<SidebarRow, Pixels>,
    dragged: Option<SidebarRow>,
    started: Instant,
    /// Sampled once per sidebar frame so a section's rows move together.
    remaining: f32,
}

impl SidebarReorderAnimation {
    fn remaining_at(&self, now: Instant) -> f32 {
        let progress = now.saturating_duration_since(self.started).as_secs_f32()
            / SIDEBAR_REORDER_DURATION.as_secs_f32();
        1.0 - gpui::ease_out_quint()(progress.clamp(0.0, 1.0))
    }

    fn between(
        old: &[SidebarRow],
        new: &[SidebarRow],
        previous: Option<&Self>,
        dragged: Option<SidebarRow>,
        now: Instant,
    ) -> Option<Self> {
        let dragged = dragged.or_else(|| previous.and_then(|animation| animation.dragged));
        let remaining = previous.map_or(0.0, |animation| animation.remaining_at(now));
        let old_positions = old
            .iter()
            .copied()
            .zip(sidebar_row_offsets(old))
            .filter(|(row, _)| *row != SidebarRow::GroupSpacer)
            .map(|(row, y)| {
                let offset = previous
                    .and_then(|animation| animation.offsets.get(&row))
                    .copied()
                    .unwrap_or(Pixels::ZERO);
                (row, y + offset * remaining)
            })
            .collect::<HashMap<_, _>>();
        let offsets = new
            .iter()
            .copied()
            .zip(sidebar_row_offsets(new))
            .filter_map(|(row, y)| {
                if Some(row) == dragged {
                    return None;
                }
                let offset = *old_positions.get(&row)? - y;
                (offset != Pixels::ZERO).then_some((row, offset))
            })
            .collect::<HashMap<_, _>>();
        (!offsets.is_empty()).then_some(Self {
            offsets,
            dragged,
            started: now,
            remaining: 1.0,
        })
    }
}

impl SidebarDragPreview {
    fn move_row(&mut self, y: Pixels) -> bool {
        if matches!(self.row, SidebarRow::Header(SidebarGroup::Project(_))) {
            return self.move_project(y);
        }
        let Some(start) = self.rows.iter().position(|row| *row == self.row) else {
            return false;
        };
        let collection = matches!(self.row, SidebarRow::Collection(Some(_)));
        if !collection
            && (self.ordering != SidebarOrdering::Manual
                || !matches!(self.row, SidebarRow::Session(_)))
        {
            return false;
        }
        let section_end = |rows: &[SidebarRow], start: usize| {
            if collection {
                (start + 1..rows.len())
                    .find(|index| matches!(rows[*index], SidebarRow::Collection(_)))
                    .unwrap_or(rows.len())
            } else {
                start + 1
            }
        };
        let end = section_end(&self.rows, start);
        let hovered = self
            .offsets
            .partition_point(|offset| *offset <= y)
            .saturating_sub(1);
        if hovered >= self.rows.len() || (start..end).contains(&hovered) {
            return false;
        }
        let target = if collection {
            let Some(target) = (0..=hovered)
                .rev()
                .find(|index| matches!(self.rows[*index], SidebarRow::Collection(_)))
            else {
                return false;
            };
            target
        } else {
            if !self.siblings.contains(&self.rows[hovered]) {
                return false;
            }
            hovered
        };
        let target_row = self.rows[target];
        let fixed_last = target_row == SidebarRow::Collection(None);
        let after = !fixed_last && y >= self.offsets[target] + sidebar_row_height(target_row) / 2.0;
        if !fixed_last && after != (start < target) {
            return false;
        }
        let mut rows = self.rows.as_ref().clone();
        let section = rows.drain(start..end).collect::<Vec<_>>();
        let target = rows.iter().position(|row| *row == target_row).unwrap();
        let insertion = if after {
            section_end(&rows, target)
        } else {
            target
        };
        rows.splice(insertion..insertion, section);
        if rows == *self.rows {
            return false;
        }
        self.offsets = sidebar_row_offsets(&rows);
        self.rows = Rc::new(rows);
        true
    }

    fn move_project(&mut self, y: Pixels) -> bool {
        let source = self.row;
        let SidebarRow::Header(SidebarGroup::Project(project)) = source else {
            return false;
        };
        let Some(start) = self.rows.iter().position(|row| *row == source) else {
            return false;
        };
        let boundary = |row: &SidebarRow| {
            matches!(
                row,
                SidebarRow::Header(_) | SidebarRow::Collection(_) | SidebarRow::GroupSpacer
            )
        };
        let end = (start + 1..self.rows.len())
            .find(|index| boundary(&self.rows[*index]))
            .unwrap_or(self.rows.len());
        let hovered = self
            .offsets
            .partition_point(|offset| *offset <= y)
            .saturating_sub(1);
        if hovered >= self.rows.len() || (start..end).contains(&hovered) {
            return false;
        }
        let Some(collection_index) = (0..=hovered)
            .rev()
            .find(|index| matches!(self.rows[*index], SidebarRow::Collection(_)))
        else {
            return false;
        };
        let SidebarRow::Collection(collection) = self.rows[collection_index] else {
            unreachable!();
        };
        let current_collection = self.rows[..start].iter().rev().find_map(|row| match row {
            SidebarRow::Collection(id) => Some(*id),
            _ => None,
        });
        if self.ordering != SidebarOrdering::Manual && current_collection == Some(collection) {
            return false;
        }
        let target = (collection_index + 1..=hovered)
            .rev()
            .find(|index| matches!(self.rows[*index], SidebarRow::Header(_)));
        let mut after = false;
        if self.ordering == SidebarOrdering::Manual
            && let Some(target) = target
        {
            if !matches!(
                self.rows[target],
                SidebarRow::Header(SidebarGroup::Project(_))
            ) {
                return false;
            }
            after = y >= self.offsets[target] + px(SIDEBAR_GROUP_HEADER_HEIGHT / 2.0);
            if current_collection == Some(collection) && after != (start < target) {
                return false;
            }
        }
        let target_row = target.map(|index| self.rows[index]);
        let mut rows = self.rows.as_ref().clone();
        let section = rows.drain(start..end).collect::<Vec<_>>();
        let collection_start = rows
            .iter()
            .position(|row| *row == SidebarRow::Collection(collection))
            .unwrap()
            + 1;
        let collection_end = (collection_start..rows.len())
            .find(|index| {
                matches!(
                    rows[*index],
                    SidebarRow::Collection(_)
                        | SidebarRow::GroupSpacer
                        | SidebarRow::Header(SidebarGroup::Projectless)
                )
            })
            .unwrap_or(rows.len());
        let insertion = if self.ordering != SidebarOrdering::Manual {
            let key = sidebar_project_sort_key(
                self.ordering,
                self.timestamps.get(&project).copied().unwrap_or_default(),
                project,
            );
            (collection_start..collection_end)
                .find(|index| match rows[*index] {
                    SidebarRow::Header(SidebarGroup::Project(id)) => {
                        key < sidebar_project_sort_key(
                            self.ordering,
                            self.timestamps.get(&id).copied().unwrap_or_default(),
                            id,
                        )
                    }
                    _ => false,
                })
                .unwrap_or(collection_end)
        } else if let Some(target) = target_row {
            let index = rows.iter().position(|row| *row == target).unwrap();
            if after {
                (index + 1..collection_end)
                    .find(|index| boundary(&rows[*index]))
                    .unwrap_or(collection_end)
            } else {
                index
            }
        } else {
            collection_start
        };
        let before = rows[insertion..collection_end]
            .iter()
            .find_map(|row| match row {
                SidebarRow::Header(SidebarGroup::Project(id)) => Some(*id),
                _ => None,
            });
        rows.splice(insertion..insertion, section);
        if rows == *self.rows {
            return false;
        }
        self.offsets = sidebar_row_offsets(&rows);
        self.rows = Rc::new(rows);
        self.destination = Some((collection, before));
        true
    }
}

impl Render for SidebarDrag {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let preview = self.preview.borrow();
        let Some(preview) = preview.as_ref() else {
            return div().into_any_element();
        };
        let row = match self.row {
            SidebarRow::Session(id) => self
                .michelle
                .update(cx, |this, cx| {
                    this.render_sidebar_session_item(id, false, cx)
                })
                .unwrap_or_else(|_| div().into_any_element()),
            SidebarRow::Collection(id) => self
                .michelle
                .update(cx, |this, cx| {
                    this.render_sidebar_collection(id, window, false, cx)
                })
                .unwrap_or_else(|_| div().into_any_element()),
            _ => session_group_header(&theme)
                .pl(px(8.0))
                .rounded(px(6.0))
                .font_weight(FontWeight::NORMAL)
                .gap(px(12.0))
                .child(icon(
                    if preview.collapsed {
                        "icons/folder.svg"
                    } else {
                        "icons/folder-open.svg"
                    },
                    14.0,
                    theme.text_secondary,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(self.label.clone()),
                )
                .when(preview.working, |header| {
                    header.child(motion::spin_slow(icon(
                        "icons/loader-circle.svg",
                        14.0,
                        theme.text,
                    )))
                })
                .into_any_element(),
        };
        div()
            .w(preview.bounds.size.width)
            .relative()
            .left(preview.bounds.left() - (window.mouse_position().x - preview.cursor_offset.x))
            .child(row)
            .into_any_element()
    }
}

fn sidebar_session_row_index(rows: &[SidebarRow], session_id: Uuid) -> Option<usize> {
    rows.iter()
        .position(|row| *row == SidebarRow::Session(session_id))
}

fn sidebar_navigation_target(rows: &[SidebarRow], current: SidebarRow, key: &str) -> Option<usize> {
    let index = rows.iter().position(|row| *row == current)?;
    let focusable = |index: &usize| rows[*index] != SidebarRow::GroupSpacer;
    match key {
        "up" => (0..index).rev().find(focusable),
        "down" => (index + 1..rows.len()).find(focusable),
        "home" => (0..rows.len()).find(focusable),
        "end" => (0..rows.len()).rev().find(focusable),
        _ => None,
    }
}

fn sidebar_row_height(row: SidebarRow) -> Pixels {
    px(match row {
        SidebarRow::Search => SIDEBAR_ACTION_ROW_HEIGHT + SIDEBAR_SEARCH_BOTTOM_GAP,
        SidebarRow::Header(_) => SIDEBAR_GROUP_HEADER_HEIGHT + SIDEBAR_GROUP_HEADER_BOTTOM_GAP,
        SidebarRow::Collection(_) => {
            SIDEBAR_COLLECTION_HEADER_HEIGHT + SIDEBAR_GROUP_HEADER_BOTTOM_GAP
        }
        SidebarRow::Session(_) => SIDEBAR_SESSION_ROW_HEIGHT,
        SidebarRow::ShowMore(_) => SIDEBAR_SHOW_MORE_ROW_HEIGHT,
        SidebarRow::GroupSpacer => SIDEBAR_GROUP_SPACER_HEIGHT,
    })
}

fn sidebar_bottom_aligned_offset(
    rows: &[SidebarRow],
    target: usize,
    viewport_height: Pixels,
) -> ListOffset {
    let mut item_ix = target;
    let mut height = sidebar_row_height(rows[target]);
    while item_ix > 0 && height < viewport_height {
        item_ix -= 1;
        height += sidebar_row_height(rows[item_ix]);
    }
    ListOffset {
        item_ix,
        offset_in_item: (height - viewport_height).max(Pixels::ZERO),
    }
}

fn reveal_sidebar_list_row(list: &ListState, rows: &[SidebarRow], index: usize) {
    let viewport = list.viewport_bounds();
    if viewport.size.height <= Pixels::ZERO {
        return;
    }
    if let Some(item) = list.bounds_for_item(index) {
        if item.top() >= viewport.top() && item.bottom() <= viewport.bottom() {
            return;
        }
        list.scroll_to_reveal_item(index);
    } else if index <= list.logical_scroll_top().item_ix {
        list.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: Pixels::ZERO,
        });
    } else {
        // Off-screen rows have not necessarily been measured yet. Their
        // sidebar heights are fixed, so align a lower target to the viewport
        // bottom just like scrollIntoView({ block: "nearest" }).
        list.scroll_to(sidebar_bottom_aligned_offset(
            rows,
            index,
            viewport.size.height,
        ));
    }
}

impl Michelle {
    pub(super) fn focus_sidebar_action(
        &mut self,
        _: &FocusSidebar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_page = None;
        self.commit_session_rename(cx);
        self.set_sidebar_visible(true, cx);
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        let index = self
            .pending_session_activation
            .map(|pending| pending.session_id)
            .or(self.state.selected_session)
            .and_then(|id| sidebar_session_row_index(&rows, id))
            .or_else(|| {
                rows.iter()
                    .position(|row| matches!(row, SidebarRow::Header(_)))
            })
            .unwrap_or(0);
        if let Some(row) = rows.get(index) {
            self.focus_sidebar_row(*row, window, cx);
        }
    }

    fn focus_sidebar_row(&self, row: SidebarRow, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        let Some(index) = rows.iter().position(|candidate| *candidate == row) else {
            return;
        };
        let focus = match row {
            SidebarRow::Search => self.sidebar_search_focus.clone(),
            SidebarRow::Collection(id) => self
                .menu_handle(format!("project-collection-{id:?}"), cx)
                .trigger_focus_handle()
                .clone(),
            SidebarRow::Header(group) => self
                .sidebar_group_header_focuses
                .borrow_mut()
                .entry(group)
                .or_insert_with(|| cx.focus_handle())
                .clone(),
            SidebarRow::ShowMore(group) => self
                .sidebar_show_more_focuses
                .borrow_mut()
                .entry(group)
                .or_insert_with(|| cx.focus_handle())
                .clone(),
            SidebarRow::Session(id) => self
                .menu_handle(format!("session-{id}"), cx)
                .trigger_focus_handle()
                .clone(),
            SidebarRow::GroupSpacer => return,
        };
        self.sync_sidebar_rows(&rows);
        if self.sidebar_list_state.viewport_bounds().size.height <= Pixels::ZERO {
            self.sidebar_list_state.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: Pixels::ZERO,
            });
        } else {
            reveal_sidebar_list_row(&self.sidebar_list_state, &rows, index);
        }
        // Virtualized rows must be painted before their focus joins the dispatch tree.
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    fn sidebar_navigation_key_down(
        &mut self,
        row: SidebarRow,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        if self.session_rename.is_none()
            && self.sidebar_project_group_rename.is_none()
            && (self.state.sidebar_ordering == SidebarOrdering::Manual
                || matches!(row, SidebarRow::Collection(Some(_))))
            && event.keystroke.modifiers.alt
            && event.keystroke.modifiers.shift
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.platform
            && matches!(key, "up" | "down")
        {
            let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
            let siblings = sidebar_reorder_siblings(&rows, row);
            let target = siblings
                .iter()
                .position(|candidate| *candidate == row)
                .and_then(|index| {
                    if key == "up" {
                        index.checked_sub(1)
                    } else {
                        index.checked_add(1)
                    }
                })
                .and_then(|index| siblings.get(index))
                .copied();
            if let Some(target) = target {
                self.reorder_sidebar_row(row, target, cx);
                self.focus_sidebar_row(row, window, cx);
            }
            cx.stop_propagation();
            return true;
        }
        if self.session_rename.is_some()
            || self.sidebar_project_group_rename.is_some()
            || event.keystroke.modifiers.modified()
            || !matches!(key, "up" | "down" | "home" | "end")
        {
            return false;
        }
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        if let Some(index) = sidebar_navigation_target(&rows, row, key) {
            self.focus_sidebar_row(rows[index], window, cx);
        }
        cx.stop_propagation();
        true
    }

    pub(super) fn window_drag_region(
        &self,
        region: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        // Windows drags from the hit test, not from a mouse-move handler:
        // `DefWindowProc` moves the window once the region reports itself as
        // caption, and performs the user's configured double-click action.
        #[cfg(target_os = "windows")]
        let region = region.window_control_area(gpui::WindowControlArea::Drag);

        region
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    crate::platform::titlebar_double_click(window);
                }
            })
            .on_mouse_down_out(cx.listener(|this, _, _, _| {
                this.header_drag_armed = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.header_drag_armed = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.header_drag_armed = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.header_drag_armed {
                    this.header_drag_armed = false;
                    crate::platform::start_window_move(window);
                }
            }))
    }
    // ── Sidebar ────────────────────────────────────────────────────────────

    fn render_fps_counter(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let fps = self.fps_value;
        let dot = if fps == 0 {
            theme.text_ghost
        } else if fps >= 55 {
            theme.success
        } else if fps >= 30 {
            theme.warning
        } else {
            theme.danger
        };
        div()
            .flex_none()
            .h(px(26.0))
            .px(px(6.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .text_size(sp(12.5))
            .line_height(sp(0.0))
            .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(dot))
            .child(
                div()
                    .text_color(theme.text_tertiary)
                    .font_family(crate::md::render::MONO_FAMILY)
                    .child(SharedString::from(format!("{fps} FPS"))),
            )
    }

    fn render_sidebar_toggle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = Theme::current(cx);
        div()
            .id("toggle-sidebar")
            .w(px(26.0))
            .h(px(26.0))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .child(icon("icons/panel-left.svg", 14.0, theme.text_tertiary))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.set_sidebar_visible(!this.sidebar_visible, cx);
            }))
    }

    pub(super) fn render_history_button(
        &self,
        id: &'static str,
        icon_path: &'static str,
        enabled: bool,
        navigate_back: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        div()
            .id(id)
            .w(px(26.0))
            .h(px(26.0))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .when(!enabled, |element| element.opacity(0.35))
            .when(enabled, |element| {
                element
                    .hover(|element| element.bg(theme.overlay))
                    .active(|element| element.bg(theme.overlay_strong))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if navigate_back {
                            this.navigate_back_action(&NavigateBack, window, cx);
                        } else {
                            this.navigate_forward_action(&NavigateForward, window, cx);
                        }
                    }))
            })
            .child(icon(icon_path, 14.0, theme.text_tertiary))
    }

    fn render_sidebar_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("sidebar-titlebar")
            .h(px(48.0))
            .flex_none()
            .flex()
            .items_center()
            .children(self.render_client_window_controls(
                super::window_chrome::WindowControlSide::Left,
                window,
                cx,
            ))
            .child(
                self.window_drag_region(
                    div()
                        .id("sidebar-traffic-light-drag-region")
                        .w(px(TRAFFIC_LIGHT_CLEARANCE))
                        .h_full()
                        .flex_none(),
                    cx,
                ),
            )
            .child(self.render_sidebar_toggle(cx))
            .child(
                div()
                    .ml(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(self.render_history_button(
                        "navigate-back",
                        "icons/arrow-left.svg",
                        !self.session_navigation.back.is_empty(),
                        true,
                        cx,
                    ))
                    .child(self.render_history_button(
                        "navigate-forward",
                        "icons/arrow-right.svg",
                        !self.session_navigation.forward.is_empty(),
                        false,
                        cx,
                    )),
            )
            .child(self.window_drag_region(
                div().id("sidebar-titlebar-drag-region").h_full().flex_1(),
                cx,
            ))
    }

    fn sidebar_menu_items(&self, weak: WeakEntity<Self>) -> Vec<MenuItem> {
        let grouping = self.state.sidebar_grouping;
        let ordering = self.state.sidebar_ordering;
        let grouping_weak = weak.clone();
        let ordering_weak = weak.clone();
        let project_weak = weak.clone();
        let mut items = vec![
            MenuItem::submenu_with_value(
                tr!("sidebar.grouping"),
                sidebar_grouping_label(grouping),
                move |_| {
                    let project_weak = grouping_weak.clone();
                    let updated_weak = grouping_weak.clone();
                    vec![
                        MenuItem::new(tr!("sidebar.grouping_project"), move |_, cx| {
                            let _ = project_weak.update(cx, |this, cx| {
                                this.set_sidebar_grouping(SidebarGrouping::Project, cx);
                            });
                        })
                        .selected(grouping == SidebarGrouping::Project),
                        MenuItem::new(tr!("sidebar.grouping_updated"), move |_, cx| {
                            let _ = updated_weak.update(cx, |this, cx| {
                                this.set_sidebar_grouping(SidebarGrouping::Updated, cx);
                            });
                        })
                        .selected(grouping == SidebarGrouping::Updated),
                    ]
                },
            ),
            MenuItem::submenu_with_value(
                tr!("sidebar.ordering"),
                sidebar_ordering_label(ordering),
                move |_| {
                    let newest_weak = ordering_weak.clone();
                    let oldest_weak = ordering_weak.clone();
                    let manual_weak = ordering_weak.clone();
                    vec![
                        MenuItem::new(tr!("sidebar.ordering_newest"), move |_, cx| {
                            let _ = newest_weak.update(cx, |this, cx| {
                                this.set_sidebar_ordering(SidebarOrdering::Newest, cx);
                            });
                        })
                        .selected(ordering == SidebarOrdering::Newest),
                        MenuItem::new(tr!("sidebar.ordering_oldest"), move |_, cx| {
                            let _ = oldest_weak.update(cx, |this, cx| {
                                this.set_sidebar_ordering(SidebarOrdering::Oldest, cx);
                            });
                        })
                        .selected(ordering == SidebarOrdering::Oldest),
                        MenuItem::new(tr!("sidebar.ordering_manual"), move |_, cx| {
                            let _ = manual_weak.update(cx, |this, cx| {
                                this.set_sidebar_ordering(SidebarOrdering::Manual, cx);
                            });
                        })
                        .selected(ordering == SidebarOrdering::Manual),
                    ]
                },
            ),
            MenuItem::Separator,
            MenuItem::new(tr!("project.new_project"), move |_, cx| {
                let _ = project_weak.update(cx, |this, cx| this.add_project(cx));
            })
            .icon("icons/folder-new.svg"),
        ];
        if grouping == SidebarGrouping::Project {
            items.push(MenuItem::new(
                tr!("sidebar.new_group"),
                move |window, cx| {
                    let _ = weak.update(cx, |this, cx| this.create_sidebar_collection(window, cx));
                },
            ));
        }
        items
    }

    fn render_sidebar_action_row(
        &self,
        id: &'static str,
        icon_path: &'static str,
        label: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        div()
            .id(id)
            .tab_index(0)
            .w_full()
            .h(px(SIDEBAR_ACTION_ROW_HEIGHT))
            .flex_none()
            .px(px(4.0))
            .rounded(px(7.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .cursor_default()
            .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
            .hover(|element| element.bg(theme.sidebar_item_background))
            .active(|element| element.bg(theme.overlay_strong))
            .child(
                div()
                    .size(px(20.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(icon_path, 14.0, theme.text_secondary)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(sp(13.0))
                    .text_color(theme.text)
                    .child(label),
            )
    }

    fn render_sidebar_new_session(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.render_sidebar_action_row(
            "sidebar-new-session",
            "icons/compose.svg",
            tr!("menu.new_task"),
            cx,
        )
        .on_click(cx.listener(|this, _, window, cx| {
            this.new_session_action(&NewSession, window, cx);
        }))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                this.new_session_action(&NewSession, window, cx);
                cx.stop_propagation();
            }
        }))
    }

    fn render_sidebar_search(&self, cx: &mut Context<Self>) -> Div {
        let search = self
            .render_sidebar_action_row(
                "sidebar-search",
                "icons/search.svg",
                tr!("sidebar.search"),
                cx,
            )
            .track_focus(&self.sidebar_search_focus)
            .tab_stop(true)
            .on_click(cx.listener(|this, _, window, cx| {
                this.toggle_command_palette_action(&ToggleCommandPalette, window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.sidebar_navigation_key_down(SidebarRow::Search, event, window, cx) {
                    return;
                }
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.toggle_command_palette_action(&ToggleCommandPalette, window, cx);
                    cx.stop_propagation();
                }
            }));
        div()
            .w_full()
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .h(px(SIDEBAR_ACTION_ROW_HEIGHT + SIDEBAR_SEARCH_BOTTOM_GAP))
            .flex_none()
            .child(search)
    }

    fn render_sidebar_footer(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        div()
            .flex_none()
            .h(px(40.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .child(
                div()
                    .id("open-settings")
                    .tab_index(0)
                    .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
                    .w(px(26.0))
                    .h(px(26.0))
                    .flex_none()
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_default()
                    .hover(|element| element.bg(theme.overlay))
                    .active(|element| element.bg(theme.overlay_strong))
                    .tooltip(Tooltip::text(tr_cow!("common.settings")))
                    .child(icon("icons/settings.svg", 14.0, theme.text_tertiary))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings_action(&OpenSettings, window, cx);
                    })),
            )
    }

    /// Resolve every ordinary local project's branch in one background pass.
    /// The render path only computes an allocation-free source fingerprint;
    /// collection building and daemon requests happen once when that moves.
    fn ensure_sidebar_branch_labels(&self, cx: &mut Context<Self>) {
        if self.state.sidebar_grouping != SidebarGrouping::Project {
            return;
        }

        let mut fingerprint = 0xb4a7_c4e5_51de_ba11;
        for session in &self.state.sessions {
            if session.has_started() && matches!(&session.workspace, SessionWorkspace::Local) {
                fingerprint = mix_uuid(fingerprint, session.id);
                fingerprint = mix_uuid(fingerprint, session.project_id);
            }
        }
        for project in &self.state.projects {
            fingerprint = mix_uuid(fingerprint, project.id);
        }
        if self.sidebar_branch_scan_fingerprint.get() == Some(fingerprint) {
            return;
        }
        self.sidebar_branch_scan_fingerprint.set(Some(fingerprint));
        let generation = self.sidebar_branch_scan_generation.get().wrapping_add(1);
        self.sidebar_branch_scan_generation.set(generation);

        let local_project_ids = self
            .state
            .sessions
            .iter()
            .filter(|session| {
                session.has_started() && matches!(&session.workspace, SessionWorkspace::Local)
            })
            .map(|session| session.project_id)
            .collect::<HashSet<_>>();
        let projectless_root = crate::projectless::workspace_root();
        let paths = self
            .state
            .projects
            .iter()
            .filter(|project| local_project_ids.contains(&project.id))
            .filter(|project| !sidebar_project_is_projectless(project, projectless_root.as_deref()))
            .map(|project| project.path.clone())
            .collect::<HashSet<_>>();
        if paths.is_empty() {
            self.sidebar_branch_labels.borrow_mut().clear();
            return;
        }

        let workspace = michelle_client::WorkspaceClient::new(self.daemon.client());
        cx.spawn(async move |michelle, cx| {
            let labels = cx
                .background_executor()
                .spawn(async move {
                    let mut labels = HashMap::new();
                    for path in paths {
                        let branch = match workspace.request(
                            michelle_client::WorkspaceOperation::InspectBranches {
                                cwd: path.clone(),
                            },
                        ) {
                            Ok(michelle_client::WorkspaceResult::Branches {
                                snapshot: Some(snapshot),
                            }) => snapshot.display_branch().map(str::to_owned),
                            _ => None,
                        };
                        if let Some(branch) = branch {
                            labels.insert(path, branch);
                        }
                    }
                    labels
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                if michelle.sidebar_branch_scan_generation.get() != generation {
                    return;
                }
                *michelle.sidebar_branch_labels.borrow_mut() = labels
                    .into_iter()
                    .map(|(path, branch)| (path, SharedString::from(branch)))
                    .collect();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn cache_sidebar_branch_label(&self, path: &Path, branch: Option<&str>) {
        let mut labels = self.sidebar_branch_labels.borrow_mut();
        if let Some(branch) = branch.filter(|branch| !branch.is_empty()) {
            labels.insert(path.to_path_buf(), SharedString::from(branch.to_owned()));
        } else {
            labels.remove(path);
        }
    }

    pub(super) fn render_sidebar(
        &self,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        self.ensure_sidebar_branch_labels(cx);
        let is_resizing = self
            .panel_resize_drag
            .is_some_and(|drag| drag.target == PanelResizeTarget::Sidebar);

        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        let rows = if cx.has_active_drag() {
            self.sidebar_drag_preview
                .borrow()
                .as_ref()
                .map(|preview| preview.rows.clone())
                .unwrap_or(rows)
        } else {
            rows
        };
        let now = Instant::now();
        if cx.reduce_motion() {
            self.sidebar_reorder_animation.borrow_mut().take();
        } else if self.sidebar_reorder_animation.borrow().is_some()
            && self.sidebar_row_cache.borrow().as_slice() != rows.as_slice()
        {
            // A cancelled drag returns to the persisted order from the current
            // visible positions, including an unfinished slide.
            self.animate_sidebar_reorder(&rows, None, now);
        }
        {
            let mut animation = self.sidebar_reorder_animation.borrow_mut();
            if let Some(slide) = animation.as_mut() {
                slide.remaining = slide.remaining_at(now);
                if slide.remaining > 0.0 {
                    motion::pulse_lease(window.current_view(), cx);
                } else {
                    animation.take();
                }
            }
        }
        self.sync_sidebar_rows(&rows);
        // Restored selection exists before ListState knows the viewport size.
        // Retry after the first layout so nearest-edge alignment has a height.
        if self.sidebar_list_state.viewport_bounds().size.height <= Pixels::ZERO
            && let Some(session_id) = self
                .pending_session_activation
                .map(|pending| pending.session_id)
                .or(self.state.selected_session)
        {
            let entity = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                let _ = entity.update(cx, |this, cx| {
                    let selected_session = this
                        .pending_session_activation
                        .map(|pending| pending.session_id)
                        .or(this.state.selected_session);
                    if selected_session == Some(session_id) {
                        this.reveal_sidebar_session(session_id);
                        cx.notify();
                    }
                });
            });
        }
        let history_scrolled =
            self.sidebar_list_state.scroll_px_offset_for_scrollbar().y < px(-0.5);
        let entity = cx.entity().downgrade();

        let history = div()
            .id("sidebar-scroll")
            .flex_1()
            .min_h_0()
            .relative()
            .on_drag_move::<SidebarDrag>(cx.listener(
                |this, event: &gpui::DragMoveEvent<SidebarDrag>, _, cx| {
                    let drag = event.drag(cx);
                    let mut preview = drag.preview.borrow_mut();
                    let Some(preview) = preview.as_mut() else {
                        return;
                    };
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    let top = this.sidebar_list_state.logical_scroll_top();
                    let scroll_y =
                        preview.offsets[top.item_ix.min(preview.rows.len())] + top.offset_in_item;
                    let y = event.event.position.y - event.bounds.top() + scroll_y
                        - preview.cursor_offset.y
                        + preview.bounds.size.height / 2.0;
                    if preview.move_row(y) {
                        if !cx.reduce_motion() {
                            this.animate_sidebar_reorder(
                                &preview.rows,
                                Some(drag.row),
                                Instant::now(),
                            );
                        }
                        this.sync_sidebar_rows(&preview.rows);
                        let item_ix = preview
                            .offsets
                            .partition_point(|offset| *offset <= scroll_y)
                            .saturating_sub(1)
                            .min(preview.rows.len());
                        this.sidebar_list_state.scroll_to(ListOffset {
                            item_ix,
                            offset_in_item: scroll_y - preview.offsets[item_ix],
                        });
                    }
                    // GPUI refreshes the window for pointer movement; no stream/pulse work here.
                },
            ))
            .can_drop(|value, _, _| {
                value
                    .downcast_ref::<SidebarDrag>()
                    .is_some_and(|drag| drag.preview.borrow().is_some())
            })
            .on_drop(cx.listener(|this, drag: &SidebarDrag, _, cx| {
                this.commit_sidebar_drag(drag, cx);
            }))
            .child(
                div().px(px(10.0)).size_full().child(
                    list(self.sidebar_list_state.clone(), move |index, window, cx| {
                        entity
                            .upgrade()
                            .map(|entity| {
                                entity.update(cx, |this, cx| {
                                    this.sidebar_row(index, &rows, window, cx)
                                })
                            })
                            .unwrap_or_else(|| div().into_any_element())
                    })
                    .size_full(),
                ),
            )
            .child(scrollbar::vertical(
                &self.sidebar_list_state,
                &self.sidebar_scrollbar,
            ))
            .when(history_scrolled, |scroll| {
                scroll.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(px(1.0))
                        .bg(theme.border),
                )
            });
        let menu = self.menu_handle("sidebar-background", cx);
        let keyboard_menu = menu.clone();
        let weak = cx.entity().downgrade();
        let sidebar = div()
            .id("sidebar")
            .track_focus(menu.trigger_focus_handle())
            .tab_index(0)
            .tab_group()
            .tab_stop(true)
            .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
            .key_context("SidebarNavigation")
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if keyboard_menu.trigger_focus_handle().is_focused(window)
                    && ((event.keystroke.key == "f10" && event.keystroke.modifiers.shift)
                        || (event.keystroke.key == "enter" && event.keystroke.modifiers.control))
                {
                    keyboard_menu.open_context_menu(window, cx);
                    cx.stop_propagation();
                }
            })
            .on_action(cx.listener(|this, action: &FocusComposer, window, cx| {
                if !cx.stop_active_drag(window) {
                    this.focus_composer_action(action, window, cx);
                }
            }))
            .w(px(width))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(if is_resizing {
                theme.sidebar_drag_background
            } else {
                theme.sidebar_surface()
            })
            .child(self.render_sidebar_titlebar(window, cx))
            .child(
                div()
                    .flex_none()
                    .px(px(10.0))
                    .child(self.render_sidebar_new_session(cx)),
            )
            .child(history)
            .child(self.render_sidebar_footer(cx));
        context_menu(sidebar, "sidebar-background-menu", &menu, move |cx| {
            weak.upgrade()
                .map(|entity| entity.read(cx).sidebar_menu_items(weak.clone()))
                .unwrap_or_default()
        })
    }

    /// Keep a newly selected task visible without disturbing the sidebar when
    /// its row is already fully inside the viewport.
    pub(super) fn reveal_sidebar_session(&self, session_id: Uuid) {
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        self.sync_sidebar_rows(&rows);
        if let Some(index) = sidebar_session_row_index(&rows, session_id) {
            reveal_sidebar_list_row(&self.sidebar_list_state, &rows, index);
        }
    }

    /// The sidebar row snapshot, rebuilt only when its inputs move.
    ///
    /// The sidebar re-renders at pulse cadence whenever one of its session
    /// rows shows a working spinner, and rebuilding the snapshot sorts every
    /// started session and runs calendar math per session — far too much per
    /// tick for values that move at most once per stream commit. The
    /// fingerprint is an allocation-free scan of exactly what
    /// [`Self::sidebar_rows`] reads: started sessions with their project and
    /// recency and activity, the presentation preferences, the collapsed-group set, and
    /// today's date and the moving project-recency boundary.
    fn sidebar_rows_cached(&self, today: NaiveDate, now: u64) -> Rc<Vec<SidebarRow>> {
        let mut fingerprint = mix(0x51de_ba5e_5eed_c0de, today.num_days_from_ce() as u64);
        fingerprint = mix(
            fingerprint,
            match self.state.sidebar_grouping {
                SidebarGrouping::Project => 1,
                SidebarGrouping::Updated => 2,
            },
        );
        fingerprint = mix(
            fingerprint,
            match self.state.sidebar_ordering {
                SidebarOrdering::Newest => 1,
                SidebarOrdering::Oldest => 2,
                SidebarOrdering::Manual => 3,
            },
        );
        for session in &self.state.sessions {
            if !session.has_started() {
                continue;
            }
            fingerprint = mix_uuid(fingerprint, session.id);
            fingerprint = mix_uuid(fingerprint, session.project_id);
            fingerprint = mix(
                fingerprint,
                u64::from(matches!(
                    session.status,
                    SessionStatus::Connecting | SessionStatus::Working
                )),
            );
            if self.state.sidebar_ordering == SidebarOrdering::Manual {
                fingerprint = mix(fingerprint, session.created_at);
            }
            if self.state.sidebar_ordering != SidebarOrdering::Manual
                || self.state.sidebar_grouping == SidebarGrouping::Updated
            {
                fingerprint = mix(fingerprint, sidebar_session_timestamp(session));
            }
            if self.state.sidebar_grouping == SidebarGrouping::Project
                && self.state.sidebar_ordering != SidebarOrdering::Manual
            {
                fingerprint = mix(
                    fingerprint,
                    u64::from(
                        sidebar_session_timestamp(session)
                            >= now.saturating_sub(SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS),
                    ),
                );
            }
        }
        if self.state.sidebar_grouping == SidebarGrouping::Project {
            for project in &self.state.projects {
                fingerprint = mix_uuid(fingerprint, project.id);
                fingerprint = mix(fingerprint, project.created_at);
            }
            fingerprint = mix(
                fingerprint,
                u64::from(self.state.sidebar_projects_collapsed),
            );
            for group in &self.state.sidebar_project_groups {
                fingerprint = mix_uuid(fingerprint, group.id);
                fingerprint = mix(fingerprint, u64::from(group.collapsed));
                for project in &group.projects {
                    fingerprint = mix_uuid(fingerprint, *project);
                }
            }
            // A map has no stable iteration order; combine order-independently.
            let revealed =
                self.sidebar_project_reveal_counts
                    .iter()
                    .fold(0u64, |combined, (group, count)| {
                        combined.wrapping_add(group.mix_fingerprint(*count as u64))
                    });
            fingerprint = mix(
                mix(fingerprint, self.sidebar_project_reveal_counts.len() as u64),
                revealed,
            );
        }
        // A set has no stable iteration order; combine order-independently.
        let collapsed = self
            .sidebar_collapsed_groups
            .iter()
            .fold(0u64, |combined, group| {
                combined.wrapping_add(group.mix_fingerprint(0))
            });
        fingerprint = mix(
            mix(fingerprint, self.sidebar_collapsed_groups.len() as u64),
            collapsed,
        );
        if self.sidebar_rows_fingerprint.get() != Some(fingerprint) {
            *self.sidebar_rows_snapshot.borrow_mut() = Rc::new(self.sidebar_rows(today, now));
            *self.sidebar_working_headers.borrow_mut() = sidebar_working_headers(
                &self.state,
                today,
                crate::projectless::workspace_root().as_deref(),
            );
            self.sidebar_rows_fingerprint.set(Some(fingerprint));
        }
        self.sidebar_rows_snapshot.borrow().clone()
    }

    /// Snapshot the session history as a flat list of lightweight rows under
    /// the current grouping and ordering preferences.
    fn sidebar_rows(&self, today: NaiveDate, now: u64) -> Vec<SidebarRow> {
        let mut sorted_sessions = self
            .state
            .sessions
            .iter()
            .filter(|session| session.has_started())
            .collect::<Vec<_>>();
        sort_sidebar_sessions(
            &mut sorted_sessions,
            self.state.sidebar_ordering,
            &self.state.sidebar_session_order,
        );

        let mut rows = vec![SidebarRow::Search];
        match self.state.sidebar_grouping {
            SidebarGrouping::Updated => {
                let mut grouped_sessions: [Vec<Uuid>; 6] = std::array::from_fn(|_| Vec::new());
                for session in sorted_sessions {
                    grouped_sessions
                        [session_date_group(sidebar_session_timestamp(session), today).index()]
                    .push(session.id);
                }
                let mut groups = SessionDateGroup::ALL;
                if self.state.sidebar_ordering == SidebarOrdering::Oldest {
                    groups.reverse();
                }
                for date_group in groups {
                    let group = SidebarGroup::Updated(date_group);
                    append_sidebar_group_rows(
                        &mut rows,
                        group,
                        &grouped_sessions[date_group.index()],
                        self.sidebar_collapsed_groups.contains(&group),
                        false,
                    );
                }
            }
            SidebarGrouping::Project => {
                let recent_cutoff = now.saturating_sub(SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS);
                let session_timestamps = sorted_sessions
                    .iter()
                    .map(|session| (session.id, sidebar_session_timestamp(session)))
                    .collect::<HashMap<_, _>>();
                let projectless_root = crate::projectless::workspace_root();
                let projectless_project_ids = self
                    .state
                    .projects
                    .iter()
                    .filter(|project| {
                        sidebar_project_is_projectless(project, projectless_root.as_deref())
                    })
                    .map(|project| project.id)
                    .collect::<HashSet<_>>();
                let mut groups = project_sidebar_groups(&sorted_sessions, &projectless_project_ids);
                let mut known = groups
                    .iter()
                    .filter_map(|(group, _)| match group {
                        SidebarGroup::Project(id) => Some(*id),
                        _ => None,
                    })
                    .collect::<HashSet<_>>();
                for project in &self.state.projects {
                    if !projectless_project_ids.contains(&project.id) && known.insert(project.id) {
                        groups.push((SidebarGroup::Project(project.id), Vec::new()));
                    }
                }
                let project_timestamps = sidebar_project_timestamps(&self.state);
                if self.state.sidebar_ordering != SidebarOrdering::Manual {
                    groups.sort_by_key(|(group, _)| {
                        let project = match group {
                            SidebarGroup::Project(id) => *id,
                            _ => Uuid::nil(),
                        };
                        let timestamp = project_timestamps
                            .get(&project)
                            .copied()
                            .unwrap_or_default();
                        (
                            matches!(group, SidebarGroup::Projectless),
                            sidebar_project_sort_key(
                                self.state.sidebar_ordering,
                                timestamp,
                                project,
                            ),
                        )
                    });
                }
                if self.state.sidebar_ordering == SidebarOrdering::Manual {
                    let ranks = self
                        .state
                        .sidebar_project_order
                        .iter()
                        .enumerate()
                        .map(|(i, id)| (*id, i))
                        .collect::<HashMap<_, _>>();
                    groups.sort_by_key(|(group, _)| match group {
                        SidebarGroup::Project(id) => {
                            (0, ranks.get(id).copied().unwrap_or(usize::MAX))
                        }
                        _ => (1, usize::MAX),
                    });
                }
                let mut sections = Vec::with_capacity(groups.len());
                for (group, sessions) in groups {
                    let mut section = Vec::new();
                    let revealed_older_sessions = self
                        .sidebar_project_reveal_counts
                        .get(&group)
                        .copied()
                        .unwrap_or_default();
                    let (visible_sessions, show_more) =
                        if self.state.sidebar_ordering == SidebarOrdering::Manual {
                            (sessions, false)
                        } else {
                            visible_project_sessions(
                                &sessions,
                                &session_timestamps,
                                recent_cutoff,
                                revealed_older_sessions,
                            )
                        };
                    if visible_sessions.is_empty() && !show_more {
                        section.extend([SidebarRow::Header(group), SidebarRow::GroupSpacer]);
                    } else {
                        append_sidebar_group_rows(
                            &mut section,
                            group,
                            &visible_sessions,
                            self.sidebar_collapsed_groups.contains(&group),
                            show_more,
                        );
                    }
                    sections.push(section);
                }
                append_sidebar_project_collections(
                    &mut rows,
                    sections,
                    &self.state.sidebar_project_groups,
                    self.state.sidebar_projects_collapsed,
                );
            }
        }
        if rows.len() == 1 {
            // Keep the header actions visible while there is no history.
            let group = match self.state.sidebar_grouping {
                SidebarGrouping::Updated => SidebarGroup::Updated(SessionDateGroup::Today),
                SidebarGrouping::Project => {
                    let projectless_root = crate::projectless::workspace_root();
                    self.state
                        .selected_project
                        .and_then(|project_id| {
                            self.state
                                .projects
                                .iter()
                                .find(|project| project.id == project_id)
                        })
                        .or_else(|| self.state.projects.first())
                        .map(|project| {
                            if sidebar_project_is_projectless(project, projectless_root.as_deref())
                            {
                                SidebarGroup::Projectless
                            } else {
                                SidebarGroup::Project(project.id)
                            }
                        })
                        .unwrap_or(SidebarGroup::Projectless)
                }
            };
            rows.push(SidebarRow::Header(group));
        }
        rows
    }

    fn animate_sidebar_reorder(
        &self,
        rows: &[SidebarRow],
        dragged: Option<SidebarRow>,
        now: Instant,
    ) {
        let mut animation = self.sidebar_reorder_animation.borrow_mut();
        *animation = SidebarReorderAnimation::between(
            &self.sidebar_row_cache.borrow(),
            rows,
            animation.as_ref(),
            dragged,
            now,
        );
    }

    /// Keep the virtualized list in sync with the current row snapshot.
    /// Rows are cheap values, so only the minimal changed suffix is spliced,
    /// preserving scroll position and measured heights across unrelated churn
    /// (e.g. the active session's `updated_at` bumping on every stream tick).
    fn sync_sidebar_rows(&self, rows: &[SidebarRow]) {
        let mut cached = self.sidebar_row_cache.borrow_mut();
        if cached.as_slice() == rows {
            return;
        }
        let prefix = cached
            .iter()
            .zip(rows.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let old_count = cached.len();
        *cached = rows.to_vec();
        if old_count == 0 {
            self.sidebar_list_state
                .reset_with_uniform_height(rows.len(), px(SIDEBAR_SESSION_ROW_HEIGHT));
        } else {
            self.sidebar_list_state
                .splice(prefix..old_count, rows.len() - prefix);
            // Newly inserted rows have no measured height yet; give them the
            // uniform hint so the scrollbar keeps a correct total height.
            self.sidebar_list_state
                .clone()
                .with_uniform_item_height(px(SIDEBAR_SESSION_ROW_HEIGHT));
        }
    }

    fn sidebar_row(
        &self,
        index: usize,
        rows: &[SidebarRow],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = rows.get(index) else {
            return div().into_any_element();
        };
        let dragged = cx.has_active_drag()
            && self
                .sidebar_drag_preview
                .borrow()
                .as_ref()
                .is_some_and(|preview| preview.row == *row);
        let element = match *row {
            SidebarRow::Search => self.render_sidebar_search(cx).into_any_element(),
            SidebarRow::Collection(id) => self.render_sidebar_collection(id, window, true, cx),
            SidebarRow::Header(group) => self
                .render_sidebar_group_header(group, cx)
                .into_any_element(),
            SidebarRow::Session(session_id) => {
                self.render_sidebar_session_item(session_id, true, cx)
            }
            SidebarRow::ShowMore(group) => {
                self.render_sidebar_show_more(group, cx).into_any_element()
            }
            SidebarRow::GroupSpacer => div()
                .w_full()
                .h(px(SIDEBAR_GROUP_SPACER_HEIGHT))
                .into_any_element(),
        };
        let offset = self
            .sidebar_reorder_animation
            .borrow()
            .as_ref()
            .and_then(|animation| {
                animation
                    .offsets
                    .get(row)
                    .map(|offset| *offset * animation.remaining)
            })
            .unwrap_or(Pixels::ZERO);
        if dragged || offset != Pixels::ZERO {
            // List rows are layout roots; apply the relative inset to a child
            // so the row keeps its measured height at the destination.
            div()
                .w_full()
                .child(
                    div()
                        .w_full()
                        .relative()
                        .top(offset)
                        .when(dragged, |row| row.opacity(0.0))
                        .child(element),
                )
                .into_any_element()
        } else {
            element
        }
    }

    fn render_sidebar_group_header(&self, group: SidebarGroup, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let collapsed = self.sidebar_collapsed_groups.contains(&group);
        let working = collapsed
            && self
                .sidebar_working_headers
                .borrow()
                .contains(&SidebarRow::Header(group));
        let group_key = group.element_key();
        let group_name = SharedString::from(format!("sidebar-group-header-{group_key}"));
        let header_focus = self
            .sidebar_group_header_focuses
            .borrow_mut()
            .entry(group)
            .or_insert_with(|| cx.focus_handle())
            .clone();
        let show_folder_icon =
            matches!(group, SidebarGroup::Project(_) | SidebarGroup::Projectless);
        let folder_icon = if collapsed {
            "icons/folder.svg"
        } else {
            "icons/folder-open.svg"
        };
        let label = match group {
            SidebarGroup::Updated(group) => group.label(),
            SidebarGroup::Project(project_id) => self
                .state
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .map(Project::display_name)
                .unwrap_or_else(|| tr!("project.no_project_name")),
            SidebarGroup::Projectless => tr!("project.no_project_name"),
        };
        let updated_chevron = matches!(group, SidebarGroup::Updated(_)).then(|| {
            icon("icons/chevron-down.svg", 14.0, theme.text_secondary)
                .when(collapsed, |icon| {
                    icon.with_transformation(gpui::Transformation::rotate(gpui::percentage(0.75)))
                })
                .invisible()
                .group_hover(group_name.clone(), |icon| icon.visible())
        });
        let compose = show_folder_icon.then(|| {
            let compose_focus = self
                .sidebar_group_compose_focuses
                .borrow_mut()
                .entry(group)
                .or_insert_with(|| cx.focus_handle())
                .clone();
            div()
                .w(px(20.0))
                .h(px(22.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "sidebar-group-compose-{group_key}"
                        )))
                        .track_focus(&compose_focus)
                        .tab_index(0)
                        .tab_stop(true)
                        .w_0()
                        .h(px(22.0))
                        .overflow_hidden()
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_default()
                        .opacity(0.0)
                        .group_hover(group_name.clone(), |style| style.w(px(20.0)).opacity(1.0))
                        .focus_visible(|style| {
                            style
                                .w(px(20.0))
                                .opacity(1.0)
                                .shadow(vec![focus_ring(theme.accent)])
                        })
                        .hover(|style| style.bg(theme.overlay))
                        .active(|style| style.bg(theme.overlay_strong))
                        .tooltip(Tooltip::text(tr!("menu.new_task")))
                        .child(icon("icons/compose.svg", 14.0, theme.text_secondary))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.open_new_task_for_sidebar_group(group, window, cx);
                        }))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                this.open_new_task_for_sidebar_group(group, window, cx);
                                cx.stop_propagation();
                            }
                        })),
                )
        });

        let header = session_group_header(&theme)
            .id(SharedString::from(format!(
                "sidebar-group-toggle-{group_key}"
            )))
            .track_focus(&header_focus)
            .tab_index(0)
            .tab_group()
            .tab_stop(true)
            .group(group_name)
            .relative()
            .w_full()
            .rounded(px(6.0))
            .when(show_folder_icon, |header| {
                header.pl(px(8.0)).font_weight(FontWeight::NORMAL)
            })
            .when(group == SidebarGroup::Projectless, |header| {
                header.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            })
            .cursor_default()
            .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
            .hover(|style| style.bg(theme.sidebar_item_background))
            .active(|style| style.bg(theme.overlay_strong))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .when(show_folder_icon, |element| {
                        element.child(icon(folder_icon, 14.0, theme.text_secondary))
                    })
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .child(div().min_w_0().truncate().child(label.clone()))
                            .text_color(theme.text)
                            .when_some(updated_chevron, |element, chevron| element.child(chevron)),
                    )
                    .child(div().flex_1()),
            )
            .when_some(compose, |element, compose| element.child(compose))
            .when(working, |element| {
                element.child(
                    div()
                        .ml(px(6.0))
                        .w(px(14.0))
                        .flex_none()
                        .child(motion::spin_slow(icon(
                            "icons/loader-circle.svg",
                            14.0,
                            theme.text,
                        ))),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_sidebar_group(group, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if this.sidebar_navigation_key_down(SidebarRow::Header(group), event, window, cx) {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "left" if !collapsed => {
                        this.set_sidebar_group_collapsed(group, true, cx);
                        cx.stop_propagation();
                    }
                    "right" if collapsed => {
                        this.set_sidebar_group_collapsed(group, false, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }));

        let header =
            self.sidebar_reorderable_row(header, SidebarRow::Header(group), label.into(), cx);
        let header = if let SidebarGroup::Project(project) = group {
            let menu = self.menu_handle(format!("sidebar-project-{project}"), cx);
            let keyboard_menu = menu.clone();
            let weak = cx.entity().downgrade();
            context_menu(
                header.capture_key_down(move |event, window, cx| {
                    if (event.keystroke.key == "f10" && event.keystroke.modifiers.shift)
                        || (event.keystroke.key == "enter" && event.keystroke.modifiers.control)
                    {
                        keyboard_menu.open_context_menu(window, cx);
                        cx.stop_propagation();
                    }
                }),
                format!("sidebar-project-menu-{project}"),
                &menu,
                move |cx| {
                    weak.upgrade()
                        .map(|michelle| {
                            michelle
                                .read(cx)
                                .sidebar_project_menu_items(project, weak.clone())
                        })
                        .unwrap_or_default()
                },
            )
        } else {
            header.into_any_element()
        };
        div()
            .w_full()
            .pb(px(SIDEBAR_GROUP_HEADER_BOTTOM_GAP))
            .child(header)
    }

    fn sidebar_project_menu_items(&self, project: Uuid, weak: WeakEntity<Self>) -> Vec<MenuItem> {
        let selected = self.state.sidebar_group_for_project(project);
        let groups = self
            .state
            .sidebar_project_groups
            .iter()
            .map(|group| (Some(group.id), group.name.clone()))
            .chain(std::iter::once((None, tr!("sidebar.projects"))))
            .collect::<Vec<_>>();
        vec![MenuItem::submenu_with_value(
            tr!("sidebar.move_to_group"),
            groups
                .iter()
                .find(|(id, _)| *id == selected)
                .map(|(_, name)| name.clone())
                .unwrap_or_default(),
            move |_| {
                groups
                    .iter()
                    .map(|(id, name)| {
                        let weak = weak.clone();
                        let id = *id;
                        MenuItem::new(name.clone(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.move_sidebar_project_to_collection(project, id, cx);
                            });
                        })
                        .selected(id == selected)
                    })
                    .collect()
            },
        )]
    }

    fn render_sidebar_collection(
        &self,
        id: Option<Uuid>,
        window: &Window,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let group = id.and_then(|id| {
            self.state
                .sidebar_project_groups
                .iter()
                .find(|group| group.id == id)
        });
        let label = group
            .map(|group| group.name.clone())
            .unwrap_or_else(|| tr!("sidebar.projects"));
        let collapsed = group.map_or(self.state.sidebar_projects_collapsed, |group| {
            group.collapsed
        });
        let working = collapsed
            && self
                .sidebar_working_headers
                .borrow()
                .contains(&SidebarRow::Collection(id));
        let menu = self.menu_handle(format!("project-collection-{id:?}"), cx);
        let keyboard_menu = menu.clone();
        let group_name = SharedString::from(format!("project-collection-header-{id:?}"));
        let renaming = interactive && id.is_some() && self.sidebar_project_group_rename == id;
        let title = if renaming {
            div()
                .key_context(SESSION_RENAME_PARENT_CONTEXT)
                .on_action(cx.listener(|this, _: &CancelSessionRename, window, cx| {
                    this.cancel_session_rename(window, cx)
                }))
                .flex_1()
                .min_w_0()
                .border_1()
                .border_color(theme.accent)
                .rounded(px(4.0))
                .bg(theme.inset)
                .child(self.session_rename_input.clone())
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(label.clone())
                .into_any_element()
        };
        let header = session_group_header(&theme)
            .id(group_name.clone())
            .group(group_name.clone())
            .h(px(SIDEBAR_COLLECTION_HEADER_HEIGHT))
            .pl(px(8.0))
            .pr(px(8.0))
            .py(px(2.0))
            .relative()
            .w_full()
            .rounded(px(6.0))
            .gap(px(12.0))
            .cursor_default()
            .child(title)
            .child(
                icon("icons/chevron-down.svg", 14.0, theme.text_secondary)
                    .flex_none()
                    .when(collapsed, |icon| {
                        icon.with_transformation(gpui::Transformation::rotate(gpui::percentage(
                            0.75,
                        )))
                    })
                    .invisible()
                    .group_hover(group_name, |icon| icon.visible())
                    .when(
                        window.last_input_was_keyboard()
                            && menu.trigger_focus_handle().is_focused(window),
                        |icon| icon.visible(),
                    ),
            )
            .when(working, |header| {
                header.child(motion::spin_slow(icon(
                    "icons/loader-circle.svg",
                    14.0,
                    theme.text,
                )))
            })
            .when(interactive && !renaming, |header| {
                header
                    .track_focus(menu.trigger_focus_handle())
                    .tab_index(0)
                    .tab_group()
                    .tab_stop(true)
                    .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
                    .hover(|style| style.bg(theme.sidebar_item_background))
                    // GPUI emits a click on Enter/Space release; handle activation only here.
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if let Some(id) = sidebar_collection_rename_target(id, event) {
                            this.begin_sidebar_collection_rename(id, window, cx);
                        } else {
                            this.set_sidebar_collection_collapsed(id, !collapsed, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if this.sidebar_navigation_key_down(
                            SidebarRow::Collection(id),
                            event,
                            window,
                            cx,
                        ) {
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        if (key == "f10" && event.keystroke.modifiers.shift)
                            || (key == "enter" && event.keystroke.modifiers.control)
                        {
                            keyboard_menu.open_context_menu(window, cx);
                            cx.stop_propagation();
                        } else if key == "left" || key == "right" {
                            this.set_sidebar_collection_collapsed(id, key == "left", cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .when(renaming, |header| {
                header
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| this.commit_session_rename(cx)))
            });
        if !interactive {
            return div()
                .w_full()
                .pb(px(SIDEBAR_GROUP_HEADER_BOTTOM_GAP))
                .child(header)
                .into_any_element();
        }
        let header = if renaming {
            header
        } else {
            self.sidebar_reorderable_row(header, SidebarRow::Collection(id), label.into(), cx)
        };
        let weak = cx.entity().downgrade();
        context_menu(
            div()
                .w_full()
                .pb(px(SIDEBAR_GROUP_HEADER_BOTTOM_GAP))
                .child(header),
            format!("project-collection-menu-{id:?}"),
            &menu,
            move |_| {
                let create = weak.clone();
                let mut items = vec![MenuItem::new(
                    tr!("sidebar.new_group"),
                    move |window, cx| {
                        let _ = create
                            .update(cx, |this, cx| this.create_sidebar_collection(window, cx));
                    },
                )];
                if let Some(id) = id {
                    let rename = weak.clone();
                    let remove = weak.clone();
                    items.extend([
                        MenuItem::new(tr!("common.rename"), move |window, cx| {
                            let _ = rename.update(cx, |this, cx| {
                                this.begin_sidebar_collection_rename(id, window, cx)
                            });
                        }),
                        MenuItem::new(tr!("common.remove"), move |window, cx| {
                            let _ = remove.update(cx, |this, cx| {
                                if this.state.remove_sidebar_project_group(id) {
                                    if this.sidebar_project_group_rename == Some(id) {
                                        this.sidebar_project_group_rename = None;
                                    }
                                    this.save_sidebar_presentation(cx);
                                    this.focus_sidebar_row(
                                        SidebarRow::Collection(None),
                                        window,
                                        cx,
                                    );
                                }
                            });
                        }),
                    ]);
                }
                items
            },
        )
    }

    fn save_sidebar_presentation(&mut self, cx: &mut Context<Self>) {
        self.sidebar_rows_fingerprint.set(None);
        self.save();
        cx.notify();
    }

    fn move_sidebar_project_to_collection(
        &mut self,
        project: Uuid,
        target: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        if self.state.sidebar_group_for_project(project) == target {
            return;
        }
        let manual = self.state.sidebar_ordering == SidebarOrdering::Manual;
        let before = if manual {
            self.remember_sidebar_manual_order();
            let membership = self
                .state
                .sidebar_project_groups
                .iter()
                .flat_map(|group| group.projects.iter().map(move |id| (*id, group.id)))
                .collect::<HashMap<_, _>>();
            self.state
                .sidebar_project_order
                .iter()
                .copied()
                .find(|id| *id != project && membership.get(id).copied() == target)
        } else {
            None
        };
        if self.state.move_project_to_sidebar_group(project, target) {
            if manual {
                self.state.sidebar_project_order.retain(|id| *id != project);
                let index = before
                    .and_then(|id| {
                        self.state
                            .sidebar_project_order
                            .iter()
                            .position(|item| *item == id)
                    })
                    .unwrap_or(self.state.sidebar_project_order.len());
                self.state.sidebar_project_order.insert(index, project);
            }
            self.save_sidebar_presentation(cx);
        }
    }

    fn set_sidebar_collection_collapsed(
        &mut self,
        id: Option<Uuid>,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) {
        let value = if let Some(id) = id {
            let Some(group) = self
                .state
                .sidebar_project_groups
                .iter_mut()
                .find(|group| group.id == id)
            else {
                return;
            };
            &mut group.collapsed
        } else {
            &mut self.state.sidebar_projects_collapsed
        };
        if *value != collapsed {
            *value = collapsed;
            self.save_sidebar_presentation(cx);
        }
    }

    fn create_sidebar_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_session_rename(cx);
        if let Some(id) = self
            .state
            .create_sidebar_project_group(&tr!("sidebar.new_group_name"))
        {
            self.save_sidebar_presentation(cx);
            self.begin_sidebar_collection_rename(id, window, cx);
        }
    }

    fn begin_sidebar_collection_rename(
        &mut self,
        id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_session_rename(cx);
        let Some(name) = self
            .state
            .sidebar_project_groups
            .iter()
            .find(|group| group.id == id)
            .map(|group| group.name.clone())
        else {
            return;
        };
        self.sidebar_project_group_rename = Some(id);
        self.session_rename_input.update(cx, |input, cx| {
            input.set_content(name, cx);
            input.select_all_text(cx);
        });
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        self.sync_sidebar_rows(&rows);
        if let Some(index) = rows
            .iter()
            .position(|row| *row == SidebarRow::Collection(Some(id)))
        {
            reveal_sidebar_list_row(&self.sidebar_list_state, &rows, index);
        }
        let focus = self.session_rename_input.read(cx).focus();
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    fn open_new_task_for_sidebar_group(
        &mut self,
        group: SidebarGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_page = None;
        match group {
            SidebarGroup::Project(project_id) => self.select_project(project_id, cx),
            SidebarGroup::Projectless => self.create_projectless_session(cx),
            SidebarGroup::Updated(_) => return,
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
    }

    fn render_sidebar_show_more(&self, group: SidebarGroup, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let group_key = group.element_key();
        let focus = self
            .sidebar_show_more_focuses
            .borrow_mut()
            .entry(group)
            .or_insert_with(|| cx.focus_handle())
            .clone();
        let button = div()
            .id(SharedString::from(format!("sidebar-show-more-{group_key}")))
            .track_focus(&focus)
            .tab_index(0)
            .tab_stop(true)
            .flex_none()
            .cursor_default()
            .text_size(sp(12.5))
            .text_color(theme.text_tertiary)
            .focus_visible(|style| style.text_color(theme.text))
            .hover(|style| style.text_color(theme.text))
            .child(tr!("sidebar.show_more"))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.show_more_project_sessions(group, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if this.sidebar_navigation_key_down(SidebarRow::ShowMore(group), event, window, cx)
                {
                    return;
                }
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    let rows = this.sidebar_rows_cached(Local::now().date_naive(), unix_time());
                    let index = rows
                        .iter()
                        .position(|row| *row == SidebarRow::ShowMore(group));
                    this.show_more_project_sessions(group, cx);
                    let rows = this.sidebar_rows_cached(Local::now().date_naive(), unix_time());
                    if let Some(row) = index.and_then(|index| rows.get(index)) {
                        this.focus_sidebar_row(*row, window, cx);
                    }
                    cx.stop_propagation();
                }
            }));

        div()
            .w_full()
            .h(px(SIDEBAR_SHOW_MORE_ROW_HEIGHT))
            .pl(px(SIDEBAR_GROUP_CHILD_PADDING))
            .flex()
            .items_center()
            .child(button)
    }

    fn show_more_project_sessions(&mut self, group: SidebarGroup, cx: &mut Context<Self>) {
        let revealed = self.sidebar_project_reveal_counts.entry(group).or_default();
        *revealed = revealed.saturating_add(SIDEBAR_PROJECT_REVEAL_BATCH);
        self.sidebar_rows_fingerprint.set(None);
        cx.notify();
    }

    fn toggle_sidebar_group(&mut self, group: SidebarGroup, cx: &mut Context<Self>) {
        let collapsed = !self.sidebar_collapsed_groups.contains(&group);
        self.set_sidebar_group_collapsed(group, collapsed, cx);
    }

    pub(super) fn collapse_all_sidebar_groups(&mut self, cx: &mut Context<Self>) {
        let groups = self
            .sidebar_rows_cached(Local::now().date_naive(), unix_time())
            .iter()
            .filter_map(|row| match row {
                SidebarRow::Header(group) => Some(*group),
                _ => None,
            })
            .collect::<Vec<_>>();
        let collections_changed = self.state.sidebar_grouping == SidebarGrouping::Project
            && (!self.state.sidebar_projects_collapsed
                || self
                    .state
                    .sidebar_project_groups
                    .iter()
                    .any(|group| !group.collapsed));
        if self.state.sidebar_grouping == SidebarGrouping::Project {
            self.state.sidebar_projects_collapsed = true;
            for group in &mut self.state.sidebar_project_groups {
                group.collapsed = true;
            }
        }
        let mut changed = false;
        for group in groups {
            changed |= self.sidebar_collapsed_groups.insert(group);
            changed |= self.sidebar_project_reveal_counts.remove(&group).is_some();
        }
        if changed || collections_changed {
            self.sidebar_rows_fingerprint.set(None);
            if self.update_sidebar_collapsed_projects() || collections_changed {
                self.save();
            }
            cx.notify();
        }
    }

    fn set_sidebar_group_collapsed(
        &mut self,
        group: SidebarGroup,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) {
        let collapse_changed = if collapsed {
            self.sidebar_collapsed_groups.insert(group)
        } else {
            self.sidebar_collapsed_groups.remove(&group)
        };
        let reveal_reset = collapsed && self.sidebar_project_reveal_counts.remove(&group).is_some();
        if collapse_changed || reveal_reset {
            self.sidebar_rows_fingerprint.set(None);
            if self.update_sidebar_collapsed_projects() {
                self.save();
            }
            cx.notify();
        }
    }

    fn update_sidebar_collapsed_projects(&mut self) -> bool {
        let collapsed_projects = self
            .sidebar_collapsed_groups
            .iter()
            .filter_map(|group| match group {
                SidebarGroup::Project(id) => Some(*id),
                _ => None,
            })
            .collect();
        if self.state.sidebar_collapsed_projects != collapsed_projects {
            self.state.sidebar_collapsed_projects = collapsed_projects;
            true
        } else {
            false
        }
    }

    fn set_sidebar_grouping(&mut self, grouping: SidebarGrouping, cx: &mut Context<Self>) {
        if self.state.sidebar_grouping == grouping {
            return;
        }
        self.state.sidebar_grouping = grouping;
        self.sidebar_rows_fingerprint.set(None);
        self.sidebar_branch_scan_fingerprint.set(None);
        self.sidebar_branch_scan_generation
            .set(self.sidebar_branch_scan_generation.get().wrapping_add(1));
        self.sidebar_list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: Pixels::ZERO,
        });
        self.save();
        cx.notify();
    }

    fn set_sidebar_ordering(&mut self, ordering: SidebarOrdering, cx: &mut Context<Self>) {
        if self.state.sidebar_ordering == ordering {
            return;
        }
        if ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
        }
        self.state.sidebar_ordering = ordering;
        self.sidebar_rows_fingerprint.set(None);
        self.sidebar_list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: Pixels::ZERO,
        });
        self.save();
        cx.notify();
    }

    fn remember_sidebar_manual_order(&mut self) {
        let mut sessions = self
            .state
            .sessions
            .iter()
            .filter(|session| session.has_started())
            .collect::<Vec<_>>();
        let ordering = if self.state.sidebar_session_order.is_empty() {
            self.state.sidebar_ordering
        } else {
            SidebarOrdering::Manual
        };
        sort_sidebar_sessions(&mut sessions, ordering, &self.state.sidebar_session_order);
        self.state.sidebar_session_order = sessions.iter().map(|session| session.id).collect();
        let mut project_order = self.state.sidebar_project_order.clone();
        let valid_projects = self
            .state
            .projects
            .iter()
            .map(|project| project.id)
            .collect::<HashSet<_>>();
        project_order.retain(|id| valid_projects.contains(id));
        let mut known = project_order.iter().copied().collect::<HashSet<_>>();
        for id in sessions
            .iter()
            .map(|session| session.project_id)
            .chain(self.state.projects.iter().map(|project| project.id))
        {
            if known.insert(id) {
                project_order.push(id);
            }
        }
        self.state.sidebar_project_order = project_order;
    }

    fn reorder_sidebar_row(
        &mut self,
        source: SidebarRow,
        target: SidebarRow,
        cx: &mut Context<Self>,
    ) {
        match (source, target) {
            (SidebarRow::Collection(Some(source)), SidebarRow::Collection(target)) => {
                if self.state.reorder_sidebar_project_group(source, target) {
                    self.save_sidebar_presentation(cx);
                }
                return;
            }
            (SidebarRow::Header(SidebarGroup::Project(source)), SidebarRow::Collection(target)) => {
                self.move_sidebar_project_to_collection(source, target, cx);
                return;
            }
            _ => {}
        }
        let rows = self.sidebar_rows_cached(Local::now().date_naive(), unix_time());
        let Some(source_index) = rows.iter().position(|row| *row == source) else {
            return;
        };
        let Some(target_index) = rows.iter().position(|row| *row == target) else {
            return;
        };
        if !sidebar_reorder_siblings(&rows, source).contains(&target) {
            return;
        }
        if self.state.sidebar_ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
        }
        let changed = match (source, target) {
            (SidebarRow::Session(source), SidebarRow::Session(target))
                if self.state.sidebar_ordering == SidebarOrdering::Manual =>
            {
                move_sidebar_item(
                    &mut self.state.sidebar_session_order,
                    source,
                    target,
                    source_index < target_index,
                )
            }
            (
                SidebarRow::Header(SidebarGroup::Project(source)),
                SidebarRow::Header(SidebarGroup::Project(target)),
            ) => {
                let destination = self.state.sidebar_group_for_project(target);
                let transferred = self
                    .state
                    .move_project_to_sidebar_group(source, destination);
                let reordered = self.state.sidebar_ordering == SidebarOrdering::Manual
                    && move_sidebar_item(
                        &mut self.state.sidebar_project_order,
                        source,
                        target,
                        source_index < target_index,
                    );
                transferred || reordered
            }
            _ => false,
        };
        if changed {
            self.sidebar_rows_fingerprint.set(None);
            self.save();
            cx.notify();
        }
    }

    fn commit_sidebar_drag(&mut self, drag: &SidebarDrag, cx: &mut Context<Self>) {
        if matches!(drag.row, SidebarRow::Header(SidebarGroup::Project(_))) {
            self.commit_sidebar_project_drag(drag, cx);
            return;
        }
        let preview = drag.preview.borrow();
        let Some(preview) = preview.as_ref() else {
            return;
        };
        let changed = match drag.row {
            SidebarRow::Collection(Some(_)) => {
                let ranks = preview
                    .rows
                    .iter()
                    .filter_map(|row| match row {
                        SidebarRow::Collection(Some(id)) => Some(*id),
                        _ => None,
                    })
                    .enumerate()
                    .map(|(index, id)| (id, index))
                    .collect::<HashMap<_, _>>();
                let old = self
                    .state
                    .sidebar_project_groups
                    .iter()
                    .map(|group| group.id)
                    .collect::<Vec<_>>();
                self.state
                    .sidebar_project_groups
                    .sort_by_key(|group| ranks.get(&group.id).copied().unwrap_or(usize::MAX));
                old.into_iter().ne(self
                    .state
                    .sidebar_project_groups
                    .iter()
                    .map(|group| group.id))
            }
            SidebarRow::Session(source)
                if self.state.sidebar_ordering == SidebarOrdering::Manual =>
            {
                let siblings = sidebar_reorder_siblings(&preview.rows, drag.row);
                if siblings == preview.siblings {
                    return;
                }
                let Some(index) = siblings.iter().position(|row| *row == drag.row) else {
                    return;
                };
                self.remember_sidebar_manual_order();
                let target = siblings
                    .get(index + 1)
                    .map(|row| (*row, false))
                    .or_else(|| index.checked_sub(1).map(|index| (siblings[index], true)));
                match target {
                    Some((SidebarRow::Session(target), after)) => move_sidebar_item(
                        &mut self.state.sidebar_session_order,
                        source,
                        target,
                        after,
                    ),
                    _ => false,
                }
            }
            _ => false,
        };
        if changed {
            self.save_sidebar_presentation(cx);
        }
    }

    fn commit_sidebar_project_drag(&mut self, drag: &SidebarDrag, cx: &mut Context<Self>) {
        let SidebarRow::Header(SidebarGroup::Project(project)) = drag.row else {
            return;
        };
        let destination = drag
            .preview
            .borrow()
            .as_ref()
            .and_then(|preview| preview.destination);
        let Some((collection, before)) = destination else {
            return;
        };
        let transferred = self
            .state
            .move_project_to_sidebar_group(project, collection);
        let mut reordered = false;
        if self.state.sidebar_ordering == SidebarOrdering::Manual {
            self.remember_sidebar_manual_order();
            let old_order = self.state.sidebar_project_order.clone();
            self.state.sidebar_project_order.retain(|id| *id != project);
            let index = before
                .and_then(|before| {
                    self.state
                        .sidebar_project_order
                        .iter()
                        .position(|id| *id == before)
                })
                .unwrap_or(self.state.sidebar_project_order.len());
            self.state.sidebar_project_order.insert(index, project);
            reordered = self.state.sidebar_project_order != old_order;
        }
        if transferred || reordered {
            self.save_sidebar_presentation(cx);
        }
    }

    fn sidebar_reorderable_row(
        &self,
        element: Stateful<Div>,
        row: SidebarRow,
        label: SharedString,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element.when(
            matches!(
                row,
                SidebarRow::Collection(Some(_)) | SidebarRow::Header(SidebarGroup::Project(_))
            ) || (self.state.sidebar_ordering == SidebarOrdering::Manual
                && matches!(row, SidebarRow::Session(_))),
            |element| {
                let rows = self.sidebar_rows_snapshot.borrow().clone();
                let michelle = cx.entity().downgrade();
                let row_bounds = Rc::new(Cell::new(Bounds::default()));
                let drag_bounds = row_bounds.clone();
                element
                    .relative()
                    .on_drag(
                        SidebarDrag {
                            row,
                            label,
                            michelle: michelle.clone(),
                            preview: Rc::default(),
                        },
                        move |drag, offset, window, cx| {
                            let _ = michelle.update(cx, |this, cx| {
                                let collapsed = match row {
                                    SidebarRow::Header(group) => {
                                        this.sidebar_collapsed_groups.contains(&group)
                                    }
                                    SidebarRow::Collection(Some(id)) => this
                                        .state
                                        .sidebar_project_groups
                                        .iter()
                                        .find(|group| group.id == id)
                                        .is_some_and(|group| group.collapsed),
                                    _ => false,
                                };
                                *drag.preview.borrow_mut() = Some(SidebarDragPreview {
                                    row,
                                    siblings: sidebar_reorder_siblings(&rows, row),
                                    rows: rows.clone(),
                                    offsets: sidebar_row_offsets(&rows),
                                    ordering: this.state.sidebar_ordering,
                                    timestamps: if matches!(row, SidebarRow::Header(_)) {
                                        sidebar_project_timestamps(&this.state)
                                    } else {
                                        HashMap::new()
                                    },
                                    destination: None,
                                    bounds: drag_bounds.get(),
                                    cursor_offset: offset,
                                    collapsed,
                                    working: collapsed
                                        && this.sidebar_working_headers.borrow().contains(&row),
                                });
                                this.sidebar_drag_preview = drag.preview.clone();
                                this.focus_sidebar_row(row, window, cx);
                            });
                            cx.new(|cx| {
                                cx.on_release(|drag: &mut SidebarDrag, _| {
                                    drag.preview.borrow_mut().take();
                                })
                                .detach();
                                drag.clone()
                            })
                        },
                    )
                    .child(
                        canvas(move |bounds, _, _| row_bounds.set(bounds), |_, _, _, _| ())
                            .absolute()
                            .inset_0(),
                    )
            },
        )
    }

    fn begin_session_rename(
        &mut self,
        session_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_session_rename(cx);
        let Some(title) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(localized_session_title)
        else {
            return;
        };

        self.session_rename = Some(session_id);
        self.session_rename_input.update(cx, |input, cx| {
            input.set_content(title, cx);
            input.select_all_text(cx);
        });
        let focus = self.session_rename_input.read(cx).focus();
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    pub(super) fn commit_session_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.sidebar_project_group_rename.take() {
            let name = self.session_rename_input.read(cx).content().to_owned();
            if self.state.rename_sidebar_project_group(id, &name) {
                self.save_sidebar_presentation(cx);
            }
            cx.notify();
            return;
        }
        let Some(session_id) = self.session_rename.take() else {
            return;
        };
        let title = self
            .session_rename_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let should_update = !title.is_empty()
            && self
                .state
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .is_some_and(|session| session.title != title);
        if should_update
            && self
                .state
                .session_mut(session_id)
                .is_some_and(|session| session.set_title(&title))
        {
            self.save();
        }
        cx.notify();
    }

    pub(super) fn finish_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let group_id = self.sidebar_project_group_rename;
        let session_id = self.session_rename;
        self.commit_session_rename(cx);
        if let Some(id) = session_id {
            self.focus_sidebar_row(SidebarRow::Session(id), window, cx);
        }
        if let Some(id) = group_id {
            self.focus_sidebar_row(SidebarRow::Collection(Some(id)), window, cx);
        }
    }

    fn cancel_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.sidebar_project_group_rename.take() {
            self.focus_sidebar_row(SidebarRow::Collection(Some(id)), window, cx);
        }
        if let Some(id) = self.session_rename.take() {
            self.focus_sidebar_row(SidebarRow::Session(id), window, cx);
        }
    }

    fn render_sidebar_session_item(
        &self,
        session_id: Uuid,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let Some(session) = self
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
        else {
            return div().into_any_element();
        };
        let selected = sidebar_session_selected(
            self.state.selected_session,
            self.pending_session_activation
                .map(|pending| pending.session_id),
            session_id,
        );
        let working = matches!(
            session.status,
            SessionStatus::Connecting | SessionStatus::Working
        );
        let project = self
            .state
            .projects
            .iter()
            .find(|project| project.id == session.project_id);
        let grouped_by_project = self.state.sidebar_grouping == SidebarGrouping::Project;
        let left_padding = if grouped_by_project {
            SIDEBAR_GROUP_CHILD_PADDING
        } else {
            8.0
        };
        let detail_label = if grouped_by_project {
            persisted_sidebar_branch_label(&session.workspace)
                .map(|branch| SharedString::from(branch.to_owned()))
                .or_else(|| {
                    if !matches!(&session.workspace, SessionWorkspace::Local) {
                        return None;
                    }
                    project.and_then(|project| {
                        self.sidebar_branch_labels
                            .borrow()
                            .get(&project.path)
                            .cloned()
                    })
                })
        } else {
            Some(SharedString::from(
                project
                    .map(Project::display_name)
                    .unwrap_or_else(|| tr!("sidebar.unknown_project")),
            ))
        };
        let has_detail_label = detail_label.is_some();
        let detail_icon = if grouped_by_project {
            "icons/git-branch.svg"
        } else {
            "icons/folder.svg"
        };
        let rename_input = (interactive && self.session_rename == Some(session_id))
            .then(|| self.session_rename_input.clone());
        let renaming = rename_input.is_some();
        let title = if let Some(rename_input) = rename_input {
            div()
                .id(SharedString::from(format!(
                    "session-rename-field-{session_id}"
                )))
                .key_context(SESSION_RENAME_PARENT_CONTEXT)
                .on_action(cx.listener(|this, _: &CancelSessionRename, window, cx| {
                    this.cancel_session_rename(window, cx);
                }))
                .h(px(18.0))
                .flex_1()
                .min_w_0()
                .px(px(4.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(theme.accent)
                .bg(theme.inset)
                .flex()
                .items_center()
                .text_size(sp(13.5))
                .text_color(theme.text)
                .child(rename_input)
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .line_clamp(1)
                .text_overflow(gpui::TextOverflow::Truncate("...".into()))
                .text_size(sp(13.5))
                .text_color(theme.text)
                .child(SharedString::from(localized_session_title(session)))
                .into_any_element()
        };
        let michelle = cx.entity().downgrade();
        let menu = self.menu_handle(format!("session-{session_id}"), cx);
        let row_focus = menu.trigger_focus_handle().clone();
        let keyboard_menu = menu.clone();
        let row = div()
            .id(SharedString::from(format!("session-{}", session.id)))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .pl(px(left_padding))
            .pr(px(8.0))
            .py(px(7.0))
            .rounded(px(7.0))
            .cursor_default()
            .when(interactive, |element| {
                element
                    .when(selected, |element| {
                        element.bg(theme.sidebar_item_background)
                    })
                    .hover(|element| element.bg(theme.sidebar_item_background))
                    .active(|element| element.bg(theme.sidebar_item_background))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    .line_height(sp(18.0))
                    .child(title)
                    .when(working, |element| {
                        element.child(motion::spin_slow(icon(
                            "icons/loader-circle.svg",
                            12.0,
                            status_color(&theme, session.status),
                        )))
                    })
                    .when(session.status == SessionStatus::Background, |element| {
                        element.child(icon(
                            "icons/hourglass.svg",
                            12.0,
                            status_color(&theme, session.status),
                        ))
                    })
                    .when(session.status == SessionStatus::Waiting, |element| {
                        element.child(icon(
                            "icons/alert.svg",
                            12.0,
                            status_color(&theme, session.status),
                        ))
                    })
                    .when(session.status == SessionStatus::Failed, |element| {
                        element.child(icon(
                            "icons/x.svg",
                            12.0,
                            status_color(&theme, session.status),
                        ))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .text_size(sp(if grouped_by_project { 12.5 } else { 13.0 }))
                    .line_height(sp(15.0))
                    .when_some(detail_label, |element, label| {
                        element
                            .child(icon(detail_icon, 12.5, theme.text_tertiary))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(theme.text_tertiary)
                                    .child(label),
                            )
                    })
                    .when(!has_detail_label, |element| element.child(div().flex_1()))
                    .when_some(
                        session_time_label(session, unix_time()),
                        |element, label| {
                            element.child(
                                div()
                                    .flex_none()
                                    .text_size(sp(12.5))
                                    .font_features(TABULAR_NUMBERS.clone())
                                    .text_color(if session.is_busy() {
                                        theme.text_tertiary
                                    } else {
                                        theme.text_ghost
                                    })
                                    .child(SharedString::from(label)),
                            )
                        },
                    ),
            )
            .when(interactive && !renaming, |element| {
                element
                    .track_focus(&row_focus)
                    .tab_index(0)
                    .tab_stop(true)
                    .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if this.sidebar_navigation_key_down(
                            SidebarRow::Session(session_id),
                            event,
                            window,
                            cx,
                        ) {
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        if (key == "enter" && event.keystroke.modifiers.control)
                            || (key == "f10" && event.keystroke.modifiers.shift)
                        {
                            keyboard_menu.open_context_menu(window, cx);
                            cx.stop_propagation();
                        } else if key == "enter" {
                            this.begin_session_rename(session_id, window, cx);
                            cx.stop_propagation();
                        } else if key == "space" {
                            this.select_session(session_id, cx);
                            this.focus_composer_action(&FocusComposer, window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_session(session_id, cx);
                    }))
            });
        if !interactive {
            return div()
                .w_full()
                .pb(px(SIDEBAR_SESSION_ROW_GAP))
                .child(row)
                .into_any_element();
        }
        let row = if !renaming {
            self.sidebar_reorderable_row(
                row,
                SidebarRow::Session(session_id),
                localized_session_title(session).into(),
                cx,
            )
        } else {
            row
        };
        let row = if renaming {
            div()
                .w_full()
                .child(row)
                .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                    if this.session_rename == Some(session_id) {
                        this.commit_session_rename(cx);
                    }
                }))
                .into_any_element()
        } else {
            context_menu(
                div().w_full().child(row),
                SharedString::from(format!("session-menu-{session_id}")),
                &menu,
                move |_| {
                    let rename_michelle = michelle.clone();
                    let remove_michelle = michelle.clone();
                    vec![
                        MenuItem::new(tr!("common.rename"), move |window, cx| {
                            let _ = rename_michelle.update(cx, |michelle, cx| {
                                michelle.begin_session_rename(session_id, window, cx);
                            });
                        }),
                        MenuItem::Separator,
                        MenuItem::new(tr!("common.remove"), move |_, cx| {
                            let _ = remove_michelle
                                .update(cx, |michelle, cx| michelle.remove_session(session_id, cx));
                        }),
                    ]
                },
            )
        };

        div()
            .w_full()
            .pb(px(SIDEBAR_SESSION_ROW_GAP))
            .child(row)
            .into_any_element()
    }

    // ── Header ─────────────────────────────────────────────────────────────

    pub(super) fn render_header(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::current(cx);
        let session = self.selected_session();
        let title = session
            .map(localized_session_title)
            .unwrap_or_else(|| tr!("session.new_task"));
        let agent_preset_label = session
            .filter(|session| session.provider == ProviderKind::DeepSeek && session.has_started())
            .and_then(|session| self.agent_preset_label_for_session(session));
        let left_window_controls = (!self.sidebar_visible)
            .then(|| {
                self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Left,
                    window,
                    cx,
                )
            })
            .flatten();
        let right_window_controls = (!self.right_panel_visible)
            .then(|| {
                self.render_client_window_controls(
                    super::window_chrome::WindowControlSide::Right,
                    window,
                    cx,
                )
            })
            .flatten();
        div()
            .id("window-header")
            .h(px(48.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .children(left_window_controls)
            // The header starts where the sidebar ends, so until the sidebar
            // is wide enough to host the traffic lights itself the header has
            // to clear them. Steady state with the sidebar open adds nothing;
            // a sidebar sliding in shrinks the inset as it takes the lights
            // over, which is what keeps the title from passing under them.
            .pl(if self.sidebar_visible {
                px(14.0 + (TRAFFIC_LIGHT_CLEARANCE - self.sidebar_rendered_width).max(0.0))
            } else {
                px(0.0)
            })
            .pr(px(14.0))
            .when(!self.sidebar_visible, |element| {
                element
                    .child(
                        self.window_drag_region(
                            div()
                                .id("header-traffic-light-drag-region")
                                .w(px(TRAFFIC_LIGHT_CLEARANCE - 8.0))
                                .h_full()
                                .flex_none(),
                            cx,
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(self.render_sidebar_toggle(cx))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(2.0))
                                    .child(self.render_history_button(
                                        "navigate-back",
                                        "icons/arrow-left.svg",
                                        !self.session_navigation.back.is_empty(),
                                        true,
                                        cx,
                                    ))
                                    .child(self.render_history_button(
                                        "navigate-forward",
                                        "icons/arrow-right.svg",
                                        !self.session_navigation.forward.is_empty(),
                                        false,
                                        cx,
                                    )),
                            ),
                    )
            })
            .child(
                self.window_drag_region(
                    div()
                        .id("header-title-drag-region")
                        .h_full()
                        .min_w_0()
                        .flex_shrink(1.0)
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(sp(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(SharedString::from(title)),
                        )
                        .children(agent_preset_label.map(|label| {
                            div()
                                .h(px(22.0))
                                .max_w(px(180.0))
                                .px(px(6.0))
                                .rounded(px(6.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .bg(theme.overlay)
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_secondary)
                                .child(icon("icons/bot.svg", 10.5, theme.text_tertiary))
                                .child(div().min_w_0().truncate().child(SharedString::from(label)))
                        })),
                    cx,
                ),
            )
            .child(
                self.window_drag_region(
                    div().id("header-center-drag-region").h_full().flex_1(),
                    cx,
                ),
            )
            .child(self.render_background_work_summary(cx))
            .when(!self.right_panel_visible, |element| {
                element
                    .when(self.fps_counter_visible, |element| {
                        element.child(self.render_fps_counter(cx))
                    })
                    .child(self.render_right_panel_toggle(cx))
            })
            .children(right_window_controls)
    }

    // ── Empty states ───────────────────────────────────────────────────────

    pub(super) fn render_empty_state(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        if self.selected_project().is_none() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .px_8()
                .pb(px(46.0))
                .child(icon("icons/sparkle.svg", 24.0, theme.accent))
                .child(
                    div()
                        .mt(px(16.0))
                        .text_size(sp(20.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr_cow!("onboarding.open_project_to_begin")),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .max_w(px(380.0))
                        .text_center()
                        .text_size(sp(12.5))
                        .line_height(sp(19.0))
                        .text_color(theme.text_tertiary)
                        .child(tr_cow!("onboarding.description")),
                )
                .child(
                    div()
                        .mt(px(20.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.0))
                        .tab_index(0)
                        .tab_group()
                        .tab_stop(false)
                        .child(
                            div()
                                .id("onboarding-add-project")
                                .track_focus(&self.onboarding_add_project_focus)
                                .tab_index(0)
                                .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
                                .h(px(32.0))
                                .px(px(14.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .cursor_default()
                                .bg(theme.inverse)
                                .text_color(theme.on_inverse)
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .hover(|element| element.opacity(0.9))
                                .active(|element| element.opacity(0.8))
                                .child(tr_cow!("onboarding.open_project_folder"))
                                .on_click(cx.listener(|this, _, _, cx| this.add_project(cx)))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        this.add_project(cx);
                                        cx.stop_propagation();
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id("onboarding-projectless")
                                .track_focus(&self.onboarding_projectless_focus)
                                .tab_index(1)
                                .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
                                .h(px(30.0))
                                .px(px(12.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .cursor_default()
                                .text_color(theme.text_secondary)
                                .text_size(sp(12.5))
                                .hover(|element| element.bg(theme.overlay))
                                .active(|element| element.bg(theme.overlay_strong))
                                .child(icon("icons/x.svg", 11.0, theme.text_tertiary))
                                .child(tr_cow!("project.no_project"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.create_projectless_session(cx);
                                }))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        this.create_projectless_session(cx);
                                        cx.stop_propagation();
                                    }
                                })),
                        ),
                );
        }
        let projectless_selected = self.selected_project().is_some_and(Project::is_projectless);
        let project_name = self
            .selected_project()
            .map(|project| {
                if project.is_projectless() {
                    tr!("project.without_a_project")
                } else {
                    project.display_name()
                }
            })
            .unwrap_or_else(|| tr!("project.your_project"));
        let handle = self.project_picker_handle(ProjectPickerSite::EmptyState, cx);
        let project_selector = self.render_project_picker(
            ProjectNameSelector::new("empty-state-project", project_name)
                .selected(handle.is_open()),
            &handle,
            ProjectPickerSite::EmptyState,
            cx,
        );
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px_8()
            .pb(px(52.0))
            .child(icon("icons/sparkle.svg", 20.0, theme.accent))
            .child(
                div()
                    .mt(px(14.0))
                    .flex()
                    .items_baseline()
                    .text_size(sp(20.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .when(projectless_selected, |element| {
                        element.child(tr_cow!("onboarding.what_should_we_build"))
                    })
                    .when(!projectless_selected, |element| {
                        element
                            .child(tr_cow!("onboarding.what_should_we_build_in"))
                            .child(project_selector)
                            .child(tr_cow!("onboarding.question_mark"))
                    }),
            )
    }
}

fn localized_session_title(session: &AgentSession) -> String {
    let title = session.display_title();
    if title == AgentSession::DEFAULT_TITLE {
        tr!("session.new_task")
    } else {
        title.to_owned()
    }
}

fn sidebar_session_selected(
    selected_session: Option<Uuid>,
    pending_session: Option<Uuid>,
    session_id: Uuid,
) -> bool {
    pending_session.map_or(selected_session == Some(session_id), |pending| {
        pending == session_id
    })
}

#[cfg(test)]
mod tests {
    use gpui::{KeyUpEvent, Keystroke, Modifiers, PlatformInput, TestAppContext};

    use super::*;

    struct CollectionActivationHarness {
        id: Option<Uuid>,
        focus: FocusHandle,
        clicks: usize,
        renamed: Option<Uuid>,
        collapsed: bool,
    }

    impl Render for CollectionActivationHarness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            session_group_header(&Theme::dark())
                .id("collection")
                .w(px(120.0))
                .track_focus(&self.focus)
                .on_click(cx.listener(|this, event, _, cx| {
                    this.clicks += 1;
                    if let Some(id) = sidebar_collection_rename_target(this.id, event) {
                        this.renamed = Some(id);
                    } else {
                        this.collapsed = !this.collapsed;
                    }
                    cx.notify();
                }))
        }
    }

    #[gpui::test]
    fn collection_enter_renames_and_space_toggles_once_on_release(cx: &mut TestAppContext) {
        let id = Uuid::from_u128(1);
        let focus = cx.update(|cx| cx.focus_handle());
        let (view, cx) = cx.add_window_view(|_, _| CollectionActivationHarness {
            id: Some(id),
            focus: focus.clone(),
            clicks: 0,
            renamed: None,
            collapsed: false,
        });
        cx.update(|window, cx| {
            window.focus(&focus, cx);
            window.draw(cx).clear(cx);
        });

        for (clicks, key, renamed, collapsed) in [
            (1, "enter", Some(id), false),
            (2, "space", Some(id), true),
            (3, "space", Some(id), false),
        ] {
            let keystroke = Keystroke::parse(key).unwrap();
            cx.update(|window, cx| {
                window.dispatch_event(
                    PlatformInput::KeyDown(KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }),
                    cx,
                );
            });
            assert_eq!(view.read_with(cx, |view, _| view.clicks), clicks - 1);
            cx.update(|window, cx| {
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
            });
            assert_eq!(
                view.read_with(cx, |view, _| (view.clicks, view.renamed, view.collapsed)),
                (clicks, renamed, collapsed),
            );
        }

        view.update(cx, |view, _| view.renamed = None);
        cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
        assert_eq!(
            view.read_with(cx, |view, _| (view.clicks, view.renamed, view.collapsed)),
            (4, None, true),
        );
        assert_eq!(
            sidebar_collection_rename_target(None, &ClickEvent::default()),
            None
        );
    }

    #[test]
    fn keyboard_navigation_follows_visible_rows_and_stops_at_edges() {
        let first = SidebarGroup::Project(Uuid::from_u128(1));
        let collapsed = SidebarGroup::Project(Uuid::from_u128(2));
        let rows = [
            SidebarRow::Search,
            SidebarRow::Header(first),
            SidebarRow::Session(Uuid::from_u128(3)),
            SidebarRow::ShowMore(first),
            SidebarRow::GroupSpacer,
            SidebarRow::Header(collapsed),
            SidebarRow::GroupSpacer,
        ];
        for (current, key, expected) in [
            (0, "up", None),
            (0, "down", Some(1)),
            (1, "down", Some(2)),
            (2, "down", Some(3)),
            (3, "down", Some(5)),
            (5, "up", Some(3)),
            (5, "down", None),
            (3, "home", Some(0)),
            (1, "end", Some(5)),
            (2, "space", None),
        ] {
            assert_eq!(
                sidebar_navigation_target(&rows, rows[current], key),
                expected
            );
        }
        assert_eq!(sidebar_navigation_target(&[], rows[0], "down"), None);
        assert_eq!(
            sidebar_navigation_target(&rows, SidebarRow::Session(Uuid::nil()), "down"),
            None,
        );
    }

    #[test]
    fn groups_sessions_by_calendar_period() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let cases = [
            ((2026, 8, 12), SessionDateGroup::Today),
            ((2026, 8, 11), SessionDateGroup::Yesterday),
            ((2026, 8, 10), SessionDateGroup::ThisWeek),
            ((2026, 8, 1), SessionDateGroup::ThisMonth),
            ((2026, 1, 1), SessionDateGroup::ThisYear),
            ((2025, 12, 31), SessionDateGroup::More),
        ];

        for ((year, month, day), expected) in cases {
            let session_date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            assert_eq!(session_date_group_for_dates(session_date, today), expected);
        }
    }

    #[test]
    fn future_sessions_stay_in_today() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let tomorrow = NaiveDate::from_ymd_opt(2026, 8, 13).unwrap();
        assert_eq!(
            session_date_group_for_dates(tomorrow, today),
            SessionDateGroup::Today
        );
    }

    #[test]
    fn collapsed_sidebar_group_keeps_only_its_header_and_spacer() {
        let sessions = [Uuid::from_u128(1), Uuid::from_u128(2)];
        let group = SidebarGroup::Updated(SessionDateGroup::Today);
        let mut expanded = Vec::new();
        append_sidebar_group_rows(&mut expanded, group, &sessions, false, false);
        assert_eq!(
            expanded,
            vec![
                SidebarRow::Header(group),
                SidebarRow::Session(sessions[0]),
                SidebarRow::Session(sessions[1]),
                SidebarRow::GroupSpacer,
            ]
        );

        let mut collapsed = Vec::new();
        append_sidebar_group_rows(&mut collapsed, group, &sessions, true, false);
        assert_eq!(
            collapsed,
            vec![SidebarRow::Header(group), SidebarRow::GroupSpacer,]
        );
    }

    #[test]
    fn working_headers_include_hidden_tasks_and_clear_when_they_settle() {
        let today = Local::now().date_naive();
        let root = Path::new("/tmp/.michelle/projects");
        let project = Uuid::from_u128(1);
        let projectless = Uuid::from_u128(2);
        let collection = Uuid::from_u128(3);
        let mut state = PersistedState::empty();
        state.sidebar_grouping = SidebarGrouping::Project;
        state.sidebar_collapsed_projects.insert(project);
        state.sidebar_projects_collapsed = true;
        state.projects.push(Project {
            id: projectless,
            name: "Task".into(),
            path: root.join("task"),
            created_at: 0,
        });
        state.sidebar_project_groups.push(SidebarProjectGroup {
            id: collection,
            name: "Work".into(),
            projects: vec![project],
            collapsed: true,
        });
        for (project, status) in [
            (project, SessionStatus::Working),
            (projectless, SessionStatus::Connecting),
            (Uuid::from_u128(4), SessionStatus::Waiting),
        ] {
            let mut session = AgentSession::new(project, ProviderKind::Codex);
            session.status = status;
            session.detail_loaded = false;
            state.sessions.push(session);
        }
        let mut draft = AgentSession::new(Uuid::from_u128(5), ProviderKind::Codex);
        draft.status = SessionStatus::Connecting;
        state.sessions.push(draft);
        assert_eq!(
            sidebar_working_headers(&state, today, Some(root)),
            HashSet::from([
                SidebarRow::Header(SidebarGroup::Project(project)),
                SidebarRow::Header(SidebarGroup::Projectless),
                SidebarRow::Collection(Some(collection)),
                SidebarRow::Collection(None),
            ])
        );
        state.sidebar_grouping = SidebarGrouping::Updated;
        assert_eq!(
            sidebar_working_headers(&state, today, Some(root)),
            HashSet::from([SidebarRow::Header(SidebarGroup::Updated(
                SessionDateGroup::Today
            ))])
        );
        for session in &mut state.sessions {
            session.status = SessionStatus::Idle;
        }
        assert!(sidebar_working_headers(&state, today, Some(root)).is_empty());
        state.sidebar_grouping = SidebarGrouping::Project;
        assert!(sidebar_working_headers(&state, today, Some(root)).is_empty());
    }

    #[test]
    fn hidden_project_sessions_keep_a_keyboard_reveal_row() {
        let group = SidebarGroup::Project(Uuid::from_u128(1));
        let mut expanded = Vec::new();
        append_sidebar_group_rows(&mut expanded, group, &[], false, true);
        assert_eq!(
            expanded,
            vec![
                SidebarRow::Header(group),
                SidebarRow::ShowMore(group),
                SidebarRow::GroupSpacer,
            ]
        );

        let mut collapsed = Vec::new();
        append_sidebar_group_rows(&mut collapsed, group, &[], true, true);
        assert_eq!(
            collapsed,
            vec![SidebarRow::Header(group), SidebarRow::GroupSpacer]
        );
    }

    #[test]
    fn project_sessions_reveal_older_history_in_thirty_item_batches() {
        let sessions = (1..=36).map(Uuid::from_u128).collect::<Vec<_>>();
        let recent_cutoff = 100;
        let timestamps = sessions
            .iter()
            .enumerate()
            .map(|(index, session_id)| {
                (
                    *session_id,
                    if index == 0 {
                        recent_cutoff
                    } else {
                        recent_cutoff - 1
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let (initial, show_more) =
            visible_project_sessions(&sessions, &timestamps, recent_cutoff, 0);
        assert_eq!(initial, vec![sessions[0]]);
        assert!(show_more);

        let (first_batch, show_more) = visible_project_sessions(
            &sessions,
            &timestamps,
            recent_cutoff,
            SIDEBAR_PROJECT_REVEAL_BATCH,
        );
        assert_eq!(first_batch, sessions[..31]);
        assert!(show_more);

        let (all_sessions, show_more) = visible_project_sessions(
            &sessions,
            &timestamps,
            recent_cutoff,
            SIDEBAR_PROJECT_REVEAL_BATCH * 2,
        );
        assert_eq!(all_sessions, sessions);
        assert!(!show_more);
    }

    #[test]
    fn sidebar_recency_uses_last_reply_with_creation_fallback() {
        let project_id = Uuid::new_v4();
        let mut renamed_old_session = AgentSession::new(project_id, ProviderKind::Codex);
        renamed_old_session.created_at = 10;
        renamed_old_session.last_reply_at = Some(20);
        renamed_old_session.updated_at = 1_000;

        let mut newer_unanswered_session = AgentSession::new(project_id, ProviderKind::Codex);
        newer_unanswered_session.created_at = 30;
        newer_unanswered_session.last_reply_at = None;
        newer_unanswered_session.updated_at = 30;

        assert_eq!(sidebar_session_timestamp(&renamed_old_session), 20);
        assert_eq!(sidebar_session_timestamp(&newer_unanswered_session), 30);

        let mut sessions = vec![&renamed_old_session, &newer_unanswered_session];
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Newest, &[]);
        assert_eq!(sessions[0].id, newer_unanswered_session.id);

        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Oldest, &[]);
        assert_eq!(sessions[0].id, renamed_old_session.id);
    }

    #[test]
    fn manual_sidebar_order_stays_stable_and_moves_only_peers() {
        let project = Uuid::from_u128(10);
        let other_project = Uuid::from_u128(20);
        let mut first = AgentSession::new(project, ProviderKind::Codex);
        first.id = Uuid::from_u128(1);
        first.created_at = 1;
        let mut second = AgentSession::new(project, ProviderKind::Codex);
        second.id = Uuid::from_u128(2);
        second.created_at = 2;
        let mut new = AgentSession::new(other_project, ProviderKind::Codex);
        new.id = Uuid::from_u128(3);
        new.created_at = 3;
        let mut newer = AgentSession::new(project, ProviderKind::Codex);
        newer.id = Uuid::from_u128(4);
        newer.created_at = 4;
        let mut order = vec![Uuid::from_u128(99), second.id, first.id];
        let ids = |sessions: &[&AgentSession]| {
            sessions
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>()
        };
        let expected = vec![newer.id, new.id, second.id, first.id];
        let mut sessions = vec![&newer, &first, &new, &second];
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Manual, &order);
        assert_eq!(ids(&sessions), expected);
        assert_eq!(
            project_sidebar_groups(&sessions, &HashSet::new()),
            vec![
                (
                    SidebarGroup::Project(project),
                    vec![newer.id, second.id, first.id]
                ),
                (SidebarGroup::Project(other_project), vec![new.id]),
            ]
        );
        drop(sessions);
        first.last_reply_at = Some(10_000);
        second.last_reply_at = Some(20_000);
        let mut sessions = vec![&new, &second, &newer, &first];
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Manual, &order);
        assert_eq!(ids(&sessions), expected);
        assert!(move_sidebar_item(&mut order, first.id, second.id, false));
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Manual, &order);
        assert_eq!(ids(&sessions), vec![newer.id, new.id, first.id, second.id]);
        assert!(move_sidebar_item(&mut order, first.id, second.id, true));
        assert_eq!(order, vec![Uuid::from_u128(99), second.id, first.id]);
        let unchanged = order.clone();
        assert!(!move_sidebar_item(&mut order, first.id, first.id, false));
        assert!(!move_sidebar_item(&mut order, Uuid::nil(), first.id, false));
        assert!(!move_sidebar_item(&mut order, first.id, Uuid::nil(), false));
        assert_eq!(order, unchanged);
        // Collection order may differ from the stored global project order.
        // Insert on the requested side even when moving the other direction.
        let mut projects = vec![project, other_project, Uuid::from_u128(30)];
        assert!(move_sidebar_item(
            &mut projects,
            other_project,
            project,
            false
        ));
        assert_eq!(projects, vec![other_project, project, Uuid::from_u128(30)]);
        assert!(move_sidebar_item(
            &mut projects,
            other_project,
            project,
            true
        ));
        assert_eq!(projects, vec![project, other_project, Uuid::from_u128(30)]);

        let first_header = SidebarRow::Header(SidebarGroup::Project(project));
        let second_header = SidebarRow::Header(SidebarGroup::Project(other_project));
        let rows = vec![
            SidebarRow::Search,
            first_header,
            SidebarRow::Session(first.id),
            SidebarRow::Session(second.id),
            SidebarRow::GroupSpacer,
            second_header,
            SidebarRow::Session(new.id),
            SidebarRow::GroupSpacer,
        ];
        assert_eq!(
            sidebar_reorder_siblings(&rows, SidebarRow::Session(first.id)),
            vec![
                SidebarRow::Session(first.id),
                SidebarRow::Session(second.id)
            ]
        );
        assert_eq!(
            sidebar_reorder_siblings(&rows, first_header),
            vec![first_header, second_header]
        );
    }

    #[test]
    fn project_grouping_preserves_global_group_and_session_order() {
        let first_project = Uuid::from_u128(1);
        let second_project = Uuid::from_u128(2);
        let first = AgentSession::new(first_project, ProviderKind::Codex);
        let second = AgentSession::new(second_project, ProviderKind::Codex);
        let third = AgentSession::new(first_project, ProviderKind::Codex);

        let groups = project_sidebar_groups(&[&second, &first, &third], &HashSet::new());

        assert_eq!(
            groups,
            vec![
                (SidebarGroup::Project(second_project), vec![second.id]),
                (
                    SidebarGroup::Project(first_project),
                    vec![first.id, third.id]
                ),
            ]
        );
    }

    #[test]
    fn project_collections_keep_projects_last_and_collapse_independently() {
        let project = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let a = Uuid::from_u128(10);
        let b = Uuid::from_u128(20);
        let section = |id| {
            vec![
                SidebarRow::Header(SidebarGroup::Project(id)),
                SidebarRow::GroupSpacer,
            ]
        };
        let mut groups = vec![
            SidebarProjectGroup {
                id: a,
                name: "Work".into(),
                projects: vec![project],
                collapsed: false,
            },
            SidebarProjectGroup {
                id: b,
                name: "Empty".into(),
                projects: Vec::new(),
                collapsed: false,
            },
        ];
        let build = |groups: &[SidebarProjectGroup], collapsed| {
            let mut rows = vec![SidebarRow::Search];
            append_sidebar_project_collections(
                &mut rows,
                vec![section(other), section(project)],
                groups,
                collapsed,
            );
            rows
        };
        let rows = build(&groups, false);
        assert_eq!(
            build(&[], false),
            vec![
                SidebarRow::Search,
                SidebarRow::Collection(None),
                SidebarRow::Header(SidebarGroup::Project(other)),
                SidebarRow::Header(SidebarGroup::Project(project)),
                SidebarRow::GroupSpacer,
            ]
        );
        assert_eq!(
            rows,
            vec![
                SidebarRow::Search,
                SidebarRow::Collection(Some(a)),
                SidebarRow::Header(SidebarGroup::Project(project)),
                SidebarRow::GroupSpacer,
                SidebarRow::Collection(Some(b)),
                SidebarRow::GroupSpacer,
                SidebarRow::Collection(None),
                SidebarRow::Header(SidebarGroup::Project(other)),
                SidebarRow::GroupSpacer
            ]
        );
        groups[0].collapsed = true;
        let rows = build(&groups, false);
        assert!(!rows.contains(&SidebarRow::Header(SidebarGroup::Project(project))));
        assert!(rows.contains(&SidebarRow::Header(SidebarGroup::Project(other))));
        assert!(rows.contains(&SidebarRow::Collection(Some(a))));
        assert!(!build(&groups, true).contains(&SidebarRow::Header(SidebarGroup::Project(other))));
        groups.swap(0, 1);
        assert_eq!(
            build(&groups, false).get(1),
            Some(&SidebarRow::Collection(Some(b)))
        );
    }

    #[test]
    fn sidebar_drag_preview_previews_whole_sections_without_changing_the_original_order() {
        let source = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let third = Uuid::from_u128(3);
        let session = SidebarRow::Session(Uuid::from_u128(100));
        let other_session = SidebarRow::Session(Uuid::from_u128(200));
        let collection = SidebarRow::Collection(Some(Uuid::from_u128(10)));
        let header = |id| SidebarRow::Header(SidebarGroup::Project(id));
        let original = Rc::new(vec![
            SidebarRow::Search,
            collection,
            header(source),
            session,
            header(other),
            other_session,
            header(third),
            SidebarRow::GroupSpacer,
            SidebarRow::Collection(None),
            SidebarRow::Header(SidebarGroup::Projectless),
            SidebarRow::GroupSpacer,
        ]);
        let preview = |rows: Rc<Vec<SidebarRow>>, ordering| SidebarDragPreview {
            row: header(source),
            siblings: Vec::new(),
            offsets: sidebar_row_offsets(&rows),
            rows,
            ordering,
            timestamps: HashMap::from([(source, 15), (other, 20), (third, 10)]),
            destination: None,
            bounds: Bounds::default(),
            cursor_offset: gpui::Point::default(),
            collapsed: false,
            working: false,
        };
        let mut manual = preview(original.clone(), SidebarOrdering::Manual);
        let other_y = manual.offsets[4];
        assert!(!manual.move_project(other_y + px(15.0)));
        assert!(manual.move_project(other_y + px(17.0)));
        assert_eq!(
            &manual.rows[2..7],
            &[
                header(other),
                other_session,
                header(source),
                session,
                header(third)
            ]
        );
        assert_eq!(
            manual.destination,
            Some((Some(Uuid::from_u128(10)), Some(third)))
        );
        // The placeholder absorbs subsequent movement, preventing oscillation.
        assert!(!manual.move_project(other_y + px(17.0)));
        assert!(manual.move_project(manual.offsets[2] + px(15.0)));
        assert_eq!(manual.rows, original);
        assert!(!manual.move_project(px(-10.0)));
        assert!(!manual.move_project(*manual.offsets.last().unwrap() + px(10.0)));
        assert!(!manual.move_project(manual.offsets[9] + px(15.0)));

        for ordering in [SidebarOrdering::Newest, SidebarOrdering::Oldest] {
            let (first, second) = if ordering == SidebarOrdering::Newest {
                (other, third)
            } else {
                (third, other)
            };
            let target = SidebarRow::Collection(Some(Uuid::from_u128(20)));
            let rows = Rc::new(vec![
                SidebarRow::Search,
                collection,
                header(source),
                session,
                SidebarRow::GroupSpacer,
                target,
                header(first),
                other_session,
                header(second),
                SidebarRow::GroupSpacer,
                SidebarRow::Collection(None),
                SidebarRow::Header(SidebarGroup::Projectless),
                SidebarRow::GroupSpacer,
            ]);
            let mut automatic = preview(rows.clone(), ordering);
            assert!(!automatic.move_project(automatic.offsets[1] + px(15.0)));
            assert!(automatic.move_project(automatic.offsets[5] + px(15.0)));
            assert_eq!(
                &automatic.rows[3..9],
                &[
                    target,
                    header(first),
                    other_session,
                    header(source),
                    session,
                    header(second)
                ]
            );
            assert_eq!(
                automatic.destination,
                Some((Some(Uuid::from_u128(20)), Some(second)))
            );
            assert!(automatic.move_project(automatic.offsets[10] + px(15.0)));
            assert_eq!(
                &automatic.rows[8..13],
                &[
                    SidebarRow::Collection(None),
                    header(source),
                    session,
                    SidebarRow::Header(SidebarGroup::Projectless),
                    SidebarRow::GroupSpacer
                ]
            );
            // Returning to the original collection is a preview too; cancellation
            // simply releases the snapshot and leaves this original Rc untouched.
            assert!(automatic.move_project(automatic.offsets[1] + px(15.0)));
            assert_eq!(automatic.rows, rows);
        }
        assert_eq!(
            &original[2..7],
            &[
                header(source),
                session,
                header(other),
                other_session,
                header(third)
            ]
        );
    }

    #[test]
    fn sidebar_drag_previews_collections_and_sessions_within_their_boundaries() {
        let a = SidebarRow::Collection(Some(Uuid::from_u128(10)));
        let b = SidebarRow::Collection(Some(Uuid::from_u128(20)));
        let c = SidebarRow::Collection(Some(Uuid::from_u128(30)));
        let project = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(1)));
        let other_project = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(2)));
        let first = SidebarRow::Session(Uuid::from_u128(100));
        let second = SidebarRow::Session(Uuid::from_u128(200));
        let third = SidebarRow::Session(Uuid::from_u128(300));
        let other = SidebarRow::Session(Uuid::from_u128(400));
        let preview = |row, rows: Rc<Vec<SidebarRow>>, ordering| SidebarDragPreview {
            row,
            siblings: sidebar_reorder_siblings(&rows, row),
            offsets: sidebar_row_offsets(&rows),
            rows,
            ordering,
            timestamps: HashMap::new(),
            destination: None,
            bounds: Bounds::default(),
            cursor_offset: gpui::Point::default(),
            collapsed: false,
            working: false,
        };
        let original = Rc::new(vec![
            SidebarRow::Search,
            a,
            project,
            first,
            SidebarRow::GroupSpacer,
            b,
            SidebarRow::GroupSpacer,
            c,
            other_project,
            other,
            SidebarRow::GroupSpacer,
            SidebarRow::Collection(None),
            SidebarRow::Header(SidebarGroup::Projectless),
            SidebarRow::GroupSpacer,
        ]);
        let mut groups = preview(a, original.clone(), SidebarOrdering::Newest);
        let b_y = groups.offsets[5];
        assert!(!groups.move_row(b_y + px(15.0)));
        assert!(groups.move_row(b_y + px(17.0)));
        assert_eq!(
            &groups.rows[1..7],
            &[
                b,
                SidebarRow::GroupSpacer,
                a,
                project,
                first,
                SidebarRow::GroupSpacer
            ]
        );
        assert!(!groups.move_row(b_y + px(17.0)));
        assert!(groups.move_row(groups.offsets[7] + px(17.0)));
        assert_eq!(
            &groups.rows[3..11],
            &[
                c,
                other_project,
                other,
                SidebarRow::GroupSpacer,
                a,
                project,
                first,
                SidebarRow::GroupSpacer
            ]
        );
        assert!(!groups.move_row(groups.offsets[11] + px(17.0)));
        assert!(groups.move_row(groups.offsets[1] + px(15.0)));
        assert_eq!(groups.rows, original);
        // A collapsed/empty collection still moves together with its spacer.
        let mut collapsed = preview(b, original.clone(), SidebarOrdering::Oldest);
        assert!(collapsed.move_row(collapsed.offsets[7] + px(17.0)));
        assert_eq!(
            &collapsed.rows[5..11],
            &[
                c,
                other_project,
                other,
                SidebarRow::GroupSpacer,
                b,
                SidebarRow::GroupSpacer
            ]
        );
        assert!(
            !preview(
                SidebarRow::Collection(None),
                original.clone(),
                SidebarOrdering::Manual
            )
            .move_row(px(90.0))
        );
        assert_eq!(groups.rows, original);

        for header in [
            project,
            SidebarRow::Header(SidebarGroup::Updated(SessionDateGroup::Today)),
        ] {
            let original = Rc::new(vec![
                SidebarRow::Search,
                header,
                first,
                second,
                third,
                SidebarRow::ShowMore(match header {
                    SidebarRow::Header(group) => group,
                    _ => unreachable!(),
                }),
                SidebarRow::GroupSpacer,
                other_project,
                other,
            ]);
            let mut chats = preview(first, original.clone(), SidebarOrdering::Manual);
            let second_y = chats.offsets[3];
            assert!(!chats.move_row(second_y + px(25.0)));
            assert!(chats.move_row(second_y + px(27.0)));
            assert_eq!(&chats.rows[2..5], &[second, first, third]);
            assert!(!chats.move_row(second_y + px(27.0)));
            assert!(chats.move_row(chats.offsets[4] + px(27.0)));
            assert_eq!(&chats.rows[2..5], &[second, third, first]);
            assert!(!chats.move_row(chats.offsets[8] + px(27.0)));
            assert!(!chats.move_row(chats.offsets[5] + px(15.0)));
            assert!(!chats.move_row(chats.offsets[7] + px(15.0)));
            assert!(chats.move_row(chats.offsets[2] + px(25.0)));
            assert_eq!(chats.rows, original);
            assert!(
                !preview(first, original, SidebarOrdering::Newest).move_row(second_y + px(27.0))
            );
        }
    }

    #[test]
    fn sidebar_reorder_animation_retargets_from_visible_positions_and_finishes() {
        let a = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(1)));
        let b = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(2)));
        let chat_a = SidebarRow::Session(Uuid::from_u128(100));
        let chat_b = SidebarRow::Session(Uuid::from_u128(200));
        let old = [
            SidebarRow::Search,
            a,
            chat_a,
            SidebarRow::GroupSpacer,
            b,
            chat_b,
            SidebarRow::GroupSpacer,
        ];
        let new = [
            SidebarRow::Search,
            b,
            chat_b,
            SidebarRow::GroupSpacer,
            a,
            chat_a,
            SidebarRow::GroupSpacer,
        ];
        let now = Instant::now();
        let animation = SidebarReorderAnimation::between(&old, &new, None, None, now).unwrap();
        let section_height = sidebar_row_height(a)
            + sidebar_row_height(chat_a)
            + sidebar_row_height(SidebarRow::GroupSpacer);
        assert_eq!(animation.offsets[&a], -section_height);
        assert_eq!(animation.offsets[&b], section_height);
        assert_eq!(animation.offsets[&chat_a], animation.offsets[&a]);
        assert_eq!(animation.offsets[&chat_b], animation.offsets[&b]);
        assert!(!animation.offsets.contains_key(&SidebarRow::GroupSpacer));
        assert!(!animation.offsets.contains_key(&SidebarRow::Search));
        assert_eq!(animation.remaining_at(now), 1.0);
        let interrupted_at = now + SIDEBAR_REORDER_DURATION / 2;
        let remaining = animation.remaining_at(interrupted_at);
        assert!(remaining > 0.0 && remaining < 0.5);
        let reversed =
            SidebarReorderAnimation::between(&new, &old, Some(&animation), None, interrupted_at)
                .unwrap();
        let old_y = sidebar_row_offsets(&old);
        let new_y = sidebar_row_offsets(&new);
        for (index, row) in old
            .iter()
            .enumerate()
            .filter(|(_, row)| animation.offsets.contains_key(*row))
        {
            let new_index = new.iter().position(|candidate| candidate == row).unwrap();
            let visible_before = new_y[new_index] + animation.offsets[row] * remaining;
            let visible_after = old_y[index] + reversed.offsets[row];
            assert!((visible_before - visible_after).abs() < px(0.001));
        }
        assert_eq!(animation.remaining_at(now + SIDEBAR_REORDER_DURATION), 0.0);
        assert_eq!(
            reversed.remaining_at(interrupted_at + SIDEBAR_REORDER_DURATION),
            0.0
        );
        assert!(SidebarReorderAnimation::between(&old, &old, None, None, now).is_none());
        // The lifted row lands directly in its slot when the mouse is released.
        let lifted = SidebarReorderAnimation::between(&old, &new, None, Some(a), now).unwrap();
        assert!(!lifted.offsets.contains_key(&a));
        let cancelled =
            SidebarReorderAnimation::between(&new, &old, Some(&lifted), None, interrupted_at)
                .unwrap();
        assert!(!cancelled.offsets.contains_key(&a));
    }

    #[test]
    fn projectless_sessions_share_one_trailing_group() {
        let ordinary_project = Uuid::from_u128(1);
        let first_projectless_project = Uuid::from_u128(2);
        let second_projectless_project = Uuid::from_u128(3);
        let first_projectless = AgentSession::new(first_projectless_project, ProviderKind::Codex);
        let ordinary = AgentSession::new(ordinary_project, ProviderKind::Codex);
        let second_projectless = AgentSession::new(second_projectless_project, ProviderKind::Codex);

        let groups = project_sidebar_groups(
            &[&first_projectless, &ordinary, &second_projectless],
            &HashSet::from([first_projectless_project, second_projectless_project]),
        );

        assert_eq!(
            groups,
            vec![
                (SidebarGroup::Project(ordinary_project), vec![ordinary.id]),
                (
                    SidebarGroup::Projectless,
                    vec![first_projectless.id, second_projectless.id]
                ),
            ]
        );
    }

    #[test]
    fn projectless_sidebar_projects_are_paths_under_the_workspace_root() {
        let root = Path::new("/tmp/.michelle/projects");
        let projectless = Project {
            id: Uuid::from_u128(1),
            name: "Task".to_owned(),
            path: root.join("2026-08-23/task"),
            created_at: 0,
        };
        let ordinary = Project {
            id: Uuid::from_u128(2),
            name: "Ordinary".to_owned(),
            path: PathBuf::from("/tmp/dev/ordinary"),
            created_at: 0,
        };

        assert!(sidebar_project_is_projectless(&projectless, Some(root)));
        assert!(!sidebar_project_is_projectless(&ordinary, Some(root)));
        assert!(!sidebar_project_is_projectless(&projectless, None));
    }

    #[test]
    fn persisted_worktree_branches_supply_sidebar_labels() {
        let local = SessionWorkspace::Local;
        let planned = SessionWorkspace::NewWorktree {
            base_branch: Some("develop".to_owned()),
        };
        let worktree = SessionWorkspace::Worktree {
            path: PathBuf::from("/tmp/worktree"),
            branch: "feature/sidebar".to_owned(),
        };

        assert_eq!(persisted_sidebar_branch_label(&local), None);
        assert_eq!(persisted_sidebar_branch_label(&planned), Some("develop"));
        assert_eq!(
            persisted_sidebar_branch_label(&worktree),
            Some("feature/sidebar")
        );
    }

    #[test]
    fn pending_session_replaces_sidebar_selection_immediately() {
        let current = Uuid::from_u128(1);
        let pending = Uuid::from_u128(2);

        assert!(!sidebar_session_selected(
            Some(current),
            Some(pending),
            current
        ));
        assert!(sidebar_session_selected(
            Some(current),
            Some(pending),
            pending
        ));
        assert!(sidebar_session_selected(Some(current), None, current));
    }

    #[test]
    fn selected_session_uses_nearest_bottom_edge_for_an_unmeasured_lower_row() {
        let target = Uuid::from_u128(31);
        let group = SidebarGroup::Updated(SessionDateGroup::Today);
        let mut rows = vec![SidebarRow::Search, SidebarRow::Header(group)];
        rows.extend((1..=40).map(|id| SidebarRow::Session(Uuid::from_u128(id))));
        rows.push(SidebarRow::GroupSpacer);

        let index = sidebar_session_row_index(&rows, target).unwrap();
        let offset = sidebar_bottom_aligned_offset(&rows, index, px(400.0));

        assert_eq!(index, 32);
        assert_eq!(offset.item_ix, 25);
        assert_eq!(offset.offset_in_item, px(16.0));
        let visible_height = rows[offset.item_ix..=index]
            .iter()
            .copied()
            .map(sidebar_row_height)
            .fold(Pixels::ZERO, |height, row| height + row)
            - offset.offset_in_item;
        assert_eq!(visible_height, px(400.0));
        assert_eq!(sidebar_session_row_index(&rows, Uuid::from_u128(41)), None);
    }
}
