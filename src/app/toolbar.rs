//! Workspace toolbar composition and its attached menus.
//!
//! The three section renderers stay in their existing pane render paths to
//! preserve cached views and layout. This module owns their controls, visibility,
//! and presentation; session, workspace, and right-panel operations stay with
//! their existing owners.

use crate::ui::TextStyle::BodyEmphasized;
use crate::ui::primitives::timing::TOOLTIP_SHOW_DELAY_MS;
use crate::ui::squircle::{SquircleStyled, squircle};
use crate::ui::toolbar::{
    ToolbarButton, toolbar_button, toolbar_button_segment, toolbar_split_button,
};
use crate::ui::{ActivationExt, IconSize, StyledTypography, TextStyle, sf_icon};
use michelle_client::persistence::SidebarProjectGroup;

use super::background_work::{rendered_work_status_icon, work_status_icon};
use super::presentation::project_label;
use super::right_panel::{file_icon_for_path, reusable_surface_index};
use super::sidebar::model::sidebar_project_is_projectless;
use super::*;

const TAB_SCROLL_FADE_WIDTH: f32 = 24.0;
const BACKGROUND_SUMMARY_MENU_ID: &str = "background-work-summary";
const OPEN_IN_MENU_ID: &str = "open-in-app";
const TASK_ID_COPY_CONTROL_ID: &str = "background-summary-copy-task-id";
const AGENT_THREAD_ID_COPY_CONTROL_ID: &str = "background-summary-copy-agent-thread-id";

#[derive(Clone)]
struct BackgroundSummaryEntry {
    item: BackgroundWorkItem,
    row_focus: FocusHandle,
    stop_focus: FocusHandle,
}

