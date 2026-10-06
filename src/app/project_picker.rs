//! The project picker behind the empty-state headline and the composer's
//! project chip: a filter field over a virtualized project list, with the
//! add-project and no-project actions pinned beneath it so a long list can
//! never push them out of reach.
//!
//! The two triggers never have their panels open together, so they share one
//! filter field, one keyboard cursor and one list. Rows are only ranked while
//! a panel is open; a closed picker costs what any other closed menu does.

use gpui::ElementId;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use super::*;
use crate::ui::primitives::navigation::next_picker_highlight;

pub(super) mod model;
use model::visible_project_entries;

pub(in crate::app) struct ProjectPickerUi {
    pub(in crate::app) search: Entity<TextInput>,
    pub(in crate::app) highlight: Option<usize>,
    pub(in crate::app) list_state: ListState,
    pub(in crate::app) rows: RefCell<Vec<Uuid>>,
    pub(in crate::app) scrollbar: Rc<ScrollbarState>,
}

impl ProjectPickerUi {
    pub(in crate::app) fn new(
        project_search: Entity<TextInput>,
        project_picker_list_state: ListState,
    ) -> Self {
        Self {
            search: project_search,
            highlight: None,
            list_state: project_picker_list_state,
            rows: RefCell::new(Vec::new()),
            scrollbar: ScrollbarState::new(),
        }
    }
}

const PROJECT_PICKER_ROW_HEIGHT: f32 = 26.0;
/// Rows drawn before the list scrolls. Whole rows only: the edge fade is the
/// cue that more follow.
const PROJECT_PICKER_VISIBLE_ROWS: f32 = 11.0;
const PROJECT_PICKER_WIDTH: f32 = 300.0;

/// Where a picker is drawn, which decides its anchoring and how a choice
/// lands. The composer's variants also carry its draft to the new project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectPickerSite {
    EmptyState,
    Composer,
}

impl ProjectPickerSite {
    fn menu_id(self) -> &'static str {
        match self {
            Self::EmptyState => "empty-state-project",
            Self::Composer => "workspace-project",
        }
    }

    fn align(self) -> MenuAlign {
        match self {
            Self::EmptyState => MenuAlign::BelowLeft,
            // The composer sits at the bottom of the window.
            Self::Composer => MenuAlign::AboveLeft,
        }
    }
}

/// A keyboard-reachable target, in drawn order: the filtered project rows,
/// then the two pinned actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectPickerAction {
    Select(Uuid),
    NewProject,
    NoProject,
}

impl Michelle {
    pub(super) fn open_project_picker_action(
        &mut self,
        _: &crate::OpenProjectPicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let site = ProjectPickerSite::EmptyState;
        let handle = self.project_picker_handle(site, cx);
        if handle.is_open() {
            return;
        }
        let other_open: Vec<_> = self
            .shell_ui
            .menus
            .borrow()
            .values()
            .filter(|menu| menu.is_open())
            .cloned()
            .collect();
        // Toggle observers update Michelle, so release this entity's lease first.
        window.defer(cx, move |window, cx| {
            for menu in other_open {
                menu.close(window, cx);
            }
            crate::ui::menu::toggle_popover(&handle, site.align(), window, cx);
        });
    }

