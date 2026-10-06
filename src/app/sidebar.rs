use gpui::{ClickEvent, KeyBinding, KeyboardButton, actions, transparent_black};
use michelle_client::persistence::SidebarProjectGroup;

use crate::ui::{
    IconSize, StyledTypography, TextStyle,
    sidebar::{
        SIDEBAR_ITEM_HEIGHT, SIDEBAR_SECTION_HEADER_HEIGHT, SidebarDisclosure, SidebarItemProps,
        sidebar_item, sidebar_section_header,
    },
};

use super::*;
use model::*;
pub(super) mod model;

pub(super) struct SidebarUi {
    pub(in crate::app) onboarding_add_project_focus: FocusHandle,
    pub(in crate::app) onboarding_projectless_focus: FocusHandle,

    pub(in crate::app) session_rename: Option<Uuid>,
    pub(in crate::app) project_group_rename: Option<Uuid>,
    pub(in crate::app) session_rename_input: Entity<TextInput>,
    pub(in crate::app) collapsed_groups: HashSet<SidebarGroup>,
    pub(in crate::app) project_reveal_counts: HashMap<SidebarGroup, usize>,
    pub(in crate::app) group_header_focuses: RefCell<HashMap<SidebarGroup, FocusHandle>>,
    pub(in crate::app) show_more_focuses: RefCell<HashMap<SidebarGroup, FocusHandle>>,
    pub(in crate::app) list_state: ListState,
    pub(in crate::app) scrollbar: Rc<ScrollbarState>,
    pub(in crate::app) row_cache: RefCell<Vec<SidebarRow>>,
    pub(in crate::app) drag_preview: Rc<RefCell<Option<sidebar::SidebarDragPreview>>>,
    pub(in crate::app) reorder_animation: RefCell<Option<sidebar::SidebarReorderAnimation>>,
}

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

/// Stable identity for a collapsible sidebar section.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SidebarGroup {
    Project(Uuid),
    Projectless,
}

impl SidebarGroup {
    fn element_key(self) -> SharedString {
        match self {
            Self::Project(project_id) => format!("project-{project_id}").into(),
            Self::Projectless => "projectless".into(),
        }
    }

    fn mix_fingerprint(self, fingerprint: u64) -> u64 {
        match self {
            Self::Project(project_id) => mix_uuid(mix(fingerprint, 0x100), project_id),
            Self::Projectless => mix(fingerprint, 0x200),
        }
    }
}

fn sidebar_ordering_label(ordering: SidebarOrdering) -> String {
    match ordering {
        SidebarOrdering::Newest => tr!("sidebar.ordering_newest"),
        SidebarOrdering::Oldest => tr!("sidebar.ordering_oldest"),
        SidebarOrdering::Manual => tr!("sidebar.ordering_manual"),
    }
}

// Paint the focus ring without changing the row's layout or measured height.
fn focus_ring(color: Hsla) -> gpui::BoxShadow {
    gpui::BoxShadow::new(px(0.0), px(0.0), color)
        .spread_radius(px(1.0))
        .inset()
}

fn sidebar_collection_rename_target(id: Option<Uuid>, event: &ClickEvent) -> Option<Uuid> {
    match event {
        ClickEvent::Keyboard(event) if event.button == KeyboardButton::Enter => id,
        _ => None,
    }
}

/// Height of a session item plus the separation reserved beneath it in the
/// virtualized sidebar list. Keep the gap inside the list row so measured and
/// estimated heights stay identical for off-screen sessions.
const SIDEBAR_SESSION_ROW_GAP: f32 = 1.0;
const SIDEBAR_SESSION_ROW_HEIGHT: f32 = SIDEBAR_ITEM_HEIGHT + SIDEBAR_SESSION_ROW_GAP;
const SIDEBAR_GROUP_HEADER_HEIGHT: f32 = SIDEBAR_ITEM_HEIGHT;
const SIDEBAR_COLLECTION_HEADER_HEIGHT: f32 = SIDEBAR_SECTION_HEADER_HEIGHT;
const SIDEBAR_GROUP_HEADER_BOTTOM_GAP: f32 = 1.0;
const SIDEBAR_SHOW_MORE_ROW_HEIGHT: f32 = 30.0;
const SIDEBAR_TOP_PADDING: f32 = 6.0;
const SIDEBAR_GROUP_SPACER_HEIGHT: f32 = 16.0;
const SIDEBAR_GROUP_CHILD_PADDING: f32 = 23.0;
const SIDEBAR_PROJECT_INITIAL_LIMIT: usize = 5;
const SIDEBAR_PROJECT_REVEAL_BATCH: usize = 10;

/// Compact "how long ago" for task pickers: "just now", then one coarse unit —
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
    /// Leading space that scrolls away with the history.
    TopSpacer,
    /// User collection, or the built-in trailing Projects collection.
    Collection(Option<Uuid>),
    /// Collapsible project or projectless group header.
    Header(SidebarGroup),
    /// A started session.
    Session(Uuid),
    /// Reveals the next batch of older sessions in a project section.
    ShowMore(SidebarGroup),
    /// Spacing between sidebar groups.
    GroupSpacer,
}