#[derive(Clone)]
struct EnvironmentSummary {
    commit_status: Option<String>,
    commit_focus: FocusHandle,
    compare_focus: FocusHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskIdentifiers {
    task_id: Uuid,
    agent_cli_thread_id: Option<String>,
}

#[derive(Clone)]
struct TaskIdentifierSection {
    values: TaskIdentifiers,
    task_id_copy_focus: FocusHandle,
    agent_cli_thread_id_copy_focus: FocusHandle,
    task_id_copied: bool,
    agent_cli_thread_id_copied: bool,
}

impl From<&AgentSession> for TaskIdentifiers {
    fn from(session: &AgentSession) -> Self {
        Self {
            task_id: session.id,
            agent_cli_thread_id: session.provider_native_id().map(str::to_owned),
        }
    }
}

fn toolbar_project_name(
    session: Option<&AgentSession>,
    projects: &[Project],
    groups: &[SidebarProjectGroup],
    projectless_root: Option<&Path>,
) -> Option<String> {
    let session = session?;
    let project = projects
        .iter()
        .find(|project| project.id == session.project_id)?;
    if sidebar_project_is_projectless(project, projectless_root) {
        return None;
    }
    let group = groups
        .iter()
        .find(|group| group.projects.contains(&project.id))
        .map(|group| group.name.as_str());
    Some(project_label(&project.name, group))
}

impl Michelle {
    pub(super) fn render_toolbar_main(
        &self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::current(cx);
        let session = self.selected_session();
        let title = session
            .map(localized_session_title)
            .unwrap_or_else(|| tr!("session.new_task"));
        let project_name = toolbar_project_name(
            session,
            &self.state.projects,
            &self.state.sidebar_project_groups,
            self.shell_model.projectless_root.as_deref(),
        );
        let agent_preset_label = session
            .filter(|session| session.provider == ProviderKind::DeepSeek && session.has_started())
            .and_then(|session| self.agent_preset_label_for_session(session));
        div()
            .id("window-header")
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            // The sidebar toggle leaves 8px before the pane edge; add 20px
            // here for a 28px gap to the title. While sliding, also clear
            // the traffic lights until the sidebar can host them.
            .pl(if self.shell_ui.sidebar_visible {
                px(
                    20.0 + (TRAFFIC_LIGHT_CLEARANCE - self.shell_ui.sidebar_rendered_width)
                        .max(0.0),
                )
            } else {
                px(0.0)
            })
            .pr(px(8.0))
            .when(!self.shell_ui.sidebar_visible, |element| {
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
                    .child(self.render_sidebar_toggle(cx))
            })
            .child(
                self.window_drag_region(
                    div()
                        .id("header-title-drag-region")
                        .h_full()
                        // Together with the header's 8px gap, leave 20px
                        // between the collapsed sidebar toggle and title.
                        .when(!self.shell_ui.sidebar_visible, |element| {
                            element.pl(px(12.0))
                        })
                        .min_w_0()
                        .flex_shrink(1.0)
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex_shrink(1.0)
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_style(BodyEmphasized)
                                        .text_color(theme.text)
                                        .child(SharedString::from(title)),
                                )
                                .children(project_name.map(|name| {
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(TextStyle::Caption.size())
                                        .line_height(TextStyle::Caption.line_height())
                                        .text_color(theme.text_secondary)
                                        .child(SharedString::from(name))
                                })),
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
                                .child(icon("cpu", 10.5, theme.text_tertiary))
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
            .child(self.render_environment_controls(cx))
            .when(!self.shell_ui.right_panel_visible, |element| {
                element
                    .when(self.shell_ui.fps_counter_visible, |element| {
                        element.child(self.render_fps_counter(cx))
                    })
                    .child(self.render_right_panel_toggle(cx))
            })
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
                this.shell_ui.header_drag_armed = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.shell_ui.header_drag_armed = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.shell_ui.header_drag_armed = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.shell_ui.header_drag_armed {
                    this.shell_ui.header_drag_armed = false;
                    crate::platform::start_window_move(window);
                }
            }))
    }
    fn render_fps_counter(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let fps = self.shell_ui.fps_value;
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

    fn render_sidebar_toggle(&self, cx: &mut Context<Self>) -> ToolbarButton {
        let focus = self.transcript_control_focus("toggle-sidebar", cx);
        toolbar_button("toggle-sidebar", "sidebar.left")
            .track_focus(&focus)
            .tooltip(tr!("menu.toggle_sidebar"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_activation(cx, |this, _, cx| {
                this.set_sidebar_visible(!this.shell_ui.sidebar_visible, cx);
            })
    }

    pub(super) fn render_toolbar_sidebar(
        &self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("sidebar-titlebar")
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .justify_between()
            .items_center()
            .child(
                self.window_drag_region(
                    div()
                        .id("sidebar-traffic-light-drag-region")
                        .min_w(px(TRAFFIC_LIGHT_CLEARANCE))
                        .h_full()
                        .flex_1(),
                    cx,
                ),
            )
            .child(div().child(self.render_sidebar_toggle(cx)).mr(px(8.0)))
    }

    fn render_right_panel_toggle(&self, cx: &mut Context<Self>) -> ToolbarButton {
        let focus = self.transcript_control_focus("toggle-right-panel", cx);
        toolbar_button("toggle-right-panel", "sidebar.right")
            .track_focus(&focus)
            .tooltip(tr!("right_panel.toggle"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_activation(cx, |this, _, cx| {
                this.set_right_panel_visible(!this.shell_ui.right_panel_visible, cx);
            })
    }

    pub(super) fn render_toolbar_right_panel(
        &self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = Theme::current(cx);
        let active_surface = self.right_panel_ui.active_surface;
        let mut tabs = div()
            .id("right-panel-tabs")
            .h_full()
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(4.0))
            .overflow_x_scroll()
            .track_scroll(&self.right_panel_ui.tabs_scroll_handle);
        for (index, surface) in self.right_panel_ui.surfaces.iter().cloned().enumerate() {
            let active = active_surface == Some(index);
            let dirty = self.right_panel_surface_is_dirty(&surface);
            let label = SharedString::from(right_panel_tab_label(
                &surface,
                self.right_panel_ui.files_selected_path.as_deref(),
            ));
            let icon_path =
                right_panel_tab_icon(&surface, self.right_panel_ui.files_selected_path.as_deref());
            let uses_file_icon = matches!(&surface, RightPanelSurface::File(_))
                || matches!(&surface, RightPanelSurface::Files)
                    && self.right_panel_ui.files_selected_path.is_some();
            let activate_weak = cx.entity().downgrade();
            let close_weak = cx.entity().downgrade();
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("right-panel-tab-{index}")))
                    .h(px(28.0))
                    .min_w(px(100.0))
                    .max_w(px(176.0))
                    .px(px(8.0))
                    .rounded(px(6.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_default()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .when(active, |element| element.bg(theme.overlay_strong))
                    .when(!active, |element| {
                        element.hover(|element| element.bg(theme.overlay))
                    })
                    .child(if uses_file_icon {
                        file_icon(icon_path, 13.0, theme.text_secondary).into_any_element()
                    } else {
                        icon(icon_path, 13.0, theme.text_secondary).into_any_element()
                    })
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .text_size(sp(12.5))
                            .text_color(if active {
                                theme.text
                            } else {
                                theme.text_secondary
                            })
                            .child(label),
                    )
                    .when(dirty, |element| {
                        element.child(
                            div()
                                .id(SharedString::from(format!("right-panel-tab-dirty-{index}")))
                                .size(px(7.0))
                                .flex_none()
                                .rounded_full()
                                .bg(theme.warning)
                                .tooltip(|window, cx| {
                                    Tooltip::new(tr!(
                                        "files.unsaved_changes",
                                        shortcut =
                                            crate::platform::primary_shortcut("⌘S", "Ctrl+S")
                                    ))
                                    .build(window, cx)
                                }),
                        )
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("close-right-panel-tab-{index}")))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|element| element.bg(theme.overlay_strong))
                            .child(icon("xmark", 10.0, theme.text_tertiary))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                let _ = close_weak.update(cx, |this, cx| {
                                    this.close_right_panel_surface(index, cx);
                                });
                            }),
                    )
                    .on_click(move |_, _, cx| {
                        let _ = activate_weak.update(cx, |this, cx| {
                            this.activate_right_panel_surface(index, cx);
                        });
                    }),
            );
        }
        tabs = tabs.child(div().w(px(TAB_SCROLL_FADE_WIDTH)).h(px(1.0)).flex_none());

        let mut header = div()
            .id("right-panel-header")
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .pl(px(10.0))
            .pr(px(14.0))
            .child(
                div()
                    .relative()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .child(tabs)
                    .when_some(
                        self.right_panel_ui.pending_tab_reveal,
                        |element, tab_index| {
                            element.child(tab_scroll_reveal_guard(
                                self.right_panel_ui.tabs_scroll_handle.clone(),
                                tab_index,
                                cx.entity().downgrade(),
                            ))
                        },
                    )
                    .child(tab_scroll_fade(
                        self.right_panel_ui.tabs_scroll_handle.clone(),
                        TabScrollFadeSide::Left,
                        theme.surface,
                    ))
                    .child(tab_scroll_fade(
                        self.right_panel_ui.tabs_scroll_handle.clone(),
                        TabScrollFadeSide::Right,
                        theme.surface,
                    )),
            );

        if !self.right_panel_ui.surfaces.is_empty() {
            let weak = cx.entity().downgrade();
            let existing_surfaces = self.right_panel_ui.surfaces.clone();
            let options = [
                RightPanelSurface::new_terminal(),
                RightPanelSurface::Files,
                RightPanelSurface::Diff,
            ];
            let handle = self.menu_handle("add-right-panel-surface", cx);
            header = header.child(
                div()
                    .flex_none()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(dropdown_menu(
                        icon_button("add-right-panel-surface", "plus", theme.ui_colors()),
                        "add-right-panel-surface-menu",
                        &handle,
                        MenuAlign::BelowRight,
                        move |_| {
                            options
                                .clone()
                                .into_iter()
                                .map(|surface| {
                                    let weak = weak.clone();
                                    let open_surface = surface.clone();
                                    let already_open =
                                        reusable_surface_index(&existing_surfaces, &surface)
                                            .is_some();
                                    MenuItem::new(surface.label(), move |_, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_right_panel_surface(open_surface.clone(), cx);
                                        });
                                    })
                                    .icon(surface.icon_path())
                                    .selected(already_open)
                                })
                                .collect()
                        },
                    )),
            );
        }

        self.window_drag_region(header.child(self.render_right_panel_toggle(cx)), cx)
    }

    fn render_environment_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = self.selected_session();
        let session_id = session.map(|session| session.id);
        let identifiers = session.map(|session| TaskIdentifierSection {
            values: TaskIdentifiers::from(session),
            task_id_copy_focus: self.transcript_control_focus(TASK_ID_COPY_CONTROL_ID, cx),
            agent_cli_thread_id_copy_focus: self
                .transcript_control_focus(AGENT_THREAD_ID_COPY_CONTROL_ID, cx),
            task_id_copied: self.control_was_copied(TASK_ID_COPY_CONTROL_ID),
            agent_cli_thread_id_copied: self.control_was_copied(AGENT_THREAD_ID_COPY_CONTROL_ID),
        });
        let entries = session_id
            .and_then(|session_id| self.sessions.runtime.background_work.get(&session_id))
            .map(|registry| {
                registry
                    .ordered_items()
                    .into_iter()
                    .cloned()
                    .map(|item| {
                        let kind = item.key.kind as u8;
                        let provider_id = &item.key.provider_id;
                        BackgroundSummaryEntry {
                            row_focus: self.transcript_control_focus(
                                format!("background-summary-row-{provider_id}-{kind}"),
                                cx,
                            ),
                            stop_focus: self.transcript_control_focus(
                                format!("background-summary-stop-{provider_id}-{kind}"),
                                cx,
                            ),
                            item,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let workspace_path = session
            .and_then(|session| self.workspace_path_for_session(session))
            .or_else(|| {
                self.selected_project()
                    .map(|project| project.path.as_path())
            });
        let snapshot = workspace_path.and_then(|path| {
            self.branches
                .visible_snapshot
                .as_ref()
                .filter(|(snapshot_path, _)| snapshot_path == path)
                .map(|(_, snapshot)| snapshot)
        });
        let environment = Some(EnvironmentSummary {
            commit_status: self.commit_operation_status_label(),
            commit_focus: self.transcript_control_focus("environment-summary-commit", cx),
            compare_focus: self.transcript_control_focus("environment-summary-compare", cx),
        });
        let (processes, agents) = session_id
            .map(|session_id| self.background_work_counts(session_id))
            .unwrap_or_default();
        let has_live_work = processes > 0 || agents > 0;
        let summary = background_work_count_summary(processes, agents);
        let theme = Theme::current(cx);
        let refresh_weak = cx.entity().downgrade();
        let handle = self.menu_handle_with(BACKGROUND_SUMMARY_MENU_ID, cx, move |open, _, cx| {
            if open {
                let _ = refresh_weak.update(cx, |this, cx| {
                    this.refresh_selected_branch_snapshot(cx);
                });
            }
        });
        let trigger = toolbar_button("environment-summary-trigger", "info.circle")
            .relative()
            .selected(handle.is_open())
            .tooltip(if summary.is_empty() {
                tr!("environment.summary")
            } else {
                summary
            })
            .when(has_live_work, |trigger| {
                trigger.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .right(px(4.0))
                        .child(pulse_dot(5.0, theme.accent)),
                )
            });
        let open_in = self.render_open_in_control(workspace_path, cx);
        let compose_focus = self.transcript_control_focus("header-compose", cx);
        let compose = toolbar_button("header-compose", "square.and.pencil")
            .track_focus(&compose_focus)
            .tooltip(tr!("menu.new_task"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_activation(cx, |this, window, cx| {
                this.new_session_action(&NewSession, window, cx);
            });
        let entries = Rc::new(entries);
        let weak = cx.entity().downgrade();
        let info = popover(
            trigger,
            &handle,
            MenuAlign::BelowRight,
            move |handle, _, cx| {
                render_background_summary_card(
                    handle,
                    session_id.unwrap_or_else(Uuid::nil),
                    identifiers.clone(),
                    environment.clone(),
                    entries.clone(),
                    weak.clone(),
                    cx,
                )
            },
        );
        div()
            .id("header-environment-controls")
            .tab_group()
            .tab_stop(false)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(compose)
            .children(open_in)
            .child(info)
            .into_any_element()
    }

    /// The app the primary "open in" button targets: the persisted choice
    /// while it is still installed, otherwise the file manager.
    fn preferred_open_in_app(&self) -> Option<&crate::platform::ExternalApp> {
        self.state
            .open_in_app
            .as_deref()
            .and_then(|id| {
                self.shell_model
                    .open_in_apps
                    .iter()
                    .find(|app| app.id == id)
            })
            .or_else(|| {
                self.shell_model
                    .open_in_apps
                    .iter()
                    .find(|app| app.id == "finder")
            })
            .or_else(|| self.shell_model.open_in_apps.first())
    }

    /// The split "open project in app" control: an icon button launching the
    /// preferred app, and an ellipsis opening the menu of every installed
    /// target. Hidden while there is no local folder to open or app detection
    /// has not landed yet.
    fn render_open_in_control(
        &self,
        workspace_path: Option<&Path>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.daemon.is_remote() {
            return None;
        }
        let path: Rc<Path> = Rc::from(workspace_path?);
        let preferred = self.preferred_open_in_app()?;
        let preferred_id = preferred.id;
        let preferred_label = preferred.label;
        let preferred_icon = preferred.icon.clone();
        let apps = self.shell_model.open_in_apps.clone();
        let theme = Theme::current(cx);
        let handle = self.menu_handle(OPEN_IN_MENU_ID, cx);
        let focus = self.transcript_control_focus("header-open-in", cx);

        let primary_path = path.clone();
        let primary = toolbar_button_segment("header-open-in", theme.ui_colors())
            .track_focus(&focus)
            .tooltip(Tooltip::text(tr!("open_in.open", app = preferred_label)))
            .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
            .child(img(preferred_icon).size(px(22.0)).flex_none())
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_activation(cx, move |this, _, cx| {
                this.open_workspace_in_app(&primary_path, preferred_id, cx);
            });

        let more = toolbar_button_segment("header-open-in-caret", theme.ui_colors())
            .when(handle.is_open(), |style| style.bg(theme.overlay_strong))
            .tooltip(Tooltip::text(tr!("open_in.choose")))
            .tooltip_show_delay(TOOLTIP_SHOW_DELAY_MS)
            .child(sf_icon(
                "ellipsis",
                IconSize::Regular.font_size(),
                theme.text,
            ));

        let weak = cx.entity().downgrade();
        let menu = dropdown_menu(
            more,
            "header-open-in-menu",
            &handle,
            MenuAlign::BelowRight,
            move |_| {
                apps.iter()
                    .map(|app| {
                        let weak = weak.clone();
                        let path = path.clone();
                        let app_id = app.id;
                        MenuItem::new(app.label, move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.open_workspace_in_app(&path, app_id, cx);
                            });
                        })
                        .image(app.icon.clone())
                        .selected(app.id == preferred_id)
                    })
                    .collect()
            },
        );

        Some(toolbar_split_button(primary, menu, theme.ui_colors()).into_any_element())
    }
}

fn right_panel_tab_label(surface: &RightPanelSurface, files_selected_path: Option<&str>) -> String {
    let label = match surface {
        RightPanelSurface::Files => files_selected_path
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| tr!("right_panel.files")),
        _ => surface.label(),
    };
    single_line_label(&label)
}

