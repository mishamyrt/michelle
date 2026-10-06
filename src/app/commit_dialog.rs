//! Modal Git commit/push flow opened from the Environment summary.
//!
//! Git inspection, mutation, and one-shot agent CLI generation all run on the
//! background executor. UI surfaces only paint the cached state below.

use gpui::{KeyBinding, actions};

use super::*;

pub(super) mod model;
use model::{CommitAction, CommitOperationState, CommitPending, commit_pending_status_label};

actions!(
    michelle_commit_dialog,
    [ConfirmCommitDialog, DismissCommitDialog]
);

const DIALOG_CONTEXT: &str = "CommitDialog";
const DIALOG_INPUT_CONTEXT: &str = "CommitDialog > TextInput";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "secondary-enter",
            ConfirmCommitDialog,
            Some(DIALOG_INPUT_CONTEXT),
        ),
        KeyBinding::new("secondary-enter", ConfirmCommitDialog, Some(DIALOG_CONTEXT)),
        KeyBinding::new("escape", DismissCommitDialog, Some(DIALOG_CONTEXT)),
    ]);
}

pub(super) struct CommitDialogState {
    id: Uuid,
    message: Entity<TextInput>,
    include_unstaged: bool,
    include_focus: FocusHandle,
    commit_focus: FocusHandle,
    commit_push_focus: FocusHandle,
    push_focus: FocusHandle,
}

impl Michelle {
    pub(super) fn commit_operation_status_label(&self) -> Option<String> {
        self.commit
            .operation
            .as_ref()
            .map(CommitOperationState::status_label)
    }

