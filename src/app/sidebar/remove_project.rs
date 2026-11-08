use super::*;
use crate::ui::{ButtonRole, Dialog, focus_dialog_input};

pub(in crate::app) struct RemoveProjectDialog {
    project_id: Uuid,
    project_name: String,
    cancel_focus: FocusHandle,
    remove_focus: FocusHandle,
}

impl Michelle {
    pub(super) fn open_remove_project_dialog(
        &mut self,
        project_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self
            .state
            .projects
            .iter()
            .find(|project| project.id == project_id && !project.is_projectless())
        else {
            return;
        };
        let cancel_focus = cx.focus_handle();
        self.sidebar_ui.remove_project_dialog = Some(RemoveProjectDialog {
            project_id,
            project_name: project.name.clone(),
            cancel_focus: cancel_focus.clone(),
            remove_focus: cx.focus_handle(),
        });
        let current_focus = cancel_focus.clone();
        focus_dialog_input(window, cx, cancel_focus, move |this, _| {
            this.sidebar_ui
                .remove_project_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.cancel_focus == current_focus)
        });
        cx.notify();
    }

    fn close_remove_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.sidebar_ui.remove_project_dialog.take() else {
            return;
        };
        self.focus_sidebar_row(
            SidebarRow::Header(SidebarGroup::Project(dialog.project_id)),
            window,
            cx,
        );
        cx.notify();
    }

    fn confirm_remove_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.sidebar_ui.remove_project_dialog.as_ref() else {
            return;
        };
        let project_id = dialog.project_id;
        let collection = self.state.sidebar_group_for_project(project_id);
        if self.remove_sidebar_project(project_id, cx) {
            self.sidebar_ui.remove_project_dialog = None;
            self.focus_sidebar_row(
                SidebarRow::Collection(
                    collection.map_or(SidebarCollection::Projects, SidebarCollection::Group),
                ),
                window,
                cx,
            );
        }
    }

    pub(in crate::app) fn render_remove_project_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.sidebar_ui.remove_project_dialog.as_ref()?;
        let theme = Theme::current(cx);
        Some(
            Dialog::new("remove-project-dialog-layer", "remove-project-dialog-card")
                .key_context("RemoveProjectDialog")
                .on_action(cx.listener(|this, _: &DismissProjectRemoval, window, cx| {
                    this.close_remove_project_dialog(window, cx);
                }))
                .w_full()
                .bg(theme.composer)
                .scrim(gpui::hsla(
                    0.0,
                    0.0,
                    0.0,
                    if theme.is_dark { 0.34 } else { 0.16 },
                ))
                .on_dismiss(cx, |this, window, cx| {
                    this.close_remove_project_dialog(window, cx)
                })
                .header(
                    div()
                        .px(px(20.0))
                        .pt(px(20.0))
                        .pb(px(8.0))
                        .text_style(TextStyle::HEADLINE)
                        .text_color(theme.text)
                        .child(tr!(
                            "sidebar.remove_project_title",
                            name = &dialog.project_name
                        )),
                )
                .child(
                    div()
                        .px(px(20.0))
                        .pb(px(20.0))
                        .text_style(TextStyle::BODY)
                        .text_color(theme.text_secondary)
                        .child(tr!("sidebar.remove_project_description")),
                )
                .actions(
                    div()
                        .p(px(12.0))
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            Button::new("remove-project-cancel", tr!("common.cancel"))
                                .track_focus(&dialog.cancel_focus)
                                .on_activation(cx, |this, window, cx| {
                                    this.close_remove_project_dialog(window, cx)
                                }),
                        )
                        .child(
                            Button::new("remove-project-confirm", tr!("common.remove"))
                                .track_focus(&dialog.remove_focus)
                                .role(ButtonRole::Destructive)
                                .on_activation(cx, |this, window, cx| {
                                    this.confirm_remove_project_dialog(window, cx)
                                }),
                        ),
                )
                .into_any_element(),
        )
    }
}