fn right_panel_tab_icon(
    surface: &RightPanelSurface,
    files_selected_path: Option<&str>,
) -> &'static str {
    match surface {
        RightPanelSurface::Files => files_selected_path
            .map(file_icon_for_path)
            .unwrap_or_else(|| surface.icon_path()),
        _ => surface.icon_path(),
    }
}

#[derive(Clone, Copy)]
enum TabScrollFadeSide {
    Left,
    Right,
}

fn tab_scroll_fade_visibility(offset_x: Pixels, max_offset: Pixels) -> (bool, bool) {
    let scrolled = -offset_x;
    let threshold = px(0.5);
    (scrolled > threshold, max_offset - scrolled > threshold)
}

fn fade_safe_tab_offset(
    current_offset: Pixels,
    max_offset: Pixels,
    item_left: Pixels,
    item_right: Pixels,
    viewport_left: Pixels,
    viewport_right: Pixels,
) -> Pixels {
    let inset = px(TAB_SCROLL_FADE_WIDTH);
    let mut offset = current_offset;
    let visible_left = item_left + offset;
    let visible_right = item_right + offset;
    if visible_left < viewport_left + inset {
        offset += viewport_left + inset - visible_left;
    } else if visible_right > viewport_right - inset {
        offset -= visible_right - (viewport_right - inset);
    }
    offset.clamp(-max_offset, px(0.0))
}

