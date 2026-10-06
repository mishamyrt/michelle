//! Project list projection and selection operations; projects remain in PersistedState.
use super::*;
use crate::app::presentation::project_label;
use michelle_client::persistence::SidebarProjectGroup;

pub(in crate::app) struct ProjectPickerModel {
    pub(in crate::app) matcher: RefCell<nucleo_matcher::Matcher>,
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
                if self.state.selected_project == Some(project_id) {
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
            (project.id, label)
        });
    let query = query.trim();
    if query.is_empty() {
        return ordered.map(|(id, label)| (id, label.into())).collect();
    }

    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut utf32 = Vec::new();
    let mut scored = ordered
        .filter_map(|(id, name)| {
            let score = pattern.score(Utf32Str::new(&name, &mut utf32), matcher)?;
            Some((score, name.chars().count(), id, SharedString::from(name)))
        })
        .collect::<Vec<_>>();
    // Stable, so full ties hold their unfiltered order.
    scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    scored
        .into_iter()
        .map(|(_, _, project_id, name)| (project_id, name))
        .collect()
}