#[derive(Clone)]
struct SidebarDrag {
    row: SidebarRow,
    move_to_project: bool,
    label: SharedString,
    michelle: WeakEntity<Michelle>,
    preview: Rc<RefCell<Option<SidebarDragPreview>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SidebarSessionDrop {
    group: SidebarGroup,
    relative_to: Option<(Uuid, bool)>,
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
    session_destination: Option<SidebarSessionDrop>,
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
            .filter(|(row, _)| !matches!(row, SidebarRow::TopSpacer | SidebarRow::GroupSpacer))
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
    fn move_session(&mut self, y: Pixels) -> bool {
        let SidebarRow::Session(source) = self.row else {
            return false;
        };
        let hovered = self
            .offsets
            .partition_point(|offset| *offset <= y)
            .saturating_sub(1);
        let hovered_row = (y >= Pixels::ZERO)
            .then(|| self.rows.get(hovered))
            .flatten();
        if hovered_row == Some(&self.row) || hovered_row == Some(&SidebarRow::TopSpacer) {
            return false;
        }
        let group = match hovered_row {
            Some(SidebarRow::Header(group) | SidebarRow::ShowMore(group)) => *group,
            Some(SidebarRow::Session(_)) => self.rows[..hovered]
                .iter()
                .rev()
                .take_while(|row| {
                    !matches!(row, SidebarRow::Collection(_) | SidebarRow::GroupSpacer)
                })
                .find_map(|row| match row {
                    SidebarRow::Header(group) => Some(*group),
                    _ => None,
                })
                .unwrap_or(SidebarGroup::Projectless),
            _ => SidebarGroup::Projectless,
        };
        let relative_to = if let Some(SidebarRow::Session(target)) = hovered_row {
            Some((
                *target,
                y >= self.offsets[hovered] + sidebar_row_height(*hovered_row.unwrap()) / 2.0,
            ))
        } else {
            let sessions = self
                .rows
                .iter()
                .skip_while(|row| **row != SidebarRow::Header(group))
                .skip(1)
                .take_while(|row| {
                    !matches!(
                        row,
                        SidebarRow::Header(_) | SidebarRow::Collection(_) | SidebarRow::GroupSpacer
                    )
                })
                .filter_map(|row| match row {
                    SidebarRow::Session(id) if *id != source => Some(*id),
                    _ => None,
                });
            if matches!(hovered_row, Some(SidebarRow::Header(_))) {
                sessions.into_iter().next().map(|id| (id, false))
            } else {
                sessions.last().map(|id| (id, true))
            }
        };
        let destination = SidebarSessionDrop { group, relative_to };
        if self.session_destination == Some(destination) {
            return false;
        }
        let mut rows = self.rows.as_ref().clone();
        rows.retain(|row| *row != self.row);
        let insertion = relative_to
            .and_then(|(target, after)| {
                rows.iter()
                    .position(|row| *row == SidebarRow::Session(target))
                    .map(|index| index + usize::from(after))
            })
            .or_else(|| {
                rows.iter()
                    .position(|row| *row == SidebarRow::Header(group))
                    .map(|index| index + 1)
            })
            .unwrap_or_else(|| {
                rows.push(SidebarRow::Header(group));
                rows.push(SidebarRow::GroupSpacer);
                rows.len() - 1
            });
        rows.insert(insertion, self.row);
        let changed = rows != *self.rows || self.session_destination != Some(destination);
        self.offsets = sidebar_row_offsets(&rows);
        self.rows = Rc::new(rows);
        self.session_destination = Some(destination);
        changed
    }

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
            _ => sidebar_item(
                "sidebar-project-drag-preview",
                "folder",
                self.label.clone(),
                SidebarItemProps {
                    disclosure: if preview.collapsed {
                        SidebarDisclosure::Collapsed
                    } else {
                        SidebarDisclosure::Expanded
                    },
                    ..Default::default()
                },
                &theme.ui_colors(),
            )
            .when(preview.working, |header| {
                header.child(motion::spinner_slow(14.0, theme.text))
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
    let focusable = |index: &usize| {
        !matches!(
            rows[*index],
            SidebarRow::TopSpacer | SidebarRow::GroupSpacer
        )
    };
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
        SidebarRow::TopSpacer => SIDEBAR_TOP_PADDING,
        SidebarRow::Header(SidebarGroup::Projectless) => SIDEBAR_SECTION_HEADER_HEIGHT,
        SidebarRow::Header(_) => SIDEBAR_GROUP_HEADER_HEIGHT + SIDEBAR_GROUP_HEADER_BOTTOM_GAP,
        SidebarRow::Collection(_) => SIDEBAR_COLLECTION_HEADER_HEIGHT,
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
    fn commit_sidebar_drag(&mut self, drag: &SidebarDrag, cx: &mut Context<Self>) {
        let preview = drag.preview.borrow();
        self.apply_sidebar_drop(drag.row, preview.as_ref(), cx);
    }

    pub(super) fn commit_session_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.sidebar_ui.project_group_rename.take() {
            let name = self
                .sidebar_ui
                .session_rename_input
                .read(cx)
                .content()
                .to_owned();
            self.rename_sidebar_collection(id, &name, cx);
            return;
        }
        let Some(session_id) = self.sidebar_ui.session_rename.take() else {
            return;
        };
        let title = self
            .sidebar_ui
            .session_rename_input
            .read(cx)
            .content()
            .to_owned();
        self.rename_sidebar_session(session_id, &title, cx);
    }
}

impl Michelle {
    pub(super) fn focus_sidebar_action(
        &mut self,
        _: &FocusSidebar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.page = None;
        self.commit_session_rename(cx);
        self.set_sidebar_visible(true, cx);
        let rows = self.sidebar_rows_cached();
        let index = self
            .sessions
            .activation
            .pending
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
        let rows = self.sidebar_rows_cached();
        let Some(index) = rows.iter().position(|candidate| *candidate == row) else {
            return;
        };
        let focus = match row {
            SidebarRow::Collection(id) => self
                .menu_handle(format!("project-collection-{id:?}"), cx)
                .trigger_focus_handle()
                .clone(),
            SidebarRow::Header(group) => self
                .sidebar_ui
                .group_header_focuses
                .borrow_mut()
                .entry(group)
                .or_insert_with(|| cx.focus_handle())
                .clone(),
            SidebarRow::ShowMore(group) => self
                .sidebar_ui
                .show_more_focuses
                .borrow_mut()
                .entry(group)
                .or_insert_with(|| cx.focus_handle())
                .clone(),
            SidebarRow::Session(id) => self
                .menu_handle(format!("session-{id}"), cx)
                .trigger_focus_handle()
                .clone(),
            SidebarRow::TopSpacer | SidebarRow::GroupSpacer => return,
        };
        self.sync_sidebar_rows(&rows);
        if self.sidebar_ui.list_state.viewport_bounds().size.height <= Pixels::ZERO {
            self.sidebar_ui.list_state.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: Pixels::ZERO,
            });
        } else {
            reveal_sidebar_list_row(&self.sidebar_ui.list_state, &rows, index);
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
        if self.sidebar_ui.session_rename.is_none()
            && self.sidebar_ui.project_group_rename.is_none()
            && (self.state.sidebar_ordering == SidebarOrdering::Manual
                || matches!(row, SidebarRow::Collection(Some(_))))
            && event.keystroke.modifiers.alt
            && event.keystroke.modifiers.shift
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.platform
            && matches!(key, "up" | "down")
        {
            let rows = self.sidebar_rows_cached();
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
        if self.sidebar_ui.session_rename.is_some()
            || self.sidebar_ui.project_group_rename.is_some()
            || event.keystroke.modifiers.modified()
            || !matches!(key, "up" | "down" | "home" | "end")
        {
            return false;
        }
        let rows = self.sidebar_rows_cached();
        if let Some(index) = sidebar_navigation_target(&rows, row, key) {
            self.focus_sidebar_row(rows[index], window, cx);
        }
        cx.stop_propagation();
        true
    }

    fn sidebar_menu_items(&self, weak: WeakEntity<Self>) -> Vec<MenuItem> {
        let ordering = self.state.sidebar_ordering;
        let ordering_weak = weak.clone();
        let project_weak = weak.clone();
        let mut items = vec![
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
            .icon("folder.badge.plus"),
        ];
        items.push(MenuItem::new(
            tr!("sidebar.new_group"),
            move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.create_sidebar_collection(window, cx));
            },
        ));
        items
    }