fn tab_scroll_reveal_guard(
    scroll_handle: ScrollHandle,
    tab_index: usize,
    michelle: WeakEntity<Michelle>,
) -> impl IntoElement {
    canvas(
        move |_, window, _| {
            if let Some(item) = scroll_handle.bounds_for_item(tab_index) {
                let viewport = scroll_handle.bounds();
                let offset = scroll_handle.offset();
                let safe_offset = fade_safe_tab_offset(
                    offset.x,
                    scroll_handle.max_offset().x,
                    item.left(),
                    item.right(),
                    viewport.left(),
                    viewport.right(),
                );
                if safe_offset != offset.x {
                    scroll_handle.set_offset(point(safe_offset, offset.y));
                }
            }

            window.on_next_frame(move |_, cx| {
                let _ = michelle.update(cx, |this, cx| {
                    if this.right_panel_ui.pending_tab_reveal == Some(tab_index) {
                        this.right_panel_ui.pending_tab_reveal = None;
                        cx.notify();
                    }
                });
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

fn tab_scroll_fade(
    scroll_handle: ScrollHandle,
    side: TabScrollFadeSide,
    surface: Hsla,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let (show_left, show_right) =
                tab_scroll_fade_visibility(scroll_handle.offset().x, scroll_handle.max_offset().x);
            let visible = match side {
                TabScrollFadeSide::Left => show_left,
                TabScrollFadeSide::Right => show_right,
            };
            visible.then(|| {
                let transparent = surface.opacity(0.0);
                let background = match side {
                    TabScrollFadeSide::Left => linear_gradient(
                        90.0,
                        linear_color_stop(surface, 0.0),
                        linear_color_stop(transparent, 1.0),
                    ),
                    TabScrollFadeSide::Right => linear_gradient(
                        90.0,
                        linear_color_stop(transparent, 0.0),
                        linear_color_stop(surface, 1.0),
                    ),
                };
                fill(bounds, background)
            })
        },
        |_, fade, window, _| {
            if let Some(fade) = fade {
                window.paint_quad(fade);
            }
        },
    )
    .absolute()
    .top_0()
    .bottom_0()
    .when(matches!(side, TabScrollFadeSide::Left), |element| {
        element.left_0()
    })
    .when(matches!(side, TabScrollFadeSide::Right), |element| {
        element.right_0()
    })
    .w(px(TAB_SCROLL_FADE_WIDTH))
}

fn background_summary_process_status_icon(
    kind: BackgroundWorkKind,
    status: BackgroundWorkStatus,
) -> Option<&'static str> {
    if !matches!(
        kind,
        BackgroundWorkKind::Process | BackgroundWorkKind::Monitor
    ) {
        return None;
    }

    match status {
        BackgroundWorkStatus::Starting
        | BackgroundWorkStatus::Running
        | BackgroundWorkStatus::Monitoring
        | BackgroundWorkStatus::Completed
        | BackgroundWorkStatus::Failed => Some(work_status_icon(status)),
        _ => None,
    }
}

fn background_work_count_summary(processes: usize, agents: usize) -> String {
    let mut parts = Vec::new();
    if processes > 0 {
        parts.push(if processes == 1 {
            tr!("background.process_count_one")
        } else {
            tr!("background.process_count", count = processes)
        });
    }
    if agents > 0 {
        parts.push(if agents == 1 {
            tr!("background.agent_count_one")
        } else {
            tr!("background.agent_count", count = agents)
        });
    }
    parts.join(" · ")
}

fn render_background_summary_card(
    handle: &ContextMenuHandle,
    session_id: Uuid,
    identifiers: Option<TaskIdentifierSection>,
    environment: Option<EnvironmentSummary>,
    entries: Rc<Vec<BackgroundSummaryEntry>>,
    weak: WeakEntity<Michelle>,
    cx: &mut App,
) -> AnyElement {
    let theme = Theme::current(cx);
    let processes = entries
        .iter()
        .filter(|entry| entry.item.key.kind != BackgroundWorkKind::Subagent)
        .cloned()
        .collect::<Vec<_>>();
    let agents = entries
        .iter()
        .filter(|entry| entry.item.key.kind == BackgroundWorkKind::Subagent)
        .cloned()
        .collect::<Vec<_>>();
    let mut content = div()
        .id("background-summary-scroll")
        .max_h(px(420.0))
        .overflow_y_scroll()
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap(px(8.0));
    let has_environment = environment.is_some();
    let has_background = !processes.is_empty() || !agents.is_empty();
    let has_identifiers = identifiers.is_some();
    if let Some(environment) = environment {
        content = content.child(render_environment_summary_section(
            environment,
            handle.clone(),
            weak.clone(),
            &theme,
        ));
    }
    if has_environment && has_background {
        content = content.child(div().mx(px(8.0)).h(px(1.0)).bg(theme.border));
    }
    if !processes.is_empty() {
        content = content.child(render_background_summary_section(
            tr!("background.processes"),
            processes,
            session_id,
            handle.clone(),
            weak.clone(),
            &theme,
        ));
    }
    if !agents.is_empty() {
        content = content.child(render_background_summary_section(
            tr!("background.agents"),
            agents,
            session_id,
            handle.clone(),
            weak.clone(),
            &theme,
        ));
    }
    if has_identifiers && (has_environment || has_background) {
        content = content.child(div().mx(px(8.0)).h(px(1.0)).bg(theme.border));
    }
    if let Some(identifiers) = identifiers {
        content = content.child(render_task_identifiers_section(identifiers, weak, &theme));
    }
    div()
        .id("background-summary-card")
        .track_focus(handle.focus_handle())
        .w(px(300.0))
        .rounded(px(12.0))
        .child(
            squircle()
                .rounded(px(12.0))
                .bg(theme.raised)
                .border(px(0.5))
                .border_color(theme.border_strong)
                .border_inside()
                .absolute_expand()
                .shadow_lg(),
        )
        // .overflow_hidden()
        .child(content)
        .into_any_element()
}

fn render_task_identifiers_section(
    section: TaskIdentifierSection,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Div {
    let mut rows = vec![render_task_identifier_row(
        tr!("environment.task_id"),
        section.values.task_id.to_string(),
        TASK_ID_COPY_CONTROL_ID,
        &section.task_id_copy_focus,
        section.task_id_copied,
        weak.clone(),
        theme,
    )];
    if let Some(thread_id) = section.values.agent_cli_thread_id {
        rows.push(render_task_identifier_row(
            tr!("environment.agent_cli_thread_id"),
            thread_id,
            AGENT_THREAD_ID_COPY_CONTROL_ID,
            &section.agent_cli_thread_id_copy_focus,
            section.agent_cli_thread_id_copied,
            weak,
            theme,
        ));
    }

    div()
        .w_full()
        .tab_group()
        .tab_stop(false)
        .flex()
        .flex_col()
        .gap(px(7.0))
        .children(rows)
}

#[allow(clippy::too_many_arguments)]
fn render_task_identifier_row(
    label: String,
    value: String,
    control_id: &'static str,
    focus: &FocusHandle,
    copied: bool,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Div {
    let tooltip = Tooltip::text(if copied {
        tr!("common.copied")
    } else {
        tr!("common.copy_named", name = label.clone())
    });
    let copy_value = value.clone();
    let copy_action = Rc::new(move |cx: &mut App| {
        cx.write_to_clipboard(ClipboardItem::new_string(copy_value.clone()));
        let _ = weak.update(cx, |this, cx| {
            this.show_control_copied(control_id, cx);
        });
    });
    let key_copy_action = copy_action.clone();
    let copy_button = div()
        .id(control_id)
        .track_focus(focus)
        .tab_index(0)
        .size(px(24.0))
        .rounded(px(6.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_default()
        .focus_visible(|style| {
            style
                .bg(theme.overlay)
                .border_1()
                .border_color(theme.accent)
        })
        .hover(|style| style.bg(theme.overlay_strong))
        .active(|style| style.bg(theme.overlay))
        .tooltip(tooltip)
        .child(icon(
            if copied { "checkmark" } else { "doc.on.doc" },
            12.0,
            theme.text_tertiary,
        ))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| {
            copy_action(cx);
            cx.stop_propagation();
        })
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                key_copy_action(cx);
                cx.stop_propagation();
            }
        });

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div().h(px(20.0)).px(px(8.0)).flex().items_center().child(
                div()
                    .text_size(sp(12.0))
                    .text_color(theme.text_tertiary)
                    .child(label),
            ),
        )
        .child(
            div()
                .w_full()
                .h(px(28.0))
                .pl(px(8.0))
                .rounded(px(6.0))
                .bg(theme.inset)
                .flex()
                .items_center()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(sp(11.5))
                        .font_family(md::render::MONO_FAMILY)
                        .text_color(theme.text_secondary)
                        .child(value),
                )
                .child(copy_button),
        )
}

