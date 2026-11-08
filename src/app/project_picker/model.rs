//! Project list projection and selection operations; projects remain in PersistedState.
use super::*;
use crate::app::presentation::project_label;
use michelle_client::persistence::SidebarProjectGroup;

pub(in crate::app) struct ProjectPickerModel {
    pub(in crate::app) matcher: RefCell<nucleo_matcher::Matcher>,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum ProjectPickerRow {
    Header(SharedString),
    Project(Uuid, SharedString),
}

impl ProjectPickerRow {
    pub(super) fn project_id(&self) -> Option<Uuid> {
        match self {
            Self::Header(_) => None,
            Self::Project(id, _) => Some(*id),
        }
    }
}

impl ProjectPickerModel {
    pub(in crate::app) fn new() -> Self {
        Self {
            matcher: RefCell::new(nucleo_matcher::Matcher::new(
                nucleo_matcher::Config::DEFAULT,
            )),
        }
    }
}

impl Michelle {
    pub(super) fn apply_project_picker_action(
        &mut self,
        action: ProjectPickerAction,
        site: ProjectPickerSite,
        cx: &mut Context<Self>,
    ) {
        match action {
            ProjectPickerAction::Select(project_id) => {
                if self.state.selected_project == Some(project_id)
                    && !self.state.sidebar_hidden_projects.contains(&project_id)
                {
                    return;
                }
                match site {
                    ProjectPickerSite::EmptyState => self.select_project(project_id, cx),
                    ProjectPickerSite::Composer => {
                        self.select_project_from_composer(project_id, cx)
                    }
                }
            }
            ProjectPickerAction::NewProject => self.add_project(cx),
            ProjectPickerAction::NoProject => {
                if self.selected_project().is_some_and(Project::is_projectless) {
                    return;
                }
                match site {
                    ProjectPickerSite::EmptyState => self.create_projectless_session(cx),
                    ProjectPickerSite::Composer => {
                        self.create_projectless_session_from_composer(cx)
                    }
                }
            }
        }
    }
}
/// The picker's project rows. Without a query that is every real project, the
/// current one first; with one, only fuzzy matches against the group and project
/// label, best first. The matcher has no length penalty, so equal scores fall
/// back to fzf's default tiebreak — the shorter label — and then to the
/// unfiltered order.
pub(super) fn visible_project_entries(
    projects: &[Project],
    groups: &[SidebarProjectGroup],
    selected: Option<Uuid>,
    query: &str,
    matcher: &mut Matcher,
) -> Vec<(Uuid, SharedString)> {
    let group_names = groups
        .iter()
        .flat_map(|group| {
            group
                .projects
                .iter()
                .map(move |id| (*id, group.name.as_str()))
        })
        .collect::<HashMap<_, _>>();
    let ordered = projects
        .iter()
        .filter(|project| Some(project.id) == selected)
        .chain(
            projects
                .iter()
                .filter(|project| Some(project.id) != selected),
        )
        .filter(|project| !project.is_projectless())
        .map(|project| {
            let label = project_label(&project.name, group_names.get(&project.id).copied());
            (project.id, project.name.clone(), label)
        });
    let query = query.trim();
    if query.is_empty() {
        return ordered.map(|(id, name, _)| (id, name.into())).collect();
    }

    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut utf32 = Vec::new();
    let mut scored = ordered
        .filter_map(|(id, name, label)| {
            let score = pattern.score(Utf32Str::new(&label, &mut utf32), matcher)?;
            Some((score, label.chars().count(), id, SharedString::from(name)))
        })
        .collect::<Vec<_>>();
    // Stable, so full ties hold their unfiltered order.
    scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    scored
        .into_iter()
        .map(|(_, _, project_id, name)| (project_id, name))
        .collect()
}

/// Keep each group together, ordered by its first (selected or best-matching) project.
pub(super) fn group_project_entries(
    entries: Vec<(Uuid, SharedString)>,
    groups: &[SidebarProjectGroup],
) -> Vec<ProjectPickerRow> {
    let membership = groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| group.projects.iter().map(move |id| (*id, index)))
        .collect::<HashMap<_, _>>();
    let mut buckets = vec![Vec::new(); groups.len() + 1];
    let mut order = Vec::new();
    for (id, name) in entries {
        let index = membership.get(&id).copied().unwrap_or(groups.len());
        if buckets[index].is_empty() {
            order.push(index);
        }
        buckets[index].push(ProjectPickerRow::Project(id, name));
    }
    let has_groups = buckets[..groups.len()].iter().any(|rows| !rows.is_empty());
    let mut rows = Vec::new();
    for index in order {
        if let Some(group) = groups.get(index) {
            rows.push(ProjectPickerRow::Header(group.name.clone().into()));
        } else if has_groups {
            rows.push(ProjectPickerRow::Header(tr!("sidebar.projects").into()));
        }
        rows.append(&mut buckets[index]);
    }
    rows
}

pub(super) fn project_row_index(rows: &[ProjectPickerRow], action_index: usize) -> Option<usize> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| row.project_id().map(|_| index))
        .nth(action_index)
}