    /// The menu handle for `site`'s picker. Opening resets the filter and the
    /// keyboard cursor and hands focus to the field; closing returns focus to
    /// the composer, as the branch picker does.
    pub(super) fn project_picker_handle(
        &self,
        site: ProjectPickerSite,
        cx: &mut Context<Self>,
    ) -> ContextMenuHandle {
        let weak = cx.entity().downgrade();
        let search = self.project_picker_ui.search.clone();
        let search_focus = search.read(cx).focus_handle(cx);
        self.menu_handle_with(site.menu_id(), cx, move |open, window, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.project_picker_ui.highlight = None;
                if open {
                    // Forces the next sync to reset the list, which also
                    // scrolls the current project back into view.
                    this.project_picker_ui.rows.borrow_mut().clear();
                    search.update(cx, |input, cx| input.clear(cx));
                } else {
                    let focus = this.composer_focus(cx);
                    window.focus(&focus, cx);
                }
                cx.notify();
            });
            if open {
                let search_focus = search_focus.clone();
                window.on_next_frame(move |window, _| {
                    window.on_next_frame(move |window, cx| window.focus(&search_focus, cx));
                });
            }
        })
    }

    pub(super) fn render_project_picker<E>(
        &self,
        trigger: E,
        handle: &ContextMenuHandle,
        site: ProjectPickerSite,
        cx: &mut Context<Self>,
    ) -> AnyElement
    where
        E: ParentElement + Styled + InteractiveElement + IntoElement + 'static,
    {
        let theme = Theme::current(cx);
        let selected_project = self.state.selected_project;
        let projectless_selected = self.selected_project().is_some_and(Project::is_projectless);
        let projects = Rc::new(if handle.is_open() {
            visible_project_entries(
                &self.state.projects,
                &self.state.sidebar_project_groups,
                selected_project,
                self.project_picker_ui.search.read(cx).content(),
                &mut self.project_picker.matcher.borrow_mut(),
            )
        } else {
            Vec::new()
        });
        if handle.is_open() {
            self.sync_project_picker_rows(&projects);
        }
        let actions = Rc::new(
            projects
                .iter()
                .map(|(project_id, _)| ProjectPickerAction::Select(*project_id))
                .chain([
                    ProjectPickerAction::NewProject,
                    ProjectPickerAction::NoProject,
                ])
                .collect::<Vec<_>>(),
        );
        let highlight = self
            .project_picker_ui
            .highlight
            .filter(|index| *index < actions.len());
        let weak = cx.entity().downgrade();
        let search = self.project_picker_ui.search.clone();
        let list_state = self.project_picker_ui.list_state.clone();
        let scrollbar_state = self.project_picker_ui.scrollbar.clone();

        popover(
            trigger,
            handle,
            site.align(),
            move |popover, _window, _cx| {
                let rows = if projects.is_empty() {
                    div()
                        .h(px(64.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(sp(12.5))
                        .text_color(theme.text_ghost)
                        .child(tr!("project.none_found"))
                        .into_any_element()
                } else {
                    let list_projects = projects.clone();
                    let list_weak = weak.clone();
                    let list_popover = popover.clone();
                    let height = (projects.len() as f32).min(PROJECT_PICKER_VISIBLE_ROWS)
                        * PROJECT_PICKER_ROW_HEIGHT;
                    div()
                        .id("project-picker-list")
                        .relative()
                        .w_full()
                        .h(px(height))
                        .flex_none()
                        .px(px(4.0))
                        .child(
                            list(list_state.clone(), move |index, _window, _cx| {
                                let Some((project_id, name)) = list_projects.get(index) else {
                                    return div().into_any_element();
                                };
                                let project_id = *project_id;
                                let select_weak = list_weak.clone();
                                let select_popover = list_popover.clone();
                                project_picker_row(
                                    ("project-row", index),
                                    highlight == Some(index),
                                    &theme,
                                )
                                .w_full()
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .truncate()
                                        .text_color(theme.text)
                                        .child(name.clone()),
                                )
                                .when(selected_project == Some(project_id), |element| {
                                    element.child(icon("checkmark", 11.0, theme.text_secondary))
                                })
                                .on_click(move |_, window, cx| {
                                    select_popover.close(window, cx);
                                    window.refresh();
                                    let _ = select_weak.update(cx, |this, cx| {
                                        this.apply_project_picker_action(
                                            ProjectPickerAction::Select(project_id),
                                            site,
                                            cx,
                                        );
                                    });
                                })
                                .into_any_element()
                            })
                            .size_full(),
                        )
                        .child(scrollbar::edge_fade(
                            list_state.clone(),
                            scrollbar::FadeEdge::Top,
                            theme.raised,
                        ))
                        .child(scrollbar::edge_fade(
                            list_state.clone(),
                            scrollbar::FadeEdge::Bottom,
                            theme.raised,
                        ))
                        .child(scrollbar::vertical(&list_state, &scrollbar_state))
                        .into_any_element()
                };

                let pinned_row = |action: ProjectPickerAction,
                                  id: &'static str,
                                  icon_path: &'static str,
                                  label: String,
                                  selected: bool| {
                    let weak = weak.clone();
                    let popover = popover.clone();
                    let index = actions.iter().position(|candidate| *candidate == action);
                    project_picker_row(id, highlight.is_some() && highlight == index, &theme)
                        .mx(px(4.0))
                        .child(icon(icon_path, 12.0, theme.text_secondary))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .text_color(theme.text)
                                .child(label),
                        )
                        .when(selected, |element| {
                            element.child(icon("checkmark", 11.0, theme.text_secondary))
                        })
                        .on_click(move |_, window, cx| {
                            popover.close(window, cx);
                            window.refresh();
                            let _ = weak.update(cx, |this, cx| {
                                this.apply_project_picker_action(action, site, cx);
                            });
                        })
                };

                let navigate = |key: &'static str| {
                    let weak = weak.clone();
                    let action_count = actions.len();
                    let row_count = projects.len();
                    move |cx: &mut App| {
                        let _ = weak.update(cx, |this, cx| {
                            this.move_project_picker_highlight(key, action_count, row_count, cx);
                        });
                    }
                };
                let next = navigate("down");
                let previous = navigate("up");
                let next_tab = navigate("down");
                let previous_tab = navigate("up");
                let confirm_weak = weak.clone();
                let confirm_actions = actions.clone();
                let confirm_popover = popover.clone();

                div()
                    .w(px(PROJECT_PICKER_WIDTH))
                    // A deferred panel inherits its trigger's text style, and
                    // the empty state's trigger sits in a 20pt medium
                    // headline. Pin the rows' metrics for the filter field.
                    .text_size(sp(12.5))
                    .line_height(sp(15.0))
                    .font_weight(FontWeight::NORMAL)
                    .rounded(px(13.0))
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(theme.raised)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .on_action(move |_: &SelectNextEntry, _, cx| next(cx))
                    .on_action(move |_: &SelectPreviousEntry, _, cx| previous(cx))
                    // Nothing here cycles tabs, so tab steps through the rows
                    // like the command palette's.
                    .on_action(move |_: &SelectNextTab, _, cx| next_tab(cx))
                    .on_action(move |_: &SelectPreviousTab, _, cx| previous_tab(cx))
                    .on_action(move |_: &ConfirmEntry, window, cx| {
                        // Read the live cursor: a keystroke can arrive before
                        // the frame that redraws the last move.
                        let action = confirm_weak
                            .update(cx, |this, _| {
                                confirm_actions
                                    .get(this.project_picker_ui.highlight.unwrap_or(0))
                                    .copied()
                            })
                            .ok()
                            .flatten();
                        let Some(action) = action else {
                            return;
                        };
                        // Close before applying: the toggle observer updates
                        // `Michelle`, so it cannot run inside the update below.
                        confirm_popover.close(window, cx);
                        window.refresh();
                        let _ = confirm_weak.update(cx, |this, cx| {
                            this.apply_project_picker_action(action, site, cx);
                        });
                    })
                    .child(
                        div()
                            .px(px(12.0))
                            .pt(px(10.0))
                            .pb(px(8.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(28.0))
                                    .px(px(10.0))
                                    .rounded(px(9.0))
                                    .bg(theme.surface)
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(icon("magnifyingglass", 12.0, theme.text_secondary))
                                    .child(div().flex_1().min_w_0().child(search.clone())),
                            ),
                    )
                    .child(rows)
                    .child(div().mx(px(6.0)).my(px(4.0)).h(px(1.0)).bg(theme.border))
                    .child(pinned_row(
                        ProjectPickerAction::NewProject,
                        "project-picker-new",
                        "folder.badge.plus",
                        tr!("project.new_project"),
                        false,
                    ))
                    .child(pinned_row(
                        ProjectPickerAction::NoProject,
                        "project-picker-none",
                        "xmark",
                        tr!("project.no_project"),
                        projectless_selected,
                    ))
                    .child(div().h(px(4.0)))
                    .into_any_element()
            },
        )
    }

    /// Keep the virtualized list's item count in step with the filtered rows.
    /// Rows have one fixed height, so a changed set only needs its count.
    fn sync_project_picker_rows(&self, rows: &[(Uuid, SharedString)]) {
        let mut cached = self.project_picker_ui.rows.borrow_mut();
        if cached
            .iter()
            .eq(rows.iter().map(|(project_id, _)| project_id))
        {
            return;
        }
        *cached = rows.iter().map(|(project_id, _)| *project_id).collect();
        self.project_picker_ui
            .list_state
            .reset_with_uniform_height(rows.len(), px(PROJECT_PICKER_ROW_HEIGHT));
    }

    /// Move the drawn selection. The filter field keeps focus throughout, so
    /// typing continues to narrow the list.
    fn move_project_picker_highlight(
        &mut self,
        key: &str,
        action_count: usize,
        row_count: usize,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .project_picker_ui
            .highlight
            .filter(|index| *index < action_count);
        let Some(next) = next_picker_highlight(current, action_count, key) else {
            return;
        };
        self.project_picker_ui.highlight = Some(next);
        // The pinned actions sit outside the list and are always visible.
        if next < row_count {
            self.project_picker_ui
                .list_state
                .scroll_to_reveal_item(next);
        }
        cx.notify();
    }
}