fn render_environment_summary_section(
    environment: EnvironmentSummary,
    handle: ContextMenuHandle,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Div {
    let commit_handle = handle.clone();
    let commit_weak = weak.clone();
    let commit_pending = environment.commit_status.is_some();
    let commit = render_environment_action_row(
        "environment-summary-commit",
        &environment.commit_focus,
        "point.topleft.down.to.point.bottomright.curvepath",
        environment
            .commit_status
            .unwrap_or_else(|| tr!("environment.commit_or_push")),
        !commit_pending,
        commit_pending,
        None,
        theme,
        move |window, cx| {
            commit_handle.close(window, cx);
            window.refresh();
            let _ = commit_weak.update(cx, |this, cx| {
                this.open_commit_dialog(window, cx);
            });
        },
    );

    let compare_handle = handle;
    let compare_weak = weak;
    let compare = render_environment_action_row(
        "environment-summary-compare",
        &environment.compare_focus,
        "icons/github.svg",
        tr!("environment.compare_branch"),
        true,
        false,
        Some(icon("arrow.up.right", 13.0, theme.text_tertiary).into_any_element()),
        theme,
        move |window, cx| {
            compare_handle.close(window, cx);
            window.refresh();
            let _ = compare_weak.update(cx, |this, cx| {
                this.set_right_panel_diff_source(ReviewDiffSource::Branch, cx);
            });
        },
    );

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_0()
        .child(
            div()
                .h(px(30.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .text_style(TextStyle::CaptionEmphasized)
                .text_color(theme.text_tertiary)
                .child(tr!("environment.title")),
        )
        .child(commit)
        .child(compare)
}

fn render_environment_action_row(
    id: &'static str,
    focus: &FocusHandle,
    icon_path: &'static str,
    label: String,
    enabled: bool,
    active: bool,
    trailing: Option<AnyElement>,
    theme: &Theme,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let foreground = if enabled {
        theme.text
    } else if active {
        theme.text_secondary
    } else {
        theme.text_ghost
    };
    let icon_foreground = if enabled || active {
        theme.text_secondary
    } else {
        theme.text_ghost
    };
    let indicator = if active {
        motion::spinner_slow(14.0, theme.text_secondary)
    } else {
        icon(icon_path, 14.0, icon_foreground).into_any_element()
    };
    let action: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(action);
    let key_action = action.clone();
    div()
        .id(id)
        .track_focus(focus)
        .when(enabled, |row| row.tab_index(0))
        .min_h(px(32.0))
        .w_full()
        .px(px(8.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .cursor_default()
        .focus_visible(|style| style.border_1().border_color(theme.accent))
        .when(enabled, |row| {
            row.hover(|style| style.bg(theme.overlay_strong))
        })
        .child(indicator)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(sp(13.5))
                .text_color(foreground)
                .child(label),
        )
        .children(trailing)
        .when(enabled, |row| {
            row.on_click(move |_, window, cx| action(window, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key_action(window, cx);
                        cx.stop_propagation();
                    }
                })
        })
}

fn render_background_summary_section(
    label: String,
    entries: Vec<BackgroundSummaryEntry>,
    session_id: Uuid,
    handle: ContextMenuHandle,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Div {
    let mut rows = div().w_full().flex().flex_col().gap(px(2.0));
    for entry in entries {
        rows = rows.child(render_background_summary_row(
            entry,
            session_id,
            handle.clone(),
            weak.clone(),
            theme,
        ));
    }
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .px(px(8.0))
                .text_style(TextStyle::Caption)
                .text_color(theme.text_tertiary)
                .child(label),
        )
        .child(rows)
}

fn render_background_summary_row(
    entry: BackgroundSummaryEntry,
    session_id: Uuid,
    handle: ContextMenuHandle,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Stateful<Div> {
    let item = entry.item;
    let group_name = SharedString::from(format!(
        "background-summary-group-{}-{}",
        item.key.provider_id, item.key.kind as u8
    ));
    let status = background_summary_process_status_icon(item.key.kind, item.status).map(|_| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .when(item.status.is_stoppable() && item.can_stop, |status| {
                status.group_hover(group_name.clone(), |style| style.invisible())
            })
            .child(rendered_work_status_icon(
                item.status,
                12.0,
                work_status_color(item.status, *theme),
            ))
    });
    let stop = (item.status.is_stoppable() && item.can_stop).then(|| {
        let click_key = item.key.clone();
        let click_weak = weak.clone();
        let key_key = item.key.clone();
        let key_weak = weak.clone();
        div()
            .id(SharedString::from(format!(
                "background-summary-stop-{}-{}",
                item.key.provider_id, item.key.kind as u8
            )))
            .track_focus(&entry.stop_focus)
            .tab_index(0)
            .size(px(24.0))
            .rounded(px(6.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .opacity(0.0)
            .group_hover(group_name.clone(), |style| style.opacity(1.0))
            .hover(|style| style.bg(theme.overlay_strong))
            .focus_visible(|style| {
                style
                    .opacity(1.0)
                    .bg(theme.raised)
                    .border_1()
                    .border_color(theme.accent)
            })
            .tooltip(Tooltip::text(tr!("background.stop")))
            .child(icon("stop.fill", 12.0, theme.text_tertiary))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let _ = click_weak.update(cx, |this, cx| {
                    this.stop_background_work(session_id, click_key.clone(), cx);
                });
            })
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    let _ = key_weak.update(cx, |this, cx| {
                        this.stop_background_work(session_id, key_key.clone(), cx);
                    });
                    cx.stop_propagation();
                }
            })
    });
    let trailing = (status.is_some() || stop.is_some()).then(|| {
        div()
            .relative()
            .size(px(24.0))
            .flex_none()
            .children(status)
            .children(stop)
    });
    let is_process = item.key.kind != BackgroundWorkKind::Subagent;
    let open_key = item.key.clone();
    let key_key = open_key.clone();
    let click_handle = handle.clone();
    let click_weak = weak.clone();
    let key_handle = handle;
    let key_weak = weak;
    div()
        .id(SharedString::from(format!(
            "background-summary-row-{}-{}",
            item.key.provider_id, item.key.kind as u8
        )))
        .group(group_name)
        .track_focus(&entry.row_focus)
        .tab_index(0)
        .h(px(32.0))
        .w_full()
        .px(px(8.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .gap(px(9.0))
        .cursor_default()
        .focus_visible(|style| style.border_1().border_color(theme.accent))
        .hover(|style| style.bg(theme.overlay_strong))
        .child(icon(
            work_kind_icon(item.key.kind),
            14.0,
            theme.text_secondary,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(px(if is_process { 12.5 } else { 13.5 }))
                .text_color(if is_process {
                    theme.text_secondary
                } else {
                    theme.text
                })
                .child(single_line_label(&item.title)),
        )
        .children(trailing)
        .on_click(move |_, window, cx| {
            click_handle.close(window, cx);
            window.refresh();
            let _ = click_weak.update(cx, |this, cx| {
                this.open_background_work_surface(session_id, open_key.clone(), cx);
            });
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key_handle.close(window, cx);
                window.refresh();
                let _ = key_weak.update(cx, |this, cx| {
                    this.open_background_work_surface(session_id, key_key.clone(), cx);
                });
                cx.stop_propagation();
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_project_name_uses_the_chat_project_and_omits_projectless_tasks() {
        let root = Path::new("/tmp/.michelle/projects");
        let mut project = Project::from_path(PathBuf::from("/tmp/dev/dragon-game"));
        project.name = "Dragon Game".into();
        let projectless = Project::from_path(root.join("2026-10-07/task"));
        let mut session = AgentSession::new(project.id, ProviderKind::Codex);
        let projects = [project, projectless];

        assert_eq!(
            toolbar_project_name(Some(&session), &projects, &[], Some(root)),
            Some("Dragon Game".into())
        );
        let groups = [SidebarProjectGroup {
            id: Uuid::new_v4(),
            name: "OSS".into(),
            projects: projects.iter().map(|project| project.id).collect(),
            collapsed: true,
        }];
        assert_eq!(
            toolbar_project_name(Some(&session), &projects, &groups, Some(root)),
            Some("OSS • Dragon Game".into())
        );
        session.project_id = projects[1].id;
        assert_eq!(
            toolbar_project_name(Some(&session), &projects, &groups, Some(root)),
            None
        );
        session.project_id = Uuid::nil();
        assert_eq!(
            toolbar_project_name(Some(&session), &projects, &groups, Some(root)),
            None
        );
        assert_eq!(
            toolbar_project_name(None, &projects, &groups, Some(root)),
            None
        );
    }

    #[test]
    fn files_tab_uses_the_selected_file_name_and_icon() {
        let files = RightPanelSurface::Files;
        assert_eq!(right_panel_tab_label(&files, None), "Files");
        assert_eq!(
            right_panel_tab_label(&files, Some("packages/desktop/bun.lock")),
            "bun.lock"
        );
        assert_eq!(
            right_panel_tab_icon(&files, Some("packages/desktop/bun.lock")),
            "icons/file-types/bun.svg"
        );

        let file = RightPanelSurface::File("src/main.rs".into());
        assert_eq!(right_panel_tab_label(&file, None), "main.rs");
        assert_eq!(
            right_panel_tab_icon(&file, None),
            "icons/file-types/rust.svg"
        );
    }

    #[test]
    fn right_panel_tab_titles_stay_on_one_line() {
        let source = include_str!("toolbar.rs");
        let header = source
            .split_once("\n    pub(super) fn render_toolbar_right_panel(")
            .expect("right panel header renderer")
            .1
            .split_once("\n    fn render_environment_controls(")
            .expect("right panel header renderer end")
            .0;

        assert!(header.contains(".truncate()"));
        assert!(!header.contains(".line_clamp(1)"));

        let background = RightPanelSurface::BackgroundWork {
            key: BackgroundWorkKey::new(BackgroundWorkKind::Process, "process-1"),
            title: "node -e '\n  const value = 1'".into(),
        };
        assert_eq!(
            right_panel_tab_label(&background, None),
            "node -e ' const value = 1'"
        );
    }

    #[test]
    fn tab_scroll_fades_only_show_toward_hidden_content() {
        assert_eq!(
            tab_scroll_fade_visibility(px(0.0), px(120.0)),
            (false, true)
        );
        assert_eq!(
            tab_scroll_fade_visibility(px(-40.0), px(120.0)),
            (true, true)
        );
        assert_eq!(
            tab_scroll_fade_visibility(px(-120.0), px(120.0)),
            (true, false)
        );
        assert_eq!(tab_scroll_fade_visibility(px(0.0), px(0.0)), (false, false));
    }

    #[test]
    fn selected_tab_offset_clears_fade_overlays() {
        assert_eq!(
            fade_safe_tab_offset(
                px(-100.0),
                px(300.0),
                px(90.0),
                px(190.0),
                px(0.0),
                px(300.0),
            ),
            px(-66.0)
        );
        assert_eq!(
            fade_safe_tab_offset(
                px(-100.0),
                px(324.0),
                px(300.0),
                px(400.0),
                px(0.0),
                px(300.0),
            ),
            px(-124.0)
        );
        assert_eq!(
            fade_safe_tab_offset(px(0.0), px(0.0), px(0.0), px(100.0), px(0.0), px(300.0),),
            px(0.0)
        );
    }

    #[test]
    fn info_popover_uses_distinct_process_status_icons() {
        assert_eq!(
            background_summary_process_status_icon(
                BackgroundWorkKind::Process,
                BackgroundWorkStatus::Completed,
            ),
            Some("checkmark")
        );
        assert_eq!(
            background_summary_process_status_icon(
                BackgroundWorkKind::Monitor,
                BackgroundWorkStatus::Failed,
            ),
            Some("xmark")
        );
        assert_eq!(
            background_summary_process_status_icon(
                BackgroundWorkKind::Process,
                BackgroundWorkStatus::Running,
            ),
            Some("arrow.clockwise")
        );
        assert_eq!(
            background_summary_process_status_icon(
                BackgroundWorkKind::Subagent,
                BackgroundWorkStatus::Completed,
            ),
            None
        );
    }

    #[test]
    fn info_popover_background_titles_stay_on_one_line() {
        let source = include_str!("toolbar.rs");
        let row = source
            .split_once("\nfn render_background_summary_row(")
            .expect("background summary row renderer")
            .1
            .split_once("\n#[cfg(test)]")
            .expect("background summary row renderer end")
            .0;

        assert!(row.contains(".truncate()"));
        assert!(row.contains(".child(single_line_label(&item.title))"));
        assert!(!row.contains(".line_clamp(1)"));
        assert_eq!(
            single_line_label("/bin/zsh -lc 'set -euo pipefail\n  for n in one two'"),
            "/bin/zsh -lc 'set -euo pipefail for n in one two'"
        );
    }

    #[test]
    fn info_popover_uses_michelle_task_and_native_agent_ids() {
        let task_id = Uuid::parse_str("ed28ee51-43cf-4a83-a52f-04c509ca2c09").unwrap();
        let mut session = AgentSession::new(Uuid::nil(), ProviderKind::Codex);
        session.id = task_id;
        session.provider_cursor = Some(ProviderResumeCursor::Codex {
            thread_id: "019cfd7a-6942-78b1-9d47-30576c562321".into(),
        });

        assert_eq!(
            TaskIdentifiers::from(&session),
            TaskIdentifiers {
                task_id,
                agent_cli_thread_id: Some("019cfd7a-6942-78b1-9d47-30576c562321".into()),
            }
        );
    }
}