    pub(super) fn open_commit_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.prepare_commit_draft(cx) else {
            return;
        };
        let message = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line()
                .placeholder(tr!("commit.message_placeholder"))
        });
        let message_focus = message.read(cx).focus();
        self.commit_dialog = Some(CommitDialogState {
            id,
            message,
            include_unstaged: true,
            include_focus: cx.focus_handle(),
            commit_focus: cx.focus_handle(),
            commit_push_focus: cx.focus_handle(),
            push_focus: cx.focus_handle(),
        });
        // Like Michelle's other deferred surfaces, the modal joins the dispatch
        // tree only after it has drawn. Focus it two frames later so typing
        // cannot fall through to the composer beneath it.
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| window.focus(&message_focus, cx));
        });
        cx.notify();

        self.load_commit_snapshot(id, cx);
    }

    fn close_commit_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.commit_dialog.take().is_none() {
            return;
        }
        self.commit.dismiss_draft();
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn toggle_include_unstaged(&mut self, cx: &mut Context<Self>) {
        if self.commit.operation.is_some() {
            return;
        }
        let Some(dialog) = self.commit_dialog.as_mut() else {
            return;
        };
        dialog.include_unstaged = !dialog.include_unstaged;
        self.commit.clear_error();
        cx.notify();
    }

    fn request_commit_action(
        &mut self,
        action: CommitAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.commit_dialog.as_ref() else {
            return;
        };
        let message = dialog.message.read(cx).content().trim().to_owned();
        let include_unstaged = dialog.include_unstaged;
        self.perform_commit_action(
            action,
            message,
            include_unstaged,
            window.window_handle(),
            cx,
        );
    }

    pub(super) fn render_commit_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.commit_dialog.as_ref()?;
        let draft = self.commit.draft.as_ref()?;
        let theme = Theme::current(cx);
        let branch = draft.snapshot.branch.clone();
        let message = dialog.message.clone();
        let include_unstaged = dialog.include_unstaged;
        let pending = self
            .commit
            .operation
            .as_ref()
            .filter(|operation| operation.id == dialog.id)
            .map(|operation| operation.pending);
        let include_enabled = pending.is_none();
        let (additions, deletions) = draft.displayed_counts(include_unstaged);
        let can_commit = pending.is_none() && draft.can_commit(include_unstaged);
        let can_push = pending.is_none() && draft.can_push();
        let pending_status = pending.map(commit_pending_status_label);
        let error = draft.error.clone();
        let weak = cx.entity().downgrade();

        let include = {
            let click_weak = weak.clone();
            let key_weak = weak.clone();
            div()
                .id("commit-dialog-include-unstaged")
                .track_focus(&dialog.include_focus)
                .when(include_enabled, |row| row.tab_index(0))
                .h(px(44.0))
                .w_full()
                .px(px(12.0))
                .rounded(px(9.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .cursor_default()
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .when(include_enabled, |row| {
                    row.hover(|style| style.bg(theme.overlay))
                })
                .child(
                    div()
                        .size(px(15.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(if include_unstaged {
                            theme.border_strong
                        } else {
                            theme.text_ghost
                        })
                        .bg(if include_unstaged {
                            theme.composer
                        } else {
                            gpui::transparent_black()
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(include_unstaged, |checkbox| {
                            checkbox.child(icon("checkmark", 12.0, theme.text))
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_size(sp(14.0))
                        .text_color(if include_enabled {
                            theme.text
                        } else {
                            theme.text_ghost
                        })
                        .child(tr!("commit.include_unstaged")),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_size(sp(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .child(
                            div()
                                .text_color(theme.success)
                                .child(format!("+{}", grouped_number(additions))),
                        )
                        .child(
                            div()
                                .text_color(theme.danger)
                                .child(format!("-{}", grouped_number(deletions))),
                        ),
                )
                .when(include_enabled, |row| {
                    row.on_click(move |_, _, cx| {
                        let _ = click_weak
                            .update(cx, |michelle, cx| michelle.toggle_include_unstaged(cx));
                    })
                    .on_key_down(move |event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            let _ = key_weak
                                .update(cx, |michelle, cx| michelle.toggle_include_unstaged(cx));
                            cx.stop_propagation();
                        }
                    })
                })
        };

        let commit_active = pending.is_some_and(|pending| match pending {
            CommitPending::Generating(action) | CommitPending::Git(action) => {
                action == CommitAction::Commit
            }
        });
        let commit = render_commit_action_row(
            "commit-dialog-commit",
            &dialog.commit_focus,
            "point.topleft.down.to.point.bottomright.curvepath",
            if commit_active {
                pending_status
                    .clone()
                    .unwrap_or_else(|| tr!("commit.commit"))
            } else {
                tr!("commit.commit")
            },
            can_commit,
            commit_active,
            Some(crate::platform::primary_shortcut("⌘↩", "Ctrl+Enter")),
            CommitAction::Commit,
            weak.clone(),
            &theme,
        );
        let commit_and_push_active = pending.is_some_and(|pending| match pending {
            CommitPending::Generating(action) | CommitPending::Git(action) => {
                action == CommitAction::CommitAndPush
            }
        });
        let commit_and_push = render_commit_action_row(
            "commit-dialog-commit-and-push",
            &dialog.commit_push_focus,
            "icloud.and.arrow.up",
            if commit_and_push_active {
                pending_status
                    .clone()
                    .unwrap_or_else(|| tr!("commit.commit_and_push"))
            } else {
                tr!("commit.commit_and_push")
            },
            can_commit,
            commit_and_push_active,
            None,
            CommitAction::CommitAndPush,
            weak.clone(),
            &theme,
        );
        let push_active = pending == Some(CommitPending::Git(CommitAction::Push));
        let push = render_commit_action_row(
            "commit-dialog-push",
            &dialog.push_focus,
            "icloud.and.arrow.up",
            if push_active {
                pending_status.unwrap_or_else(|| tr!("commit.push"))
            } else {
                tr!("commit.push")
            },
            can_push,
            push_active,
            None,
            CommitAction::Push,
            weak,
            &theme,
        );

        let card = div()
            .id("commit-dialog-card")
            .key_context(DIALOG_CONTEXT)
            .on_action(
                cx.listener(|michelle, _: &ConfirmCommitDialog, window, cx| {
                    michelle.request_commit_action(CommitAction::Commit, window, cx)
                }),
            )
            .on_action(
                cx.listener(|michelle, _: &DismissCommitDialog, window, cx| {
                    michelle.close_commit_dialog(window, cx)
                }),
            )
            .tab_group()
            .tab_stop(false)
            .w_full()
            .max_w(px(420.0))
            .overflow_hidden()
            .rounded(px(18.0))
            .bg(theme.composer)
            .shadow_xl()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .h(px(48.0))
                    .px(px(16.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .text_size(sp(14.0))
                    .text_color(theme.text)
                    .child(icon("arrow.triangle.branch", 15.0, theme.text))
                    .child(div().min_w_0().truncate().child(branch)),
            )
            .child(
                div()
                    .h(px(112.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .text_size(sp(14.0))
                    .line_height(sp(21.0))
                    .text_color(theme.text)
                    .child(message),
            )
            .child(div().px(px(8.0)).child(include))
            .when_some(error, |card, error| {
                card.child(
                    div()
                        .px(px(20.0))
                        .pb(px(10.0))
                        .text_size(sp(12.5))
                        .line_height(sp(16.0))
                        .text_color(theme.danger)
                        .child(error),
                )
            })
            .child(div().mx(px(8.0)).h(px(1.0)).bg(theme.border))
            .child(
                div()
                    .p(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(commit)
                    .child(commit_and_push)
                    .child(push),
            );

        let scrim = if theme.is_dark {
            gpui::hsla(0.0, 0.0, 0.0, 0.34)
        } else {
            gpui::hsla(0.0, 0.0, 0.0, 0.16)
        };
        let layer = div()
            .id("commit-dialog-layer")
            .absolute()
            .inset_0()
            .occlude()
            .bg(scrim)
            .p(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|michelle, _, window, cx| michelle.close_commit_dialog(window, cx)),
            )
            .child(card);
        Some(gpui::deferred(layer).with_priority(4).into_any_element())
    }
}

#[allow(clippy::too_many_arguments)]
fn render_commit_action_row(
    id: &'static str,
    focus: &FocusHandle,
    icon_path: &'static str,
    label: String,
    enabled: bool,
    active: bool,
    shortcut: Option<&'static str>,
    action: CommitAction,
    weak: WeakEntity<Michelle>,
    theme: &Theme,
) -> Stateful<Div> {
    let foreground = if enabled {
        theme.text
    } else if active {
        theme.text_secondary
    } else {
        theme.text_ghost
    };
    let indicator = if active {
        motion::spinner(15.0, theme.text_secondary)
    } else {
        icon(icon_path, 15.0, foreground).into_any_element()
    };
    let click_weak = weak.clone();
    let key_weak = weak;
    div()
        .id(id)
        .track_focus(focus)
        .when(enabled, |row| row.tab_index(0))
        .h(px(38.0))
        .w_full()
        .px(px(10.0))
        .rounded(px(9.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .cursor_default()
        .text_size(sp(14.0))
        .text_color(foreground)
        .focus_visible(|style| style.border_1().border_color(theme.accent))
        .when(enabled, |row| {
            row.hover(|style| style.bg(theme.overlay_strong))
        })
        .child(indicator)
        .child(div().min_w_0().flex_1().truncate().child(label))
        .when_some(shortcut, |row, shortcut| {
            row.child(
                div()
                    .h(px(22.0))
                    .min_w(px(34.0))
                    .px(px(7.0))
                    .rounded(px(11.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.overlay_strong)
                    .text_size(sp(12.5))
                    .text_color(if enabled {
                        theme.text_secondary
                    } else {
                        theme.text_ghost
                    })
                    .child(shortcut),
            )
        })
        .when(enabled, |row| {
            row.on_click(move |_, window, cx| {
                let _ = click_weak.update(cx, |michelle, cx| {
                    michelle.request_commit_action(action, window, cx)
                });
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if !event.keystroke.modifiers.modified()
                    && matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    let _ = key_weak.update(cx, |michelle, cx| {
                        michelle.request_commit_action(action, window, cx)
                    });
                    cx.stop_propagation();
                }
            })
        })
}

fn grouped_number(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_change_counts() {
        assert_eq!(grouped_number(0), "0");
        assert_eq!(grouped_number(2_849), "2,849");
        assert_eq!(grouped_number(1_234_567), "1,234,567");
    }
}