    pub(super) fn render_sidebar(
        &self,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let rows = self.sidebar_rows_cached();
        let rows = if cx.has_active_drag() {
            self.sidebar_ui
                .drag_preview
                .borrow()
                .as_ref()
                .map(|preview| preview.rows.clone())
                .unwrap_or(rows)
        } else {
            rows
        };
        let now = Instant::now();
        if cx.reduce_motion() {
            self.sidebar_ui.reorder_animation.borrow_mut().take();
        } else if self.sidebar_ui.reorder_animation.borrow().is_some()
            && self.sidebar_ui.row_cache.borrow().as_slice() != rows.as_slice()
        {
            // A cancelled drag returns to the persisted order from the current
            // visible positions, including an unfinished slide.
            self.animate_sidebar_reorder(&rows, None, now);
        }
        {
            let mut animation = self.sidebar_ui.reorder_animation.borrow_mut();
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
        if self.sidebar_ui.list_state.viewport_bounds().size.height <= Pixels::ZERO
            && let Some(session_id) = self
                .sessions
                .activation
                .pending
                .map(|pending| pending.session_id)
                .or(self.state.selected_session)
        {
            let entity = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                let _ = entity.update(cx, |this, cx| {
                    let selected_session = this
                        .sessions
                        .activation
                        .pending
                        .map(|pending| pending.session_id)
                        .or(this.state.selected_session);
                    if selected_session == Some(session_id) {
                        this.reveal_sidebar_session(session_id);
                        cx.notify();
                    }
                });
            });
        }
        let history_scrolled = self
            .sidebar_ui
            .list_state
            .scroll_px_offset_for_scrollbar()
            .y
            < px(-SIDEBAR_TOP_PADDING - 0.5);
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
                    let top = this.sidebar_ui.list_state.logical_scroll_top();
                    let scroll_y =
                        preview.offsets[top.item_ix.min(preview.rows.len())] + top.offset_in_item;
                    let pointer_y = event.event.position.y - event.bounds.top() + scroll_y;
                    let y = pointer_y - preview.cursor_offset.y + preview.bounds.size.height / 2.0;
                    let changed = if drag.move_to_project {
                        preview.move_session(pointer_y)
                    } else {
                        preview.move_row(y)
                    };
                    if changed {
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
                        this.sidebar_ui.list_state.scroll_to(ListOffset {
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
                    list(
                        self.sidebar_ui.list_state.clone(),
                        move |index, window, cx| {
                            entity
                                .upgrade()
                                .map(|entity| {
                                    entity.update(cx, |this, cx| {
                                        this.sidebar_row(index, &rows, window, cx)
                                    })
                                })
                                .unwrap_or_else(|| div().into_any_element())
                        },
                    )
                    .size_full(),
                ),
            )
            .child(scrollbar::vertical(
                &self.sidebar_ui.list_state,
                &self.sidebar_ui.scrollbar,
            ))
            .when(history_scrolled, |scroll| {
                scroll.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(px(0.5))
                        .bg(theme.border),
                )
            });
        let menu = self.menu_handle("sidebar-background", cx);
        let keyboard_menu = menu.clone();
        let weak = cx.entity().downgrade();
        let sidebar = div()
            .id("sidebar")
            .on_drop(cx.listener(|this, drag: &SidebarDrag, _, cx| {
                if drag.move_to_project {
                    if let Some(preview) = drag.preview.borrow_mut().as_mut() {
                        let outside = *preview.offsets.last().unwrap() + px(1.0);
                        preview.move_session(outside);
                    }
                    this.commit_sidebar_drag(drag, cx);
                }
            }))
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
            .bg(transparent_black())
            .child(self.render_toolbar_sidebar(window, cx))
            .child(history);
        context_menu(sidebar, "sidebar-background-menu", &menu, move |cx| {
            weak.upgrade()
                .map(|entity| entity.read(cx).sidebar_menu_items(weak.clone()))
                .unwrap_or_default()
        })
    }

    /// Keep a newly selected task visible without disturbing the sidebar when
    /// its row is already fully inside the viewport.
    pub(super) fn reveal_sidebar_session(&self, session_id: Uuid) {
        let rows = self.sidebar_rows_cached();
        self.sync_sidebar_rows(&rows);
        if let Some(index) = sidebar_session_row_index(&rows, session_id) {
            reveal_sidebar_list_row(&self.sidebar_ui.list_state, &rows, index);
        }
    }

    fn animate_sidebar_reorder(
        &self,
        rows: &[SidebarRow],
        dragged: Option<SidebarRow>,
        now: Instant,
    ) {
        let mut animation = self.sidebar_ui.reorder_animation.borrow_mut();
        *animation = SidebarReorderAnimation::between(
            &self.sidebar_ui.row_cache.borrow(),
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
        let mut cached = self.sidebar_ui.row_cache.borrow_mut();
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
            self.sidebar_ui
                .list_state
                .reset_with_uniform_height(rows.len(), px(SIDEBAR_SESSION_ROW_HEIGHT));
        } else {
            self.sidebar_ui
                .list_state
                .splice(prefix..old_count, rows.len() - prefix);
            // Newly inserted rows have no measured height yet; give them the
            // uniform hint so the scrollbar keeps a correct total height.
            self.sidebar_ui
                .list_state
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
                .sidebar_ui
                .drag_preview
                .borrow()
                .as_ref()
                .is_some_and(|preview| preview.row == *row);
        let element = match *row {
            SidebarRow::TopSpacer => div().w_full().h(px(SIDEBAR_TOP_PADDING)).into_any_element(),
            SidebarRow::Collection(id) => self.render_sidebar_collection(id, window, true, cx),
            SidebarRow::Header(group) => self
                .render_sidebar_group_header(group, window, cx)
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
            .sidebar_ui
            .reorder_animation
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

    fn render_sidebar_group_header(
        &self,
        group: SidebarGroup,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let collapsed = self.sidebar_ui.collapsed_groups.contains(&group);
        let working = collapsed
            && self
                .sidebar_model
                .working_headers
                .borrow()
                .contains(&SidebarRow::Header(group));
        let group_key = group.element_key();
        let group_name = SharedString::from(format!("sidebar-group-header-{group_key}"));
        let header_focus = self
            .sidebar_ui
            .group_header_focuses
            .borrow_mut()
            .entry(group)
            .or_insert_with(|| cx.focus_handle())
            .clone();
        let is_project = matches!(group, SidebarGroup::Project(_));
        let label = match group {
            SidebarGroup::Project(project_id) => self
                .state
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .map(Project::display_name)
                .unwrap_or_else(|| tr!("project.no_project_name")),
            SidebarGroup::Projectless => tr!("sidebar.chats"),
        };
        let header = if is_project {
            sidebar_item(
                SharedString::from(format!("sidebar-group-toggle-{group_key}")),
                "folder",
                label.clone(),
                SidebarItemProps {
                    disclosure: if collapsed {
                        SidebarDisclosure::Collapsed
                    } else {
                        SidebarDisclosure::Expanded
                    },
                    ..Default::default()
                },
                &theme.ui_colors(),
            )
        } else {
            sidebar_section_header(label.clone(), &theme.ui_colors())
                .pl(px(4.0))
                .id(SharedString::from(format!(
                    "sidebar-group-toggle-{group_key}"
                )))
        }
        .track_focus(&header_focus)
        .tab_index(0)
        .tab_group()
        .tab_stop(true)
        .group(group_name.clone())
        .relative()
        .rounded(px(7.0))
        .cursor_default()
        .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
        .when(!is_project, |header| {
            header.child(
                icon(
                    if collapsed {
                        "chevron.right"
                    } else {
                        "chevron.down"
                    },
                    12.0,
                    theme.text_secondary,
                )
                .invisible()
                .group_hover(group_name, |style| style.visible())
                .when(
                    window.last_input_was_keyboard() && header_focus.is_focused(window),
                    |icon| icon.visible(),
                ),
            )
        })
        .when(working, |element| {
            element.child(
                div()
                    .ml(px(6.0))
                    .w(px(14.0))
                    .flex_none()
                    .child(motion::spinner_slow(14.0, theme.text)),
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

        let header = self.sidebar_reorderable_row(
            header,
            SidebarRow::Header(group),
            label.into(),
            false,
            cx,
        );
        let menu = self.menu_handle(format!("sidebar-{group_key}"), cx);
        let keyboard_menu = menu.clone();
        let weak = cx.entity().downgrade();
        let header = context_menu(
            header.capture_key_down(move |event, window, cx| {
                if (event.keystroke.key == "f10" && event.keystroke.modifiers.shift)
                    || (event.keystroke.key == "enter" && event.keystroke.modifiers.control)
                {
                    keyboard_menu.open_context_menu(window, cx);
                    cx.stop_propagation();
                }
            }),
            format!("sidebar-group-menu-{group_key}"),
            &menu,
            move |cx| {
                weak.upgrade()
                    .map(|michelle| {
                        michelle
                            .read(cx)
                            .sidebar_group_menu_items(group, weak.clone())
                    })
                    .unwrap_or_default()
            },
        );
        div()
            .w_full()
            .when(is_project, |row| {
                row.pb(px(SIDEBAR_GROUP_HEADER_BOTTOM_GAP))
            })
            .child(header)
    }

    fn sidebar_group_menu_items(
        &self,
        group: SidebarGroup,
        weak: WeakEntity<Self>,
    ) -> Vec<MenuItem> {
        let new_task_weak = weak.clone();
        let mut items = vec![
            MenuItem::new(tr!("menu.new_task"), move |window, cx| {
                let _ = new_task_weak.update(cx, |this, cx| {
                    this.open_new_task_for_sidebar_group(group, window, cx);
                });
            })
            .icon("square.and.pencil"),
        ];
        let SidebarGroup::Project(project) = group else {
            return items;
        };
        let selected = self.state.sidebar_group_for_project(project);
        let groups = self
            .state
            .sidebar_project_groups
            .iter()
            .map(|group| (Some(group.id), group.name.clone()))
            .chain(std::iter::once((None, tr!("sidebar.projects"))))
            .collect::<Vec<_>>();
        items.push(MenuItem::Separator);
        items.push(MenuItem::submenu_with_value(
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
        ));
        items
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
                .sidebar_model
                .working_headers
                .borrow()
                .contains(&SidebarRow::Collection(id));
        let menu = self.menu_handle(format!("project-collection-{id:?}"), cx);
        let keyboard_menu = menu.clone();
        let group_name = SharedString::from(format!("project-collection-header-{id:?}"));
        let renaming = interactive && id.is_some() && self.sidebar_ui.project_group_rename == id;
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
                .line_height(px(SIDEBAR_COLLECTION_HEADER_HEIGHT - 2.0))
                .rounded(px(4.0))
                .bg(theme.inset)
                .child(self.sidebar_ui.session_rename_input.clone())
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(label.clone())
                .into_any_element()
        };
        let header = sidebar_section_header(title, &theme.ui_colors())
            .id(group_name.clone())
            .group(group_name.clone())
            .pl(px(4.0))
            .pr(px(8.0))
            .relative()
            .w_full()
            .rounded(px(6.0))
            .gap(px(12.0))
            .cursor_default()
            .child(
                icon("chevron.down", 14.0, theme.text_secondary)
                    .flex_none()
                    .invisible()
                    .group_hover(group_name, |style| style.visible())
                    .when(
                        interactive
                            && !renaming
                            && window.last_input_was_keyboard()
                            && menu.trigger_focus_handle().is_focused(window),
                        |icon| icon.visible(),
                    )
                    .when(collapsed, |icon| {
                        icon.with_transformation(gpui::Transformation::rotate(gpui::percentage(
                            0.75,
                        )))
                    }),
            )
            .when(working, |header| {
                header.child(motion::spinner_slow(14.0, theme.text))
            })
            .when(interactive && !renaming, |header| {
                header
                    .track_focus(menu.trigger_focus_handle())
                    .tab_index(0)
                    .tab_group()
                    .tab_stop(true)
                    .focus_visible(|style| style.shadow(vec![focus_ring(theme.accent)]))
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
            return div().w_full().child(header).into_any_element();
        }
        let header = if renaming {
            header
        } else {
            self.sidebar_reorderable_row(
                header,
                SidebarRow::Collection(id),
                label.into(),
                false,
                cx,
            )
        };
        let weak = cx.entity().downgrade();
        context_menu(
            div().w_full().child(header),
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
                                if this.remove_sidebar_collection(id, cx) {
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

    fn create_sidebar_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_session_rename(cx);
        if let Some(id) = self.add_sidebar_collection(&tr!("sidebar.new_group_name"), cx) {
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
        self.sidebar_ui.project_group_rename = Some(id);
        self.sidebar_ui
            .session_rename_input
            .update(cx, |input, cx| {
                input.set_content(name, cx);
                input.select_all_text(cx);
            });
        let rows = self.sidebar_rows_cached();
        self.sync_sidebar_rows(&rows);
        if let Some(index) = rows
            .iter()
            .position(|row| *row == SidebarRow::Collection(Some(id)))
        {
            reveal_sidebar_list_row(&self.sidebar_ui.list_state, &rows, index);
        }
        let focus = self.sidebar_ui.session_rename_input.read(cx).focus();
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    fn open_new_task_for_sidebar_group(
        &mut self,
        group: SidebarGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.page = None;
        match group {
            SidebarGroup::Project(project_id) => self.select_project(project_id, cx),
            SidebarGroup::Projectless => self.create_projectless_session(cx),
        }
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
    }

    fn render_sidebar_show_more(&self, group: SidebarGroup, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let group_key = group.element_key();
        let focus = self
            .sidebar_ui
            .show_more_focuses
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
            .w_full()
            .h(px(SIDEBAR_SHOW_MORE_ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(px(4.0))
            .cursor_default()
            .text_style(TextStyle::Body)
            .text_color(theme.text_secondary)
            .focus_visible(|style| style.text_color(theme.text))
            .child(
                div()
                    .w(px(if group == SidebarGroup::Projectless {
                        IconSize::Regular.max_width()
                    } else {
                        IconSize::Small.max_width()
                    }))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("chevron.down", 10.0, theme.text_secondary)),
            )
            .child(tr!("sidebar.show_more"))
            .on_click(cx.listener(move |this, event, window, cx| {
                let index = if matches!(event, ClickEvent::Keyboard(_)) {
                    let rows = this.sidebar_rows_cached();
                    rows.iter()
                        .position(|row| *row == SidebarRow::ShowMore(group))
                } else {
                    None
                };
                this.show_more_project_sessions(group, cx);
                if let Some(index) = index {
                    let rows = this.sidebar_rows_cached();
                    if let Some(row) = rows.get(index) {
                        this.focus_sidebar_row(*row, window, cx);
                    }
                }
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.sidebar_navigation_key_down(SidebarRow::ShowMore(group), event, window, cx);
            }));

        div()
            .w_full()
            .h(px(SIDEBAR_SHOW_MORE_ROW_HEIGHT))
            .pl(px(if group == SidebarGroup::Projectless {
                14.0
            } else {
                SIDEBAR_GROUP_CHILD_PADDING
            }))
            .flex()
            .items_center()
            .child(button)
    }

    fn show_more_project_sessions(&mut self, group: SidebarGroup, cx: &mut Context<Self>) {
        let revealed = self
            .sidebar_ui
            .project_reveal_counts
            .entry(group)
            .or_default();
        *revealed = revealed.saturating_add(SIDEBAR_PROJECT_REVEAL_BATCH);
        self.sidebar_model.rows_fingerprint.set(None);
        cx.notify();
    }

    fn toggle_sidebar_group(&mut self, group: SidebarGroup, cx: &mut Context<Self>) {
        let collapsed = !self.sidebar_ui.collapsed_groups.contains(&group);
        self.set_sidebar_group_collapsed(group, collapsed, cx);
    }

    fn sidebar_reorderable_row(
        &self,
        element: Stateful<Div>,
        row: SidebarRow,
        label: SharedString,
        move_to_project: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        element.when(
            matches!(
                row,
                SidebarRow::Collection(Some(_)) | SidebarRow::Header(SidebarGroup::Project(_))
            ) || move_to_project
                || (self.state.sidebar_ordering == SidebarOrdering::Manual
                    && matches!(row, SidebarRow::Session(_))),
            |element| {
                let rows = self.sidebar_model.rows_snapshot.borrow().clone();
                let michelle = cx.entity().downgrade();
                let row_bounds = Rc::new(Cell::new(Bounds::default()));
                let drag_bounds = row_bounds.clone();
                element
                    .relative()
                    .on_drag(
                        SidebarDrag {
                            row,
                            move_to_project,
                            label,
                            michelle: michelle.clone(),
                            preview: Rc::default(),
                        },
                        move |drag, offset, window, cx| {
                            let _ = michelle.update(cx, |this, cx| {
                                let collapsed = match row {
                                    SidebarRow::Header(group) => {
                                        this.sidebar_ui.collapsed_groups.contains(&group)
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
                                    session_destination: None,
                                    bounds: drag_bounds.get(),
                                    cursor_offset: offset,
                                    collapsed,
                                    working: collapsed
                                        && this
                                            .sidebar_model
                                            .working_headers
                                            .borrow()
                                            .contains(&row),
                                });
                                this.sidebar_ui.drag_preview = drag.preview.clone();
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

        self.sidebar_ui.session_rename = Some(session_id);
        self.sidebar_ui
            .session_rename_input
            .update(cx, |input, cx| {
                input.set_content(title, cx);
                input.select_all_text(cx);
            });
        let focus = self.sidebar_ui.session_rename_input.read(cx).focus();
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    pub(super) fn finish_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let group_id = self.sidebar_ui.project_group_rename;
        let session_id = self.sidebar_ui.session_rename;
        self.commit_session_rename(cx);
        if let Some(id) = session_id {
            self.focus_sidebar_row(SidebarRow::Session(id), window, cx);
        }
        if let Some(id) = group_id {
            self.focus_sidebar_row(SidebarRow::Collection(Some(id)), window, cx);
        }
    }

    fn cancel_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.sidebar_ui.project_group_rename.take() {
            self.focus_sidebar_row(SidebarRow::Collection(Some(id)), window, cx);
        }
        if let Some(id) = self.sidebar_ui.session_rename.take() {
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
            self.sessions
                .activation
                .pending
                .map(|pending| pending.session_id),
            session_id,
        );
        let working = matches!(
            session.status,
            SessionStatus::Connecting | SessionStatus::Working
        );
        let projectless = self
            .sidebar_ui
            .drag_preview
            .borrow()
            .as_ref()
            .filter(|preview| preview.row == SidebarRow::Session(session_id))
            .and_then(|preview| preview.session_destination)
            .or_else(|| {
                self.sidebar_model
                    .pending_session_drops
                    .get(&session_id)
                    .copied()
            })
            .map_or_else(
                || {
                    self.sidebar_model
                        .projectless_projects
                        .borrow()
                        .contains(&session.project_id)
                },
                |destination| destination.group == SidebarGroup::Projectless,
            );
        let rename_input = (interactive && self.sidebar_ui.session_rename == Some(session_id))
            .then(|| self.sidebar_ui.session_rename_input.clone());
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
                .text_size(px(13.5))
                .text_color(theme.text)
                .child(rename_input)
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .line_clamp(1)
                .text_size(px(13.5))
                .text_color(theme.text)
                .child(SharedString::from(localized_session_title(session)))
                .into_any_element()
        };
        let michelle = cx.entity().downgrade();
        let menu = self.menu_handle(format!("session-{session_id}"), cx);
        let row_focus = menu.trigger_focus_handle().clone();
        let keyboard_menu = menu.clone();
        let content = div()
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(6.0))
            .overflow_hidden()
            .line_height(px(18.0))
            .child(title)
            .when(working, |element| {
                element.child(motion::spinner_slow(
                    14.0,
                    status_color(&theme, session.status),
                ))
            })
            .when(session.status == SessionStatus::Background, |element| {
                element.child(icon(
                    "hourglass",
                    12.0,
                    status_color(&theme, session.status),
                ))
            })
            .when(session.status == SessionStatus::Waiting, |element| {
                element.child(icon(
                    "exclamationmark.triangle",
                    12.0,
                    status_color(&theme, session.status),
                ))
            })
            .when(session.status == SessionStatus::Failed, |element| {
                element.child(icon("xmark", 12.0, status_color(&theme, session.status)))
            });
        let row = sidebar_item(
            SharedString::from(format!("session-{}", session.id)),
            "bubble.left",
            content,
            SidebarItemProps {
                selected: interactive && selected,
                depth: usize::from(!projectless),
                disclosure: SidebarDisclosure::Reserved,
                icon_size: if projectless {
                    IconSize::Regular
                } else {
                    IconSize::Small
                },
                ..Default::default()
            },
            &theme.ui_colors(),
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
                self.sidebar_session_can_move_to_project(session),
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
                    if this.sidebar_ui.session_rename == Some(session_id) {
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
                    let mut items = vec![
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
                    ];
                    let move_michelle = michelle.clone();
                    items.insert(
                        1,
                        MenuItem::Submenu {
                            label: tr!("sidebar.move_to_project").into(),
                            value: None,
                            items: Rc::new(move |cx| {
                                let Some(entity) = move_michelle.upgrade() else {
                                    return Vec::new();
                                };
                                let this = entity.read(cx);
                                let session = this
                                    .state
                                    .sessions
                                    .iter()
                                    .find(|session| session.id == session_id);
                                let enabled = session.is_some_and(|session| {
                                    this.sidebar_session_can_move_to_project(session)
                                });
                                let mut destinations = this
                                    .state
                                    .projects
                                    .iter()
                                    .filter(|project| {
                                        !this
                                            .sidebar_model
                                            .projectless_projects
                                            .borrow()
                                            .contains(&project.id)
                                            && session.is_some_and(|session| {
                                                session.project_id != project.id
                                            })
                                    })
                                    .map(|project| {
                                        let weak = move_michelle.clone();
                                        let id = project.id;
                                        MenuItem::new(project.display_name(), move |_, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.move_sidebar_session_to_project(
                                                    session_id, id, cx,
                                                );
                                            });
                                        })
                                        .disabled(!enabled)
                                    })
                                    .collect::<Vec<_>>();
                                if session.is_some_and(|session| {
                                    !this
                                        .sidebar_model
                                        .projectless_projects
                                        .borrow()
                                        .contains(&session.project_id)
                                }) {
                                    let weak = move_michelle.clone();
                                    destinations.push(MenuItem::Separator);
                                    destinations.push(
                                        MenuItem::new(tr!("sidebar.chats"), move |_, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.move_sidebar_session(
                                                    session_id, None, None, cx,
                                                );
                                            });
                                        })
                                        .disabled(!enabled),
                                    );
                                }
                                destinations
                            }),
                        },
                    );
                    items
                },
            )
        };

        div()
            .w_full()
            .pb(px(SIDEBAR_SESSION_ROW_GAP))
            .child(row)
            .into_any_element()
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
                .child(icon("sparkles", 24.0, theme.accent))
                .child(
                    div()
                        .mt(px(16.0))
                        .text_size(px(20.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(tr_cow!("onboarding.open_project_to_begin")),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .max_w(px(380.0))
                        .text_center()
                        .text_size(px(12.5))
                        .line_height(px(19.0))
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
                                .track_focus(&self.sidebar_ui.onboarding_add_project_focus)
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
                                .text_size(px(12.5))
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
                                .track_focus(&self.sidebar_ui.onboarding_projectless_focus)
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
                                .text_size(px(12.5))
                                .hover(|element| element.bg(theme.overlay))
                                .active(|element| element.bg(theme.overlay_strong))
                                .child(icon("xmark", 11.0, theme.text_tertiary))
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
            .child(icon("sparkles", 20.0, theme.accent))
            .child(
                div()
                    .mt(px(14.0))
                    .flex()
                    .items_baseline()
                    .text_size(px(20.0))
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
            sidebar_section_header("", &Theme::dark().ui_colors())
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
            SidebarRow::TopSpacer,
            SidebarRow::Header(first),
            SidebarRow::Session(Uuid::from_u128(3)),
            SidebarRow::ShowMore(first),
            SidebarRow::GroupSpacer,
            SidebarRow::Header(collapsed),
            SidebarRow::GroupSpacer,
        ];
        for (current, key, expected) in [
            (1, "up", None),
            (1, "down", Some(2)),
            (2, "down", Some(3)),
            (3, "down", Some(5)),
            (5, "up", Some(3)),
            (5, "down", None),
            (3, "home", Some(1)),
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
    fn collapsed_sidebar_group_keeps_only_its_header_and_spacer() {
        let sessions = [Uuid::from_u128(1), Uuid::from_u128(2)];
        let group = SidebarGroup::Projectless;
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
        let root = Path::new("/tmp/.michelle/projects");
        let project = Uuid::from_u128(1);
        let projectless = Uuid::from_u128(2);
        let collection = Uuid::from_u128(3);
        let mut state = PersistedState::empty();
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
            sidebar_working_headers(&state, Some(root)),
            HashSet::from([
                SidebarRow::Header(SidebarGroup::Project(project)),
                SidebarRow::Header(SidebarGroup::Projectless),
                SidebarRow::Collection(Some(collection)),
            ])
        );
        for session in &mut state.sessions {
            session.status = SessionStatus::Idle;
        }
        assert!(sidebar_working_headers(&state, Some(root)).is_empty());
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
    fn project_sessions_show_up_to_six_or_start_with_five_and_reveal_ten_at_a_time() {
        let sessions = (1..=36).map(Uuid::from_u128).collect::<Vec<_>>();
        for (revealed, count, more) in [
            (0, 5, true),
            (10, 15, true),
            (20, 25, true),
            (30, 35, true),
            (40, 36, false),
            (usize::MAX, 36, false),
        ] {
            let (visible, show_more) = visible_project_sessions(&sessions, revealed);
            assert_eq!(visible, &sessions[..count]);
            assert_eq!(show_more, more);
        }
        for (count, initially_visible, more) in [
            (0, 0, false),
            (1, 1, false),
            (5, 5, false),
            (6, 6, false),
            (7, 5, true),
            (15, 5, true),
            (16, 5, true),
        ] {
            let (visible, show_more) = visible_project_sessions(&sessions[..count], 0);
            assert_eq!(visible, &sessions[..initially_visible]);
            assert_eq!(show_more, more);
            let (visible, show_more) =
                visible_project_sessions(&sessions[..count], SIDEBAR_PROJECT_REVEAL_BATCH);
            assert_eq!(visible, &sessions[..count.min(15)]);
            assert_eq!(show_more, count > 15);
        }
    }

    #[test]
    fn collapsing_a_collection_resets_only_its_project_reveals() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let collection = Uuid::from_u128(3);
        let project = SidebarGroup::Project(a);
        let ungrouped = SidebarGroup::Project(b);
        let mut state = PersistedState::empty();
        state.sidebar_project_groups.push(SidebarProjectGroup {
            id: collection,
            name: "Work".into(),
            projects: vec![a],
            collapsed: false,
        });
        let mut reveals = HashMap::from([
            (project, 20),
            (ungrouped, 10),
            (SidebarGroup::Projectless, 10),
        ]);
        assert!(reset_sidebar_collection_reveals(
            &mut reveals,
            &state,
            Some(collection)
        ));
        assert!(!reveals.contains_key(&project));
        assert_eq!(reveals.get(&ungrouped), Some(&10));
        assert!(!reset_sidebar_collection_reveals(
            &mut reveals,
            &state,
            Some(collection)
        ));
        assert!(reset_sidebar_collection_reveals(&mut reveals, &state, None));
        assert_eq!(reveals, HashMap::from([(SidebarGroup::Projectless, 10)]));
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
            project_sidebar_groups(&sessions, &HashSet::new(), &HashMap::new()),
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

        let groups =
            project_sidebar_groups(&[&second, &first, &third], &HashSet::new(), &HashMap::new());

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
    fn accepted_chat_drop_keeps_its_slot_until_the_daemon_settles() {
        let source = Uuid::from_u128(1);
        let destination = Uuid::from_u128(2);
        let general_workspace = Uuid::from_u128(3);
        let remaining = AgentSession::new(source, ProviderKind::Codex);
        let first = AgentSession::new(destination, ProviderKind::Codex);
        let second = AgentSession::new(destination, ProviderKind::Codex);
        for group in [
            SidebarGroup::Project(destination),
            SidebarGroup::Projectless,
        ] {
            for after in [false, true] {
                let mut moved = AgentSession::new(source, ProviderKind::Codex);
                let projectless = if group == SidebarGroup::Projectless {
                    HashSet::from([destination, general_workspace])
                } else {
                    HashSet::new()
                };
                let pending = HashMap::from([(
                    moved.id,
                    SidebarSessionDrop {
                        group,
                        relative_to: Some((first.id, after)),
                    },
                )]);
                let expected = vec![
                    (SidebarGroup::Project(source), vec![remaining.id]),
                    (
                        group,
                        if after {
                            vec![first.id, moved.id, second.id]
                        } else {
                            vec![moved.id, first.id, second.id]
                        },
                    ),
                ];
                let original = project_sidebar_groups(
                    &[&remaining, &moved, &first, &second],
                    &projectless,
                    &HashMap::new(),
                );
                // Mouse release clears the drag preview before the daemon replies.
                assert_eq!(
                    project_sidebar_groups(
                        &[&remaining, &moved, &first, &second],
                        &projectless,
                        &pending,
                    ),
                    expected
                );
                assert_eq!(moved.project_id, source);
                // Rejection rolls the presentation back without changing session data.
                assert_eq!(
                    project_sidebar_groups(
                        &[&remaining, &moved, &first, &second],
                        &projectless,
                        &HashMap::new(),
                    ),
                    original
                );
                // The catalog can publish the new workspace before the request completes.
                moved.project_id = if group == SidebarGroup::Projectless {
                    general_workspace
                } else {
                    destination
                };
                assert_eq!(
                    project_sidebar_groups(
                        &[&remaining, &moved, &first, &second],
                        &projectless,
                        &pending,
                    ),
                    expected
                );
                let settled = if after {
                    vec![&remaining, &first, &moved, &second]
                } else {
                    vec![&remaining, &moved, &first, &second]
                };
                assert_eq!(
                    project_sidebar_groups(&settled, &projectless, &HashMap::new(),),
                    expected
                );
            }
        }
    }

    #[test]
    fn accepted_chat_drop_stays_visible_after_the_last_visible_chat() {
        let source = Uuid::from_u128(1);
        let destination = Uuid::from_u128(2);
        let moved = AgentSession::new(source, ProviderKind::Codex);
        let peers = (0..10)
            .map(|_| AgentSession::new(destination, ProviderKind::Codex))
            .collect::<Vec<_>>();
        let mut sessions = vec![&moved];
        sessions.extend(peers.iter());
        let pending = HashMap::from([(
            moved.id,
            SidebarSessionDrop {
                group: SidebarGroup::Project(destination),
                relative_to: Some((peers[SIDEBAR_PROJECT_INITIAL_LIMIT - 1].id, true)),
            },
        )]);
        let groups = project_sidebar_groups(&sessions, &HashSet::new(), &pending);
        let (visible, show_more) = visible_project_sessions(&groups[0].1, pending.len());
        assert_eq!(visible.last(), Some(&moved.id));
        assert!(show_more);
        assert!(!visible.contains(&peers[SIDEBAR_PROJECT_INITIAL_LIMIT].id));
    }

    #[test]
    fn chats_are_top_level_and_independent_of_collapsed_projects() {
        let project = SidebarGroup::Project(Uuid::from_u128(1));
        let chat = Uuid::from_u128(2);
        let mut rows = Vec::new();
        append_sidebar_project_collections(
            &mut rows,
            vec![
                vec![SidebarRow::Header(project), SidebarRow::GroupSpacer],
                vec![
                    SidebarRow::Header(SidebarGroup::Projectless),
                    SidebarRow::Session(chat),
                    SidebarRow::GroupSpacer,
                ],
            ],
            &[],
            true,
        );
        assert_eq!(
            rows,
            vec![
                SidebarRow::Collection(None),
                SidebarRow::GroupSpacer,
                SidebarRow::Header(SidebarGroup::Projectless),
                SidebarRow::Session(chat),
                SidebarRow::GroupSpacer,
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
            let mut rows = Vec::new();
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
                SidebarRow::Collection(None),
                SidebarRow::Header(SidebarGroup::Project(other)),
                SidebarRow::Header(SidebarGroup::Project(project)),
                SidebarRow::GroupSpacer,
            ]
        );
        assert_eq!(
            rows,
            vec![
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
            build(&groups, false).first(),
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
            session_destination: None,
            bounds: Bounds::default(),
            cursor_offset: gpui::Point::default(),
            collapsed: false,
            working: false,
        };
        let mut manual = preview(original.clone(), SidebarOrdering::Manual);
        let other_y = manual.offsets[3];
        assert!(!manual.move_project(other_y + px(15.0)));
        assert!(manual.move_project(other_y + px(17.0)));
        assert_eq!(
            &manual.rows[1..6],
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
        assert!(manual.move_project(manual.offsets[1] + px(15.0)));
        assert_eq!(manual.rows, original);
        assert!(!manual.move_project(px(-10.0)));
        assert!(!manual.move_project(*manual.offsets.last().unwrap() + px(10.0)));
        assert!(!manual.move_project(manual.offsets[8] + px(15.0)));

        for ordering in [SidebarOrdering::Newest, SidebarOrdering::Oldest] {
            let (first, second) = if ordering == SidebarOrdering::Newest {
                (other, third)
            } else {
                (third, other)
            };
            let target = SidebarRow::Collection(Some(Uuid::from_u128(20)));
            let rows = Rc::new(vec![
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
            assert!(!automatic.move_project(automatic.offsets[0] + px(15.0)));
            assert!(automatic.move_project(automatic.offsets[4] + px(15.0)));
            assert_eq!(
                &automatic.rows[2..8],
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
            assert!(automatic.move_project(automatic.offsets[9] + px(15.0)));
            assert_eq!(
                &automatic.rows[7..12],
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
            assert!(automatic.move_project(automatic.offsets[0] + px(15.0)));
            assert_eq!(automatic.rows, rows);
        }
        assert_eq!(
            &original[1..6],
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
            session_destination: None,
            bounds: Bounds::default(),
            cursor_offset: gpui::Point::default(),
            collapsed: false,
            working: false,
        };
        let original = Rc::new(vec![
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
        let b_y = groups.offsets[4];
        let before_midpoint = sidebar_row_height(b) / 2.0 - px(1.0);
        let after_midpoint = sidebar_row_height(b) / 2.0 + px(1.0);
        assert!(!groups.move_row(b_y + before_midpoint));
        assert!(groups.move_row(b_y + after_midpoint));
        assert_eq!(
            &groups.rows[0..6],
            &[
                b,
                SidebarRow::GroupSpacer,
                a,
                project,
                first,
                SidebarRow::GroupSpacer
            ]
        );
        assert!(!groups.move_row(b_y + after_midpoint));
        assert!(groups.move_row(groups.offsets[6] + after_midpoint));
        assert_eq!(
            &groups.rows[2..10],
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
        assert!(!groups.move_row(groups.offsets[10] + after_midpoint));
        assert!(groups.move_row(groups.offsets[0] + before_midpoint));
        assert_eq!(groups.rows, original);
        // A collapsed/empty collection still moves together with its spacer.
        let mut collapsed = preview(b, original.clone(), SidebarOrdering::Oldest);
        assert!(collapsed.move_row(collapsed.offsets[6] + after_midpoint));
        assert_eq!(
            &collapsed.rows[4..10],
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

        for header in [project, SidebarRow::Header(SidebarGroup::Projectless)] {
            let original = Rc::new(vec![
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
            let second_y = chats.offsets[2];
            let before_midpoint = sidebar_row_height(second) / 2.0 - px(1.0);
            let after_midpoint = sidebar_row_height(second) / 2.0 + px(1.0);
            assert!(!chats.move_row(second_y + before_midpoint));
            assert!(chats.move_row(second_y + after_midpoint));
            assert_eq!(&chats.rows[1..4], &[second, first, third]);
            assert!(!chats.move_row(second_y + after_midpoint));
            assert!(chats.move_row(chats.offsets[3] + after_midpoint));
            assert_eq!(&chats.rows[1..4], &[second, third, first]);
            assert!(!chats.move_row(chats.offsets[7] + after_midpoint));
            assert!(!chats.move_row(chats.offsets[4] + px(15.0)));
            assert!(!chats.move_row(chats.offsets[6] + px(15.0)));
            assert!(chats.move_row(chats.offsets[1] + before_midpoint));
            assert_eq!(chats.rows, original);
            assert!(
                !preview(first, original, SidebarOrdering::Newest).move_row(second_y + px(27.0))
            );
        }
    }

    #[test]
    fn session_drop_previews_transfer_and_position_in_any_ordering() {
        let source = SidebarRow::Session(Uuid::from_u128(100));
        let first = SidebarRow::Session(Uuid::from_u128(200));
        let second = SidebarRow::Session(Uuid::from_u128(300));
        let target = SidebarGroup::Project(Uuid::from_u128(2));
        let original = Rc::new(vec![
            SidebarRow::Collection(Some(Uuid::from_u128(10))),
            SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(1))),
            source,
            SidebarRow::GroupSpacer,
            SidebarRow::Collection(None),
            SidebarRow::Header(target),
            first,
            second,
            SidebarRow::GroupSpacer,
        ]);
        for ordering in [
            SidebarOrdering::Manual,
            SidebarOrdering::Newest,
            SidebarOrdering::Oldest,
        ] {
            for after in [false, true] {
                let mut preview = SidebarDragPreview {
                    row: source,
                    siblings: sidebar_reorder_siblings(&original, source),
                    offsets: sidebar_row_offsets(&original),
                    rows: original.clone(),
                    ordering,
                    timestamps: HashMap::new(),
                    destination: None,
                    session_destination: None,
                    bounds: Bounds::default(),
                    cursor_offset: gpui::Point::default(),
                    collapsed: false,
                    working: false,
                };
                let y = preview.offsets[6]
                    + sidebar_row_height(first) * if after { 0.75 } else { 0.25 };
                assert!(preview.move_session(y));
                assert_eq!(
                    preview.session_destination,
                    Some(SidebarSessionDrop {
                        group: target,
                        relative_to: Some((Uuid::from_u128(200), after)),
                    })
                );
                let expected = if after {
                    [first, source, second]
                } else {
                    [source, first, second]
                };
                assert_eq!(&preview.rows[5..8], &expected);
                let slot = preview.rows.iter().position(|row| *row == source).unwrap();
                assert!(!preview.move_session(preview.offsets[slot] + px(1.0)));
                // Previewing never mutates the original list, including on cancellation.
                assert_eq!(original[2], source);

                let outside = *preview.offsets.last().unwrap() + px(10.0);
                assert!(preview.move_session(outside));
                assert_eq!(
                    preview.session_destination.unwrap().group,
                    SidebarGroup::Projectless
                );
                assert_eq!(
                    &preview.rows[preview.rows.len() - 3..],
                    &[
                        SidebarRow::Header(SidebarGroup::Projectless),
                        source,
                        SidebarRow::GroupSpacer,
                    ]
                );
                let group_index = preview
                    .rows
                    .iter()
                    .position(|row| *row == SidebarRow::Header(target))
                    .unwrap();
                assert!(preview.move_session(preview.offsets[group_index] + px(1.0)));
                assert_eq!(
                    preview.session_destination.unwrap().relative_to,
                    Some((Uuid::from_u128(200), false))
                );
                assert_eq!(preview.rows.iter().filter(|row| **row == source).count(), 1);

                // A collection gap is outside all projects, even above the Chats group.
                let gap = preview
                    .rows
                    .iter()
                    .position(|row| *row == SidebarRow::GroupSpacer)
                    .unwrap();
                assert!(preview.move_session(preview.offsets[gap] + px(1.0)));
                assert_eq!(
                    preview.session_destination.unwrap().group,
                    SidebarGroup::Projectless
                );
            }
        }
    }

    #[test]
    fn session_drop_places_between_general_chats_and_into_empty_projects() {
        let source = SidebarRow::Session(Uuid::from_u128(100));
        let first = SidebarRow::Session(Uuid::from_u128(200));
        let second = SidebarRow::Session(Uuid::from_u128(300));
        let empty_project = SidebarGroup::Project(Uuid::from_u128(2));
        let rows = Rc::new(vec![
            SidebarRow::TopSpacer,
            SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(1))),
            source,
            SidebarRow::Header(empty_project),
            SidebarRow::GroupSpacer,
            SidebarRow::Header(SidebarGroup::Projectless),
            first,
            second,
            SidebarRow::ShowMore(SidebarGroup::Projectless),
            SidebarRow::GroupSpacer,
        ]);
        let mut preview = SidebarDragPreview {
            row: source,
            siblings: sidebar_reorder_siblings(&rows, source),
            offsets: sidebar_row_offsets(&rows),
            rows,
            ordering: SidebarOrdering::Newest,
            timestamps: HashMap::new(),
            destination: None,
            session_destination: None,
            bounds: Bounds::default(),
            cursor_offset: gpui::Point::default(),
            collapsed: false,
            working: false,
        };
        assert!(!preview.move_session(px(1.0)));
        assert!(preview.session_destination.is_none());
        assert!(preview.move_session(preview.offsets[7] + px(1.0)));
        assert_eq!(
            preview.session_destination,
            Some(SidebarSessionDrop {
                group: SidebarGroup::Projectless,
                relative_to: Some((Uuid::from_u128(300), false)),
            })
        );
        assert_eq!(&preview.rows[5..8], &[first, source, second]);
        let show_more = preview
            .rows
            .iter()
            .position(|row| matches!(row, SidebarRow::ShowMore(_)))
            .unwrap();
        assert!(preview.move_session(preview.offsets[show_more] + px(1.0)));
        assert_eq!(
            preview.session_destination.unwrap().relative_to,
            Some((Uuid::from_u128(300), true))
        );
        let empty = preview
            .rows
            .iter()
            .position(|row| *row == SidebarRow::Header(empty_project))
            .unwrap();
        assert!(preview.move_session(preview.offsets[empty] + px(1.0)));
        assert_eq!(
            preview.session_destination,
            Some(SidebarSessionDrop {
                group: empty_project,
                relative_to: None
            })
        );
        assert_eq!(preview.rows[empty + 1], source);
    }

    #[test]
    fn sidebar_reorder_animation_retargets_from_visible_positions_and_finishes() {
        let a = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(1)));
        let b = SidebarRow::Header(SidebarGroup::Project(Uuid::from_u128(2)));
        let chat_a = SidebarRow::Session(Uuid::from_u128(100));
        let chat_b = SidebarRow::Session(Uuid::from_u128(200));
        let old = [
            SidebarRow::TopSpacer,
            a,
            chat_a,
            SidebarRow::GroupSpacer,
            b,
            chat_b,
            SidebarRow::GroupSpacer,
        ];
        let new = [
            SidebarRow::TopSpacer,
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
        assert!(!animation.offsets.contains_key(&SidebarRow::TopSpacer));
        assert!(!animation.offsets.contains_key(&SidebarRow::GroupSpacer));
        assert_eq!(animation.remaining_at(now), 1.0);
        let interrupted_at = now + SIDEBAR_REORDER_DURATION / 2;
        let remaining = animation.remaining_at(interrupted_at);
        assert!(remaining > 0.0 && remaining < 0.5);
        let reversed =
            SidebarReorderAnimation::between(&new, &old, Some(&animation), None, interrupted_at)
                .unwrap();
        let old_y = sidebar_row_offsets(&old);
        let new_y = sidebar_row_offsets(&new);
        assert_eq!(old_y[1], px(6.0));
        assert_eq!(new_y[1], px(6.0));
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
            &HashMap::new(),
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
        assert_eq!(
            project_sidebar_groups(
                &[&ordinary],
                &HashSet::from([first_projectless_project, second_projectless_project]),
                &HashMap::new(),
            ),
            vec![(SidebarGroup::Project(ordinary_project), vec![ordinary.id])]
        );
        assert!(project_sidebar_groups(&[], &HashSet::new(), &HashMap::new()).is_empty());
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
        let group = SidebarGroup::Projectless;
        let mut rows = vec![SidebarRow::Header(group)];
        rows.extend((1..=40).map(|id| SidebarRow::Session(Uuid::from_u128(id))));
        rows.push(SidebarRow::GroupSpacer);

        let index = sidebar_session_row_index(&rows, target).unwrap();
        let offset = sidebar_bottom_aligned_offset(&rows, index, px(400.0));

        assert_eq!(index, 31);
        assert_eq!(offset.item_ix, 19);
        assert_eq!(offset.offset_in_item, px(29.0));
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
