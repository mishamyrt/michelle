//! Modal Git commit/push flow opened from the Environment summary.
//!
//! Git inspection, mutation, and one-shot agent CLI generation all run on the
//! background executor. UI surfaces only paint the cached state below.

use gpui::{KeyBinding, actions};

use super::*;
use crate::ui::{
    ActionRow, ActivationExt, Checkbox, Dialog, KeyHint, TextArea, focus_dialog_input,
};

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
        focus_dialog_input(window, cx, message_focus, move |this, _| {
            this.commit_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.id == id)
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

        let include = Checkbox::new(
            "commit-dialog-include-unstaged",
            include_unstaged,
            tr!("commit.include_unstaged"),
        )
        .track_focus(&dialog.include_focus)
        .checked_background(theme.composer)
        .disabled(!include_enabled)
        .on_activation(cx, |this, _, cx| this.toggle_include_unstaged(cx))
        .trailing(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.5))
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
        );

        let commit_active = pending.is_some_and(|pending| match pending {
            CommitPending::Generating(action) | CommitPending::Git(action) => {
                action == CommitAction::Commit
            }
        });
        let commit = ActionRow::new(
            "commit-dialog-commit",
            if commit_active {
                pending_status
                    .clone()
                    .unwrap_or_else(|| tr!("commit.commit"))
            } else {
                tr!("commit.commit")
            },
        )
        .track_focus(&dialog.commit_focus)
        .icon("point.topleft.down.to.point.bottomright.curvepath")
        .disabled(!can_commit)
        .loading(commit_active)
        .trailing(
            KeyHint::new(crate::platform::primary_shortcut("⌘↩", "Ctrl+Enter")).text_color(
                if can_commit {
                    theme.text_secondary
                } else {
                    theme.text_ghost
                },
            ),
        )
        .on_activation(cx, |this, window, cx| {
            this.request_commit_action(CommitAction::Commit, window, cx)
        });
        let commit_and_push_active = pending.is_some_and(|pending| match pending {
            CommitPending::Generating(action) | CommitPending::Git(action) => {
                action == CommitAction::CommitAndPush
            }
        });
        let commit_and_push = ActionRow::new(
            "commit-dialog-commit-and-push",
            if commit_and_push_active {
                pending_status
                    .clone()
                    .unwrap_or_else(|| tr!("commit.commit_and_push"))
            } else {
                tr!("commit.commit_and_push")
            },
        )
        .track_focus(&dialog.commit_push_focus)
        .icon("icloud.and.arrow.up")
        .disabled(!can_commit)
        .loading(commit_and_push_active)
        .on_activation(cx, |this, window, cx| {
            this.request_commit_action(CommitAction::CommitAndPush, window, cx)
        });
        let push_active = pending == Some(CommitPending::Git(CommitAction::Push));
        let push = ActionRow::new(
            "commit-dialog-push",
            if push_active {
                pending_status.unwrap_or_else(|| tr!("commit.push"))
            } else {
                tr!("commit.push")
            },
        )
        .track_focus(&dialog.push_focus)
        .icon("icloud.and.arrow.up")
        .disabled(!can_push)
        .loading(push_active)
        .on_activation(cx, |this, window, cx| {
            this.request_commit_action(CommitAction::Push, window, cx)
        });

        let card = Dialog::new("commit-dialog-layer", "commit-dialog-card")
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
            .bg(theme.composer)
            .on_dismiss(cx, |this, window, cx| this.close_commit_dialog(window, cx))
            .header(
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
                TextArea::new("commit-dialog-message", message)
                    .text_size(px(self.state.content_font_size))
                    .line_height(px(self.state.content_font_size * 1.5)),
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
            .actions(
                div()
                    .p(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(commit)
                    .child(commit_and_push)
                    .child(push),
            );

        Some(
            card.scrim(gpui::hsla(
                0.0,
                0.0,
                0.0,
                if theme.is_dark { 0.34 } else { 0.16 },
            ))
            .into_any_element(),
        )
    }
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
