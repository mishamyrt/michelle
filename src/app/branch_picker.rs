//! Branch picker: search, create form, virtualized rows and keyboard navigation.
use super::*;
use crate::ui::squircle::{SquircleExt, SquircleStyled, squircle};
use crate::ui::{IconSize, SelectableRow, SfSymbol};
use crate::ui::{StyledTypography, TextStyle};
use gpui::BoxShadow;
pub(super) mod model;
use model::visible_branch_entries;
pub(super) const BRANCH_PICKER_MENU_ID: &str = "workspace-branch-picker";
const BRANCH_PICKER_ROW_HEIGHT: f32 = 28.0;

fn branch_picker_enabled(session: Option<&AgentSession>, operation_pending: bool) -> bool {
    !operation_pending && session.is_some_and(|session| !session.is_busy())
}

fn styled_branch_picker_row(
    row: SelectableRow,
    highlighted: bool,
    disabled: bool,
    theme: &Theme,
) -> SelectableRow {
    row.h(px(BRANCH_PICKER_ROW_HEIGHT))
        .px(px(11.0))
        .gap(px(4.0))
        .rounded(px(8.0))
        .highlighted(highlighted)
        .disabled(disabled)
        .checkmark_at_start()
        .font_weight(FontWeight::MEDIUM)
        .bg(if highlighted && !disabled {
            theme.accent
        } else {
            gpui::transparent_black()
        })
        .text_color(if disabled {
            theme.text_ghost
        } else if highlighted {
            gpui::white()
        } else {
            theme.text
        })
        .hover(|style| style.bg(theme.accent).text_color(gpui::white()))
        .active(|style| style.opacity(1.0))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum BranchPickerMode {
    #[default]
    Browse,
    Create,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum BranchPickerAction {
    Checkout(String),
    Create,
}

pub(in crate::app) struct BranchPickerUi {
    pub(in crate::app) search: Entity<TextInput>,
    pub(in crate::app) create_input: Entity<TextInput>,
    pub(in crate::app) mode: BranchPickerMode,
    pub(in crate::app) highlight: Option<usize>,
    pub(in crate::app) list_state: ListState,
    pub(in crate::app) rows: RefCell<Vec<crate::git_branch::BranchEntry>>,
}

impl BranchPickerUi {
    pub(in crate::app) fn new(
        branch_search: Entity<TextInput>,
        branch_create_input: Entity<TextInput>,
        branch_picker_list_state: ListState,
    ) -> Self {
        Self {
            search: branch_search,
            create_input: branch_create_input,
            mode: BranchPickerMode::Browse,
            highlight: None,
            list_state: branch_picker_list_state,
            rows: RefCell::new(Vec::new()),
        }
    }
}

impl Michelle {
    pub(super) fn toggle_branch_picker_action(
        &mut self,
        _: &crate::ToggleBranchPicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.is_some()
            || !branch_picker_enabled(self.selected_session(), self.branches.operation_pending)
            || !self
                .selected_project()
                .is_some_and(|project| !project.is_projectless())
        {
            return;
        }
        let Some(path) = self.selected_workspace_path().map(Path::to_path_buf) else {
            return;
        };
        if self.branch_snapshot_for_workspace(&path, cx).is_none() {
            return;
        }
        self.toggle_composer_menu(BRANCH_PICKER_MENU_ID, window, cx);
    }

    pub(in crate::app) fn render_branch_selector(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::current(cx);
        let session = self.selected_session()?;
        let workspace = session.workspace.clone();
        let workspace_path = self.workspace_path_for_session(session)?.to_path_buf();
        self.selected_project()
            .filter(|project| !project.is_projectless())?;
        let branch_enabled = branch_picker_enabled(Some(session), self.branches.operation_pending);
        let planned_worktree = matches!(workspace, SessionWorkspace::NewWorktree { .. });
        let snapshot = self.branch_snapshot_for_workspace(&workspace_path, cx)?;
        let selected_branch = match &workspace {
            SessionWorkspace::Local => snapshot.display_branch().map(str::to_owned),
            SessionWorkspace::NewWorktree { base_branch } => base_branch
                .clone()
                .or_else(|| snapshot.default_branch.clone())
                .or_else(|| snapshot.display_branch().map(str::to_owned)),
            SessionWorkspace::Worktree { branch, .. } => snapshot
                .current
                .clone()
                .or_else(|| Some(branch.clone()))
                .or_else(|| snapshot.detached_head.clone()),
        }
        .unwrap_or_else(|| tr!("branches.detached_head"));

        let weak = cx.entity().downgrade();
        let search = self.branch_picker_ui.search.clone();
        let create_input = self.branch_picker_ui.create_input.clone();
        let search_focus = search.read(cx).focus_handle(cx);
        let handle = {
            let toggle_weak = weak.clone();
            let reset_search = search.clone();
            let reset_create = create_input.clone();
            let picker_focus = search_focus.clone();
            self.menu_handle_with(BRANCH_PICKER_MENU_ID, cx, move |open, window, cx| {
                let _ = toggle_weak.update(cx, |this, cx| {
                    if open {
                        this.branch_picker_ui.mode = BranchPickerMode::Browse;
                        this.branch_picker_ui.highlight = None;
                        let project_name = this
                            .selected_project()
                            .map(Project::display_name)
                            .unwrap_or_else(|| tr!("project.project_lower"));
                        reset_search.update(cx, |input, cx| {
                            input.set_placeholder(
                                tr!("branches.search_project", project = project_name),
                                cx,
                            );
                            input.clear(cx);
                        });
                        reset_create.update(cx, |input, cx| input.clear(cx));
                        this.refresh_selected_branch_snapshot(cx);
                    } else {
                        this.branch_picker_ui.mode = BranchPickerMode::Browse;
                        let focus = this.composer_focus(cx);
                        window.focus(&focus, cx);
                    }
                    cx.notify();
                });
                if open {
                    let picker_focus = picker_focus.clone();
                    window.on_next_frame(move |window, _| {
                        window.on_next_frame(move |window, cx| window.focus(&picker_focus, cx));
                    });
                }
            })
        };

        let trigger = MenuChip::new("workspace-branch")
            .text_style(TextStyle::CAPTION_2)
            .icon("arrow.triangle.branch", theme.text_secondary)
            .label(if self.branches.operation_pending {
                tr!("branches.switching")
            } else {
                selected_branch.clone()
            })
            .caret(false)
            .disabled(!branch_enabled)
            .selected(branch_enabled && handle.is_open())
            .max_w(px(210.0));
        if !branch_enabled {
            return Some(trigger.into_any_element());
        }

        let normalized_query = self
            .branch_picker_ui
            .search
            .read(cx)
            .content()
            .trim()
            .to_ascii_lowercase();
        let visible_branches = Rc::new(
            if handle.is_open() && self.branch_picker_ui.mode == BranchPickerMode::Browse {
                visible_branch_entries(&snapshot.branches, &selected_branch, &normalized_query)
            } else {
                Vec::new()
            },
        );
        let allow_create = !planned_worktree;
        let actions = Rc::new(
            visible_branches
                .iter()
                .filter(|branch| planned_worktree || !branch.checked_out_elsewhere)
                .map(|branch| BranchPickerAction::Checkout(branch.name.clone()))
                .chain(allow_create.then_some(BranchPickerAction::Create))
                .collect::<Vec<_>>(),
        );
        let highlight = self
            .branch_picker_ui
            .highlight
            .filter(|index| *index < actions.len());
        let mode = self.branch_picker_ui.mode;
        if handle.is_open() && mode == BranchPickerMode::Browse {
            self.sync_branch_picker_rows(&visible_branches);
        }
        let branch_list = self.branch_picker_ui.list_state.clone();

        Some(popover(
            trigger,
            &handle,
            MenuAlign::AboveLeft,
            move |popover, window, cx| {
                let popover = popover.clone();
                let next_actions = actions.clone();
                let previous_actions = actions.clone();
                let confirm_actions = actions.clone();
                let dismiss_weak = weak.clone();
                let next_weak = weak.clone();
                let previous_weak = weak.clone();
                let confirm_weak = weak.clone();
                let confirm_popover = popover.clone();
                let picker_input =
                    |id: &'static str, input: &Entity<TextInput>, symbol: &'static str| {
                        let focused = input.read(cx).is_visually_focused(window);
                        div()
                            .id(id)
                            .w_full()
                            .h(px(32.0))
                            .flex_none()
                            .px(px(10.0))
                            .squircle()
                            .rounded(px(9.0))
                            .border(px(0.5))
                            .border_color(theme.border_strong)
                            .bg(theme.raised)
                            .shadow(if focused {
                                vec![
                                    BoxShadow::new(px(0.0), px(0.0), theme.accent)
                                        .spread_radius(px(1.5))
                                        .inset(),
                                    BoxShadow::new(px(0.0), px(2.0), theme.accent.alpha(0.25))
                                        .blur_radius(px(3.0)),
                                ]
                            } else {
                                vec![]
                            })
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(icon(symbol, 14.0, theme.text_tertiary))
                            .child(div().min_w_0().flex_1().child(input.clone()))
                    };

                let body = if mode == BranchPickerMode::Create {
                    div()
                        .w_full()
                        .p(px(5.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .text_size(sp(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(icon("plus", 14.0, theme.text_secondary))
                                .child(tr!("branches.create_and_checkout")),
                        )
                        .child(
                            picker_input(
                                "branch-picker-create-input",
                                &create_input,
                                "arrow.triangle.branch",
                            )
                            .mt(px(12.0)),
                        )
                        .child(
                            div()
                                .mt(px(9.0))
                                .text_size(sp(12.5))
                                .text_color(theme.text_tertiary)
                                .child(tr!("branches.create_hint")),
                        )
                        .into_any_element()
                } else {
                    let rows = if visible_branches.is_empty() {
                        div()
                            .id("branch-picker-list-empty")
                            .h(px(64.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(sp(12.5))
                            .text_color(theme.text_ghost)
                            .child(tr!("branches.none_found"))
                            .into_any_element()
                    } else {
                        let list_branches = visible_branches.clone();
                        let list_actions = actions.clone();
                        let list_selected_branch = selected_branch.clone();
                        let list_weak = weak.clone();
                        let list_popover = popover.clone();
                        let height =
                            (visible_branches.len() as f32 * BRANCH_PICKER_ROW_HEIGHT).min(260.0);
                        div()
                            .id("branch-picker-list")
                            .w_full()
                            .h(px(height))
                            .flex_none()
                            .child(
                                list(branch_list.clone(), move |index, _window, _cx| {
                                    let Some(branch) = list_branches.get(index) else {
                                        return div().into_any_element();
                                    };
                                    let selected = branch.name == list_selected_branch;
                                    let disabled =
                                        branch.checked_out_elsewhere && !planned_worktree;
                                    let highlighted = highlight
                                        .and_then(|index| list_actions.get(index))
                                        .is_some_and(|action| {
                                            matches!(
                                                action,
                                                BranchPickerAction::Checkout(name)
                                                    if name == &branch.name
                                            )
                                        });
                                    let row = styled_branch_picker_row(
                                        SelectableRow::new(
                                            SharedString::from(format!(
                                                "branch-row-{}",
                                                branch.name
                                            )),
                                            SharedString::from(branch.name.clone()),
                                        )
                                        .selected(selected),
                                        highlighted,
                                        disabled,
                                        &theme,
                                    )
                                    .leading(
                                        SfSymbol::new("arrow.triangle.branch")
                                            .size(IconSize::Small),
                                    );
                                    if disabled {
                                        row.into_any_element()
                                    } else {
                                        let branch_name = branch.name.clone();
                                        let select_weak = list_weak.clone();
                                        let select_popover = list_popover.clone();
                                        row.on_click(move |_, window, cx| {
                                            let should_close = select_weak
                                                .update(cx, |this, cx| {
                                                    this.choose_workspace_branch(
                                                        branch_name.clone(),
                                                        cx,
                                                    )
                                                })
                                                .unwrap_or(false);
                                            if should_close {
                                                select_popover.close(window, cx);
                                                window.refresh();
                                            }
                                        })
                                        .into_any_element()
                                    }
                                })
                                .size_full(),
                            )
                            .into_any_element()
                    };

                    let create_row = allow_create.then(|| {
                        let create_weak = weak.clone();
                        styled_branch_picker_row(
                            SelectableRow::new(
                                "create-workspace-branch",
                                tr!("branches.create_and_checkout_ellipsis"),
                            ),
                            highlight.and_then(|index| actions.get(index))
                                == Some(&BranchPickerAction::Create),
                            false,
                            &theme,
                        )
                        .leading(SfSymbol::new("plus").size(IconSize::Small))
                        .on_click(move |_, window, cx| {
                            let _ = create_weak.update(cx, |this, cx| {
                                this.begin_branch_creation(window, cx);
                            });
                        })
                    });

                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(
                            picker_input("branch-picker-search", &search, "magnifyingglass")
                                .mb(px(10.0)),
                        )
                        .child(
                            div()
                                .h(px(BRANCH_PICKER_ROW_HEIGHT))
                                .px(px(10.0))
                                .flex()
                                .items_center()
                                .text_style(TextStyle::SUBHEADLINE.bold())
                                .text_color(theme.text_tertiary)
                                .child(tr!("branches.title")),
                        )
                        .child(rows)
                        .when_some(create_row, |element, create_row| {
                            element
                                .child(div().mx(px(6.0)).my(px(4.0)).h(px(1.0)).bg(theme.border))
                                .child(create_row)
                        })
                        .into_any_element()
                };

                div()
                    .w(px(320.0))
                    .max_h(px(390.0))
                    .relative()
                    .p(px(5.0))
                    .rounded(px(12.0))
                    .text_style(TextStyle::BODY)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .child(
                        squircle()
                            .rounded(px(12.0))
                            .bg(theme.raised)
                            .border(px(0.5))
                            .border_color(theme.border_strong)
                            .border_inside()
                            .absolute_expand(),
                    )
                    .on_action(move |_: &SelectNextEntry, _, cx| {
                        let _ = next_weak.update(cx, |this, cx| {
                            this.move_branch_picker_highlight("down", &next_actions, cx);
                        });
                    })
                    .on_action(move |_: &SelectPreviousEntry, _, cx| {
                        let _ = previous_weak.update(cx, |this, cx| {
                            this.move_branch_picker_highlight("up", &previous_actions, cx);
                        });
                    })
                    .on_action(move |_: &ConfirmEntry, window, cx| {
                        let should_close = confirm_weak
                            .update(cx, |this, cx| {
                                this.confirm_branch_picker_action(&confirm_actions, window, cx)
                            })
                            .unwrap_or(false);
                        if should_close {
                            confirm_popover.close(window, cx);
                            window.refresh();
                        }
                    })
                    // Escape backs the create form out to browsing. The rest
                    // of the peel is the fields' own clear-on-escape: a
                    // non-empty filter (or typed branch name) clears before
                    // this handler ever sees the keystroke, and an empty
                    // browse view propagates on to the menu's own dismiss.
                    .on_action(move |_: &DismissMenu, window, cx| {
                        let handled = dismiss_weak
                            .update(cx, |this, cx| {
                                if this.branch_picker_ui.mode == BranchPickerMode::Create {
                                    this.cancel_branch_creation(window, cx);
                                    return true;
                                }
                                false
                            })
                            .unwrap_or(false);
                        if !handled {
                            cx.propagate();
                        }
                    })
                    .child(body)
                    .into_any_element()
            },
        ))
    }

    pub(super) fn sync_branch_picker_rows(&self, rows: &[crate::git_branch::BranchEntry]) {
        let mut cached = self.branch_picker_ui.rows.borrow_mut();
        if cached.as_slice() == rows {
            return;
        }
        *cached = rows.to_vec();
        self.branch_picker_ui
            .list_state
            .reset_with_uniform_height(rows.len(), px(BRANCH_PICKER_ROW_HEIGHT));
    }

    pub(super) fn begin_branch_creation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_begin_branch_creation() {
            return;
        }
        self.branch_picker_ui.mode = BranchPickerMode::Create;
        self.branch_picker_ui.highlight = None;
        self.branch_picker_ui
            .create_input
            .update(cx, |input, cx| input.clear(cx));
        let focus = self.branch_picker_ui.create_input.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        });
        cx.notify();
    }

    /// Escape from the create form: back to browsing with the filter
    /// refocused — the reverse of [`begin_branch_creation`], with the same
    /// double-frame focus dance because the search field only exists once
    /// the browse body has rendered.
    ///
    /// [`begin_branch_creation`]: Self::begin_branch_creation
    pub(super) fn cancel_branch_creation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.branch_picker_ui.mode = BranchPickerMode::Browse;
        self.branch_picker_ui.highlight = None;
        let focus = self.branch_picker_ui.search.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        });
        cx.notify();
    }

    pub(super) fn confirm_branch_creation(&mut self, cx: &mut Context<Self>) -> bool {
        if self.branch_picker_ui.mode != BranchPickerMode::Create {
            return false;
        }
        let branch = self
            .branch_picker_ui
            .create_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        self.create_workspace_branch(branch, cx)
    }

    pub(super) fn move_branch_picker_highlight(
        &mut self,
        key: &str,
        actions: &[BranchPickerAction],
        cx: &mut Context<Self>,
    ) {
        if self.branch_picker_ui.mode != BranchPickerMode::Browse || actions.is_empty() {
            return;
        }
        let current = self
            .branch_picker_ui
            .highlight
            .filter(|index| *index < actions.len());
        let next = match (key, current) {
            ("up", Some(0)) => actions.len() - 1,
            ("up", Some(index)) => index - 1,
            ("up", None) => actions.len() - 1,
            (_, Some(index)) => (index + 1) % actions.len(),
            (_, None) => 0,
        };
        self.branch_picker_ui.highlight = Some(next);
        if let Some(BranchPickerAction::Checkout(branch)) = actions.get(next)
            && let Some(row) = self
                .branch_picker_ui
                .rows
                .borrow()
                .iter()
                .position(|entry| entry.name == *branch)
        {
            self.branch_picker_ui.list_state.scroll_to_reveal_item(row);
        }
        cx.notify();
    }

    /// Apply the keyboard-selected action, returning whether the caller should
    /// dismiss the picker after releasing its `Michelle` update lease.
    pub(super) fn confirm_branch_picker_action(
        &mut self,
        actions: &[BranchPickerAction],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.branch_picker_ui.mode == BranchPickerMode::Create {
            return self.confirm_branch_creation(cx);
        }
        let Some(action) = actions.get(self.branch_picker_ui.highlight.unwrap_or(0)) else {
            return false;
        };
        match action {
            BranchPickerAction::Checkout(branch) => {
                self.choose_workspace_branch(branch.clone(), cx)
            }
            BranchPickerAction::Create => {
                self.begin_branch_creation(window, cx);
                false
            }
        }
    }
}

pub(super) fn subscribe_search(search_input: &Entity<TextInput>, cx: &mut Context<Michelle>) {
    cx.subscribe(
        search_input,
        |this: &mut Michelle, search, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Edited)
                && this.branch_picker_ui.mode == BranchPickerMode::Browse
            {
                if search.read(cx).content().trim().is_empty() {
                    this.branch_picker_ui.highlight = None;
                } else {
                    this.branch_picker_ui.highlight = Some(0);
                    this.branch_picker_ui.list_state.scroll_to_reveal_item(0);
                }
                cx.notify();
            }
        },
    )
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_picker_cannot_bypass_busy_or_pending_state() {
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        assert!(!branch_picker_enabled(None, false));
        assert!(branch_picker_enabled(Some(&session), false));
        assert!(!branch_picker_enabled(Some(&session), true));

        session.status = SessionStatus::Working;
        assert!(!branch_picker_enabled(Some(&session), false));
        session.status = SessionStatus::Connecting;
        assert!(!branch_picker_enabled(Some(&session), false));

        session.status = SessionStatus::Idle;
        session.workspace = SessionWorkspace::NewWorktree { base_branch: None };
        assert!(branch_picker_enabled(Some(&session), false));
    }
}
