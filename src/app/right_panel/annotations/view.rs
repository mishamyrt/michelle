use super::*;
use crate::ui::{Button, ControlSize, TextArea};
use gpui::{ClickEvent, ElementId};

impl Michelle {
    pub(in crate::app::right_panel) fn render_review_entry(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = self.menu_handle("review-entry", cx);
        let weak = cx.entity().downgrade();
        let can_start = !self.right_panel_model.diff_loading
            && self
                .right_panel_model
                .diff_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.data.is_some() && !snapshot.files.is_empty());
        dropdown_menu(
            MenuChip::new("review-entry")
                .label(tr!("review.title"))
                .height(px(28.0))
                .selected(handle.is_open()),
            "review-entry-menu",
            &handle,
            MenuAlign::BelowLeft,
            move |_| {
                let start = weak.clone();
                let resume = weak.clone();
                vec![
                    MenuItem::new(tr!("review.start"), move |_, cx| {
                        let _ = start.update(cx, |this, cx| this.start_review(cx));
                    })
                    .disabled(!can_start),
                    MenuItem::new(tr!("review.resume"), move |_, cx| {
                        let _ = resume.update(cx, |this, cx| this.resume_review(cx));
                    }),
                ]
            },
        )
    }

    pub(in crate::app::right_panel) fn review_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        disabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        let id = id.into();
        let focus = self.review_control_focus(format!("review-{id:?}"), cx);
        Button::new(id, label)
            .control_size(ControlSize::Small)
            .track_focus(&focus)
            .disabled(disabled)
            .on_activation(cx, action)
    }

    pub(in crate::app::right_panel) fn render_review_controls(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let review = self.active_review()?;
        let theme = Theme::current(cx);
        let open = review.index.open;
        let target = match &review.saved.data.source {
            ReviewDiffSource::CompareBranches { target, .. }
                if self
                    .review_branch_snapshot()
                    .and_then(|snapshot| snapshot.current.as_deref())
                    != Some(target) =>
            {
                Some(target.clone())
            }
            _ => None,
        };
        let bulk = review.bulk.as_ref();
        let row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.0))
            .child(self.review_button(
                "review-comment",
                tr!("review.add"),
                self.right_panel_model.review_selection.is_none(),
                cx,
                |this, window, cx| this.edit_review_selection(window, cx),
            ))
            .child(self.review_button(
                "review-agent",
                tr!("review.run_codex"),
                review.request_pending,
                cx,
                |this, _, cx| this.request_agent_review(None, cx),
            ))
            .child(self.review_button(
                "review-bulk",
                tr!("review.fix_all", count = open),
                open == 0,
                cx,
                |this, _, cx| this.toggle_review_bulk(cx),
            ))
            .child(self.review_button(
                "review-close",
                tr!("review.close"),
                false,
                cx,
                |this, _, cx| {
                    this.close_review(cx);
                    this.refresh_right_panel_diff(cx);
                },
            ));
        let result = if review.request_pending {
            Some(tr!("review.working"))
        } else if review.saved.agent_finished && !review.index.has_agent_findings {
            Some(tr!("review.no_findings"))
        } else {
            None
        };
        Some(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .px(px(12.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(theme.border)
                .child(row)
                .when_some(target, |row, target| {
                    let label = tr!("review.checkout", branch = target);
                    row.child(self.review_button(
                        "review-checkout-target",
                        label,
                        self.branches.operation_pending,
                        cx,
                        move |this, _, cx| {
                            if !this.choose_workspace_branch(target.clone(), cx) {
                                this.show_toast(tr!("review.wait_for_workspace"));
                                cx.notify();
                            }
                        },
                    ))
                })
                .when_some(result, |row, result| {
                    row.child(div().text_color(theme.text_secondary).child(result))
                })
                .when_some(bulk, |row, selected| {
                    row.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(sp(12.0))
                                    .child(tr!("review.bulk_selection", count = selected.len())),
                            )
                            .child(self.review_button(
                                "review-bulk-send",
                                tr!("review.send_fixes"),
                                selected.is_empty(),
                                cx,
                                |this, _, cx| this.submit_review_fixes(None, cx),
                            )),
                    )
                }),
        )
    }

    pub(in crate::app::right_panel) fn render_review_summary(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let review = self.active_review()?;
        if review.index.summary.is_empty() {
            return None;
        }
        Some(
            div().max_h(px(160.0)).child(
                div()
                    .id("review-summary")
                    .max_h(px(160.0))
                    .overflow_y_scroll()
                    .children(review.index.summary.iter().map(|&index| {
                        self.render_discussion(&review.saved.discussions[index], cx)
                    })),
            ),
        )
    }

    pub(in crate::app::right_panel) fn render_review_line_selector(
        &self,
        file: &crate::review_diff::File,
        line: &crate::review_diff::Line,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.active_review()?;
        let number = line.new_line.or(line.old_line)?;
        let theme = Theme::current(cx);
        let focus = self.review_control_focus(format!("review-line-{}-{index}", file.path), cx);
        let selected = self
            .right_panel_model
            .review_selection
            .is_some_and(|(a, b)| (a.min(b)..=a.max(b)).contains(&index));
        Some(
            div()
                .id(("review-line", index))
                .track_focus(&focus)
                .tab_index(0)
                .tab_stop(true)
                .min_w(px(35.0))
                .px(px(4.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .when(selected, |button| {
                    button.bg(theme.selection).font_weight(FontWeight::SEMIBOLD)
                })
                .hover(|style| style.bg(theme.overlay_strong))
                .focus_visible(|style| {
                    style
                        .bg(theme.overlay_strong)
                        .border_1()
                        .border_color(theme.accent)
                })
                .tooltip(Tooltip::text(tr!("review.select_line", line = number)))
                .child(number.to_string())
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    this.select_review_line(index, event.modifiers().shift, cx);
                    cx.stop_propagation();
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.select_review_line(index, event.keystroke.modifiers.shift, cx);
                        if event.keystroke.key == "enter" {
                            this.edit_review_selection(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .into_any_element(),
        )
    }

    fn render_review_editor(&self, editor: &ReviewEditor, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .p(px(8.0))
            .child(
                TextArea::new("review-comment-editor", editor.input.clone())
                    .h_auto()
                    .min_h(px(54.0)),
            )
            .child(self.review_button(
                "review-save-draft",
                tr!("review.save_draft"),
                false,
                cx,
                |this, _, cx| this.save_review_editor(cx),
            ))
    }

    pub(in crate::app::right_panel) fn render_review_line_discussions(
        &self,
        code: AnyElement,
        file: &crate::review_diff::File,
        line: &crate::review_diff::Line,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(review) = self.active_review() else {
            return code;
        };
        let matches_row = |anchor: &Anchor| {
            anchor.path == file.path && anchor.side.number(line) == Some(anchor.end)
        };
        let editor = self
            .right_panel_model
            .review_editor
            .as_ref()
            .filter(|editor| {
                editor.discussion.is_none() && editor.anchor.as_ref().is_some_and(matches_row)
            });
        let lines = review.index.lines.get(&file.path);
        let discussions = [(Side::Old, line.old_line), (Side::New, line.new_line)]
            .into_iter()
            .filter_map(|(side, number)| lines?.get(&(side, number?)))
            .flatten()
            .map(|&i| &review.saved.discussions[i])
            .collect::<Vec<_>>();
        if discussions.is_empty() && editor.is_none() {
            return code;
        }
        div()
            .flex()
            .flex_col()
            .child(code)
            .children(
                discussions
                    .into_iter()
                    .map(|item| self.render_discussion(item, cx)),
            )
            .when_some(editor, |row, editor| {
                row.child(self.render_review_editor(editor, cx))
            })
            .into_any_element()
    }

    fn render_discussion(&self, thread: &Discussion, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let id = thread.id;
        let editor = self
            .right_panel_model
            .review_editor
            .as_ref()
            .filter(|editor| editor.discussion == Some(id));
        let pending = self
            .active_review()
            .is_some_and(|review| review.request_pending);
        let status = if thread.dismissed {
            Some(tr!("review.dismissed"))
        } else if thread.fix_turn.is_some() {
            Some(tr!("review.fix_sent"))
        } else if thread.draft {
            Some(tr!("review.draft"))
        } else {
            None
        };
        let bulk = self.active_review().and_then(|review| review.bulk.as_ref());
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(px(4.0))
            .child(self.review_button(
                SharedString::from(format!("review-reply-{id}")),
                tr!("review.reply"),
                editor.is_some() || thread.dismissed,
                cx,
                move |this, window, cx| this.edit_existing_discussion(id, window, cx),
            ))
            .when(thread.draft && !thread.dismissed, |row| {
                row.child(self.review_button(
                    SharedString::from(format!("review-ask-{id}")),
                    tr!("review.ask_codex"),
                    pending,
                    cx,
                    move |this, _, cx| this.request_agent_review(Some(id), cx),
                ))
                .child(self.review_button(
                    SharedString::from(format!("review-publish-{id}")),
                    tr!("review.publish"),
                    false,
                    cx,
                    move |this, _, cx| {
                        this.update_review_discussion(id, |thread| thread.draft = false, cx)
                    },
                ))
            })
            .when(!thread.dismissed && thread.fix_turn.is_none(), |row| {
                row.child(self.review_button(
                    SharedString::from(format!("review-fix-{id}")),
                    tr!("review.fix"),
                    false,
                    cx,
                    move |this, _, cx| this.submit_review_fixes(Some(id), cx),
                ))
            })
            .child(self.review_button(
                SharedString::from(format!("review-dismiss-{id}")),
                if thread.dismissed {
                    tr!("review.reopen")
                } else {
                    tr!("review.dismiss")
                },
                thread.fix_turn.is_some(),
                cx,
                move |this, _, cx| {
                    this.update_review_discussion(
                        id,
                        |thread| thread.dismissed = !thread.dismissed,
                        cx,
                    )
                },
            ))
            .when_some(thread.fix_turn, |row, turn| {
                row.child(self.review_button(
                    SharedString::from(format!("review-fix-diff-{id}")),
                    tr!("review.fix_diff"),
                    false,
                    cx,
                    move |this, _, cx| this.open_review_fix_diff(turn, cx),
                ))
                .child(self.review_button(
                    SharedString::from(format!("review-retry-{id}")),
                    tr!("review.reopen"),
                    false,
                    cx,
                    move |this, _, cx| {
                        this.update_review_discussion(id, |thread| thread.fix_turn = None, cx)
                    },
                ))
            })
            .when_some(bulk.filter(|_| thread.open()), |row, bulk| {
                row.child(self.review_button(
                    SharedString::from(format!("review-bulk-item-{id}")),
                    if bulk.contains(&id) {
                        tr!("review.included")
                    } else {
                        tr!("review.excluded")
                    },
                    false,
                    cx,
                    move |this, _, cx| this.toggle_review_bulk_item(id, cx),
                ))
            });
        div()
            .mx(px(10.0))
            .my(px(4.0))
            .p(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .flex()
            .flex_col()
            .gap(px(4.0))
            .when_some(status, |row, status| {
                row.child(div().text_color(theme.text_secondary).child(status))
            })
            .when_some(thread.anchor.as_ref(), |row, anchor| {
                row.child(
                    div()
                        .text_size(sp(11.0))
                        .text_color(theme.text_secondary)
                        .child(if anchor.start == anchor.end {
                            anchor.end.to_string()
                        } else {
                            format!("{}–{}", anchor.start, anchor.end)
                        }),
                )
            })
            .children(thread.messages.iter().map(|message| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(sp(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if message.agent {
                                tr!("review.agent")
                            } else {
                                tr!("review.you")
                            }),
                    )
                    .child(div().whitespace_normal().child(message.body.clone()))
            }))
            .child(actions)
            .when_some(editor, |row, editor| {
                row.child(self.render_review_editor(editor, cx))
            })
    }
}