/// The shared row: fixed height for the virtualized list, hover, and the
/// keyboard cursor drawn beneath it.
fn project_picker_row(id: impl Into<ElementId>, highlighted: bool, theme: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(PROJECT_PICKER_ROW_HEIGHT))
        .px(px(8.0))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .cursor_default()
        .text_size(sp(12.5))
        .line_height(sp(15.0))
        .when(highlighted, |element| element.bg(theme.overlay_strong))
        .hover(|element| element.bg(theme.overlay))
        .active(|element| element.opacity(0.85))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use michelle_client::persistence::SidebarProjectGroup;

    use super::*;

    fn project(name: &str) -> Project {
        Project::from_path(PathBuf::from("/work").join(name))
    }

    fn names(entries: &[(Uuid, SharedString)]) -> Vec<&str> {
        entries.iter().map(|(_, name)| name.as_ref()).collect()
    }

    #[test]
    fn project_picker_lists_the_current_project_first() {
        let projects = vec![project("before.town"), project("wikis"), project("crow")];
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let entries =
            visible_project_entries(&projects, &[], Some(projects[1].id), "", &mut matcher);
        assert_eq!(names(&entries), vec!["wikis", "before.town", "crow"]);
    }

    #[test]
    fn project_picker_query_keeps_fuzzy_matches_best_first() {
        let projects = vec![
            project("crow-companion"),
            project("speech-to-subtitle"),
            project("crow"),
            project("michelle-crow-sync"),
        ];
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let entries = visible_project_entries(&projects, &[], None, "  CROW ", &mut matcher);
        // Every match survives, the exact name ranks above its longer
        // namesakes, and the non-matching project is gone.
        assert_eq!(
            names(&entries),
            vec!["crow", "crow-companion", "michelle-crow-sync"]
        );

        let entries = visible_project_entries(&projects, &[], None, "sts", &mut matcher);
        assert_eq!(names(&entries), vec!["speech-to-subtitle"]);

        assert!(visible_project_entries(&projects, &[], None, "zzz", &mut matcher).is_empty());
    }

    #[test]
    fn project_picker_ties_keep_the_unfiltered_order() {
        let projects = vec![project("alpha-one"), project("alpha-two")];
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        // The current project leads ties the same way it leads the full list.
        let entries =
            visible_project_entries(&projects, &[], Some(projects[1].id), "alpha", &mut matcher);
        assert_eq!(names(&entries), vec!["alpha-two", "alpha-one"]);
    }

    #[test]
    fn project_picker_matches_group_and_project_words() {
        let projects = vec![project("frontend"), project("backend"), project("frontend")];
        let mut groups = vec![SidebarProjectGroup {
            id: Uuid::new_v4(),
            name: "geoportal".into(),
            projects: vec![projects[0].id, projects[1].id],
            collapsed: true,
        }];
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

        for query in ["geo front", "FRONT GEO"] {
            let entries = visible_project_entries(&projects, &groups, None, query, &mut matcher);
            assert_eq!(entries[0].0, projects[0].id);
            assert_eq!(names(&entries), vec!["geoportal • frontend"]);
        }

        let entries = visible_project_entries(&projects, &groups, None, "geo", &mut matcher);
        assert_eq!(
            names(&entries),
            vec!["geoportal • backend", "geoportal • frontend"]
        );

        let entries = visible_project_entries(&projects, &groups, None, "frontend", &mut matcher);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(id, _)| *id == projects[2].id));

        groups[0].name = "Maps".into();
        assert!(
            visible_project_entries(&projects, &groups, None, "geo front", &mut matcher).is_empty()
        );
        let entries = visible_project_entries(&projects, &groups, None, "maps front", &mut matcher);
        assert_eq!(names(&entries), vec!["Maps • frontend"]);
    }
}

pub(super) fn subscribe_search(search_input: &Entity<TextInput>, cx: &mut Context<Michelle>) {
    cx.subscribe(
        search_input,
        |this: &mut Michelle, search, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Edited) {
                if search.read(cx).content().trim().is_empty() {
                    this.project_picker_ui.highlight = None;
                } else {
                    this.project_picker_ui.highlight = Some(0);
                    this.project_picker_ui.list_state.scroll_to_reveal_item(0);
                }
                cx.notify();
            }
        },
    )
    .detach();
}
