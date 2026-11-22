//! Presentation and follow-ups for the code review skill's ordinary Markdown.
use super::*;
use crate::model::{ReviewReply, ReviewThreadRef, is_review_request};
use crate::ui::{Button, ButtonStyle, ControlSize, Divider};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

#[derive(Debug)]
struct Finding {
    priority: String,
    title: String,
    location: String,
    body: String,
}

pub(super) struct Report {
    pub(super) summary: String,
    findings: Vec<Finding>,
}

fn parse_report(text: &str) -> Option<Report> {
    if !text.contains("### [P") {
        return None;
    }
    let mut depth = 0;
    let mut headings = Vec::new();
    for (event, range) in Parser::new(text).into_offset_iter() {
        if let Event::Start(tag) = event {
            if depth == 0
                && matches!(
                    tag,
                    Tag::Heading {
                        level: HeadingLevel::H3,
                        ..
                    }
                )
            {
                headings.push(range.start);
            }
            depth += 1;
        } else if matches!(event, Event::End(_)) {
            depth -= 1;
        }
    }
    let first = headings
        .iter()
        .position(|start| text[*start..].starts_with("### [P"))?;
    let summary = text[..headings[first]].trim().to_owned();
    let mut findings = Vec::new();
    for (i, &start) in headings.iter().enumerate().skip(first) {
        let section = &text[start..headings.get(i + 1).copied().unwrap_or(text.len())];
        let (heading, rest) = section.split_once('\n')?;
        let (priority, title) = heading.strip_prefix("### [")?.split_once("] ")?;
        if !matches!(priority, "P0" | "P1" | "P2" | "P3") || title.trim().is_empty() {
            return None;
        }
        let (location, body) = rest.trim_start().split_once('\n')?;
        let location = location.trim().strip_prefix('`')?.strip_suffix('`')?;
        if !valid_location(location) || body.trim().is_empty() {
            return None;
        }
        findings.push(Finding {
            priority: priority.to_owned(),
            title: title.trim().to_owned(),
            location: location.to_owned(),
            body: body.trim().to_owned(),
        });
    }
    Some(Report { summary, findings })
}

fn valid_location(location: &str) -> bool {
    let Some((path, lines)) = location.rsplit_once(':') else {
        return false;
    };
    if path.trim().is_empty() || path.contains('`') {
        return false;
    }
    let (first, last) = lines.split_once('-').unwrap_or((lines, lines));
    matches!((first.parse::<u32>(), last.parse::<u32>()), (Ok(first), Ok(last)) if first > 0 && last >= first)
}

impl Finding {
    fn context(&self) -> String {
        format!("{} ({})\n\n{}", self.title, self.location, self.body)
    }
}

fn chat_review_key(thread: ReviewThreadRef) -> String {
    format!(
        "review-thread-{}-{}",
        thread.message_id, thread.finding_index
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReviewFixRequest {
    Recent,
    Latest,
}

pub(in crate::app) fn is_explicit_review_fix_request(text: &str) -> bool {
    review_fix_request(text) == Some(ReviewFixRequest::Latest)
}

fn review_fix_request(text: &str) -> Option<ReviewFixRequest> {
    if text.len() > 128 {
        return None;
    }
    let text = text.trim().trim_end_matches(['.', '!', '?']).to_lowercase();
    let text = text
        .strip_prefix("пожалуйста, ")
        .or_else(|| text.strip_prefix("please "))
        .unwrap_or(&text);
    let text = text
        .strip_suffix(", пожалуйста")
        .or_else(|| text.strip_suffix(", please"))
        .unwrap_or(text);
    if matches!(
        text,
        "исправляй"
            | "исправь"
            | "исправляй всё"
            | "исправляй все"
            | "исправь всё"
            | "исправь все"
            | "исправь оставшиеся"
            | "исправь открытые"
            | "fix them"
            | "давай исправляй"
            | "можешь исправлять"
            | "fix it"
            | "fix all"
    ) {
        Some(ReviewFixRequest::Recent)
    } else if [
        "исправь все замечания",
        "исправь открытые замечания",
        "исправь оставшиеся замечания",
        "исправляй замечания",
        "исправь замечания ревью",
        "fix the findings",
        "fix the review findings",
        "fix all findings",
        "fix the remaining findings",
    ]
    .iter()
    .any(|phrase| text == *phrase)
    {
        Some(ReviewFixRequest::Latest)
    } else {
        None
    }
}

fn open_findings(
    report: &Report,
    message_id: Uuid,
    dismissed: &HashSet<ReviewThreadRef>,
) -> String {
    report
        .findings
        .iter()
        .enumerate()
        .filter(|(finding_index, _)| {
            !dismissed.contains(&ReviewThreadRef {
                message_id,
                finding_index: *finding_index,
            })
        })
        .map(|(_, finding)| finding.context())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[derive(Default)]
pub(in crate::app) struct ChatReviews {
    reports: RefCell<HashMap<Uuid, (usize, Option<Rc<Report>>)>>,
    markdown: RefCell<HashMap<ReviewThreadRef, MarkdownView>>,
}

impl ChatReviews {
    pub(in crate::app) fn clear(&self) {
        self.reports.borrow_mut().clear();
        self.markdown.borrow_mut().clear();
    }

    pub(super) fn report(&self, message: &Message) -> Option<Rc<Report>> {
        if message.role != MessageRole::Assistant || message.streaming {
            return None;
        }
        let text = message.visible_content();
        let mut reports = self.reports.borrow_mut();
        let (length, report) = reports
            .entry(message.id)
            .or_insert_with(|| (text.len(), parse_report(text).map(Rc::new)));
        if *length != text.len() {
            *length = text.len();
            *report = parse_report(text).map(Rc::new);
        }
        report.clone()
    }

    fn fixes(&self, session: &AgentSession, request: ReviewFixRequest) -> Option<String> {
        let follow_ups: HashSet<_> = session
            .turns
            .iter()
            .filter(|turn| turn.review_thread.is_some())
            .map(|turn| turn.id)
            .collect();
        for message in session.messages.iter().rev() {
            if message.turn_id.is_some_and(|id| follow_ups.contains(&id)) {
                continue;
            }
            if message.role == MessageRole::User {
                if is_review_request(message.visible_content()) {
                    return Some(String::new());
                }
                if request == ReviewFixRequest::Recent {
                    return None;
                }
            }
            if message.role != MessageRole::Assistant
                || !message.visible_content().contains("### [P")
            {
                continue;
            }
            if let Some(report) = self.report(message) {
                return Some(open_findings(
                    &report,
                    message.id,
                    &session.dismissed_review_findings,
                ));
            }
        }
        (request == ReviewFixRequest::Latest).then(String::new)
    }
}

impl Michelle {
    pub(in crate::app) fn chat_review_fixes(
        &self,
        session: &AgentSession,
        submission: &ComposerSubmission,
    ) -> Option<String> {
        if submission.review_reply.is_some() || submission.review_fix.is_some() {
            return None;
        }
        let request = review_fix_request(&submission.human_prompt())?;
        self.transcript_model.chat_reviews.fixes(session, request)
    }

    pub(in crate::app) fn review_control_focus(&self, key: String, cx: &App) -> FocusHandle {
        self.transcript_ui
            .control_focuses
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone()
    }

    fn chat_review_button(
        &self,
        key: String,
        label: String,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        let focus = self.review_control_focus(key.clone(), cx);
        Button::new(SharedString::from(key), label)
            .button_style(ButtonStyle::Ghost)
            .control_size(ControlSize::Mini)
            .px(px(10.0))
            .border_0()
            .text_style(TextStyle::SUBHEADLINE)
            .track_focus(&focus)
            .on_activation(cx, action)
    }

    fn render_chat_review_location(
        &self,
        location: &str,
        key: &str,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = format!("{key}-copy-path");
        let focus = self.review_control_focus(id.clone(), cx);
        let group = SharedString::from(id);
        let location = SharedString::from(location.to_owned());
        div()
            .flex()
            .text_style(TextStyle::SUBHEADLINE)
            .text_color(theme.text_secondary)
            .child(
                div()
                    .id(group.clone())
                    .group(group.clone())
                    .track_focus(&focus)
                    .tab_index(0)
                    .min_w_0()
                    .min_h(px(22.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .focus_visible(|style| style.bg(theme.overlay_strong))
                    .tooltip(Tooltip::text(tr!("common.copy")))
                    .child(div().min_w_0().child(location.clone()))
                    .child(
                        div()
                            .flex_none()
                            .opacity(0.0)
                            .group_hover(group, |style| style.opacity(1.0))
                            .when(
                                focus.is_focused(window) && window.last_input_was_keyboard(),
                                |icon| icon.opacity(1.0),
                            )
                            .child(icon("doc.on.doc", 10.0, theme.text_secondary)),
                    )
                    .on_activation(cx, move |_, _, cx| {
                        let (path, lines) = location
                            .rsplit_once(':')
                            .expect("validated review location");
                        let start = lines.split_once('-').map_or(lines, |(start, _)| start);
                        cx.write_to_clipboard(ClipboardItem::new_string(format!("{path}:{start}")));
                    }),
            )
    }

    fn set_chat_review_expanded(
        &mut self,
        thread: ReviewThreadRef,
        index: usize,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        self.pin_transcript_for_disclosure();
        self.transcript_ui
            .review_findings_expanded
            .insert(thread, expanded);
        self.remeasure_transcript_message(index);
        cx.notify();
    }

    fn toggle_chat_review_dismissed(
        &mut self,
        thread: ReviewThreadRef,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pin_transcript_for_disclosure();
        let Some(session) = self.selected_session_mut() else {
            return;
        };
        session.updated_at = unix_time();
        let restored = session.dismissed_review_findings.remove(&thread);
        let drain = restored && !session.is_busy() && !session.queued_messages.is_empty();
        let session_id = session.id;
        if !restored {
            session.dismissed_review_findings.insert(thread);
        } else {
            self.transcript_ui
                .review_findings_expanded
                .insert(thread, true);
            let key = chat_review_key(thread);
            self.review_control_focus(format!("{key}-disclosure"), cx)
                .focus(window, cx);
        }
        self.remeasure_transcript_message(index);
        self.save();
        if drain {
            self.drain_queued_message(session_id, cx);
        }
        cx.notify();
    }

    fn chat_review_reply(&self, thread: ReviewThreadRef, index: usize) -> Option<ReviewReply> {
        let session = self.selected_session()?;
        if session.dismissed_review_findings.contains(&thread) {
            return None;
        }
        let message = session
            .messages
            .get(index)
            .filter(|message| message.id == thread.message_id)?;
        let report = self.transcript_model.chat_reviews.report(message)?;
        let finding = report.findings.get(thread.finding_index)?;
        Some(ReviewReply {
            thread,
            finding: finding.context(),
        })
    }

    fn attach_chat_review_reply(
        &mut self,
        thread: ReviewThreadRef,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(reply) = self.chat_review_reply(thread, index) else {
            return;
        };
        self.composer_model.review_reply = Some(reply);
        self.schedule_composer_draft_save(cx);
        window.focus(&self.composer_focus(cx), cx);
        cx.notify();
    }

    fn fix_chat_review_finding(
        &mut self,
        thread: ReviewThreadRef,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(reply) = self.chat_review_reply(thread, index) else {
            return;
        };
        let Some(session) = self.selected_session() else {
            return;
        };
        let mut submission = ComposerSubmission::plain(tr!("review.chat_request_fix"));
        submission.review_reply = Some(reply);
        self.submit_submission_for_session(session.id, submission, cx);
    }

    pub(super) fn render_chat_review(
        &self,
        message_index: usize,
        message: &Message,
        report: &Report,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let palette = MarkdownPalette::from_theme(&theme);
        let session = self.selected_session();
        div().flex().flex_col().pt(px(6.0)).children(
            report
                .findings
                .iter()
                .enumerate()
                .map(|(finding_index, finding)| {
                    let thread = ReviewThreadRef {
                        message_id: message.id,
                        finding_index,
                    };
                    let key = chat_review_key(thread);
                    let dismissed = session
                        .is_some_and(|session| session.dismissed_review_findings.contains(&thread));
                    let expanded = !dismissed
                        && self
                            .transcript_ui
                            .review_findings_expanded
                            .get(&thread)
                            .copied()
                            .unwrap_or(finding_index == 0);
                    let dismiss = |cx: &mut Context<Self>| {
                        self.chat_review_button(
                            format!("{key}-dismiss"),
                            if dismissed {
                                tr!("review.reopen")
                            } else {
                                tr!("review.dismiss")
                            },
                            cx,
                            move |this, window, cx| {
                                this.toggle_chat_review_dismissed(thread, message_index, window, cx)
                            },
                        )
                    };
                    let title = div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .min_h(px(22.0))
                        .rounded(px(5.0))
                        .child(
                            div()
                                .min_w_0()
                                .text_style(TextStyle::CALL_OUT)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if dismissed {
                                    theme.text_secondary
                                } else {
                                    theme.text
                                })
                                .child(finding.title.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_style(TextStyle::SUBHEADLINE)
                                .text_color(theme.text_secondary)
                                .child(format!("· {}", finding.priority)),
                        )
                        .when(!dismissed, |title| {
                            title.child(icon(
                                if expanded {
                                    "chevron.down"
                                } else {
                                    "chevron.right"
                                },
                                10.0,
                                theme.text,
                            ))
                        });
                    let location = self.render_chat_review_location(
                        &finding.location,
                        &key,
                        &theme,
                        window,
                        cx,
                    );
                    if dismissed {
                        return div().child(Divider::horizontal()).child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .py(px(4.0))
                                .child(div().flex_1().min_w_0().child(title).child(location))
                                .child(dismiss(cx)),
                        );
                    }
                    let header = {
                        let id = format!("{key}-disclosure");
                        let focus = self.review_control_focus(id.clone(), cx);
                        title
                            .id(SharedString::from(id))
                            .track_focus(&focus)
                            .tab_index(0)
                            .tab_stop(true)
                            .squircle()
                            .cursor_default()
                            .focus_visible(|style| {
                                style.shadow(vec![
                                    gpui::BoxShadow::new(px(0.0), px(0.0), theme.accent)
                                        .spread_radius(px(1.0))
                                        .inset(),
                                ])
                            })
                            .on_activation(cx, move |this, _, cx| {
                                this.set_chat_review_expanded(thread, message_index, !expanded, cx);
                            })
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if !event.keystroke.modifiers.modified()
                                    && ((!expanded && event.keystroke.key == "right")
                                        || (expanded && event.keystroke.key == "left"))
                                {
                                    this.set_chat_review_expanded(
                                        thread,
                                        message_index,
                                        !expanded,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }
                            }))
                    };
                    div().child(Divider::horizontal()).child(
                        DisclosureGroup::new(expanded, div().child(header).child(location), || {
                            let context = self.markdown_ctx(
                                key.clone(),
                                &palette,
                                self.content_markdown_metrics(MarkdownMetrics::BODY),
                                false,
                            );
                            let body = {
                                let mut views =
                                    self.transcript_model.chat_reviews.markdown.borrow_mut();
                                let view = views.entry(thread).or_insert_with(MarkdownView::seeded);
                                view.set_text(&finding.body, false);
                                md::render::markdown(view, &context)
                            };
                            let reply_and_fix = div()
                                .flex()
                                .flex_wrap()
                                .ml_auto()
                                .justify_end()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    self.chat_review_button(
                                        format!("{key}-reply"),
                                        tr!("review.chat_reply"),
                                        cx,
                                        move |this, window, cx| {
                                            this.attach_chat_review_reply(
                                                thread,
                                                message_index,
                                                window,
                                                cx,
                                            );
                                        },
                                    )
                                    .icon(icon(
                                        "bubble.left",
                                        11.0,
                                        theme.text_secondary,
                                    )),
                                )
                                .child(
                                    self.chat_review_button(
                                        format!("{key}-fix"),
                                        tr!("review.fix"),
                                        cx,
                                        move |this, _, cx| {
                                            this.fix_chat_review_finding(thread, message_index, cx);
                                        },
                                    )
                                    .bg(theme.overlay_strong)
                                    .text_color(theme.text),
                                );
                            div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .pb(px(6.0))
                                .children(body)
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(dismiss(cx))
                                        .child(reply_and_fix),
                                )
                                .into_any_element()
                        })
                        .py(px(4.0)),
                    )
                })
                .collect::<Vec<_>>(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::folded_transcript_row_kinds;
    use super::*;

    const REPORT: &str = "Reviewed the cache.\n\n### [P1] Lock held during disk I/O\n`src/cache.rs:12-16`\n\nThe reader blocks all lookups.\n\n```text\n### [P2] This is sample output\n```\n\n### [P3] Repeated allocation\n`src/cache.rs:40`\n\nReuse the buffer.";

    #[test]
    fn splits_review_markdown_at_real_headings_only() {
        let report = parse_report(REPORT).unwrap();
        assert_eq!(report.summary, "Reviewed the cache.");
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].location, "src/cache.rs:12-16");
        assert!(report.findings[0].body.contains("This is sample output"));
        assert_eq!(report.findings[1].title, "Repeated allocation");
        assert!(parse_report("### [P1] Missing location\n\nOrdinary prose.").is_none());
        assert!(
            parse_report("### [P1] Backward range\n`src/cache.rs:9-3`\n\nOrdinary prose.")
                .is_none()
        );
    }

    #[test]
    fn batch_fixes_follow_the_latest_review_and_skip_dismissed_findings() {
        let reviews = ChatReviews::default();
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        assert!(reviews.fixes(&session, ReviewFixRequest::Recent).is_none());
        session.begin_turn("Проведи ревью модуля на производительность");
        let message_id = session.push_message(MessageRole::Assistant, REPORT);
        session.finish_active_turn(TurnStatus::Completed);
        let first = ReviewThreadRef {
            message_id,
            finding_index: 0,
        };
        session.dismissed_review_findings.insert(first);
        let selected = reviews.fixes(&session, ReviewFixRequest::Recent).unwrap();
        assert!(selected.contains("Repeated allocation"));
        assert!(!selected.contains("Lock held"));
        let second = ReviewThreadRef {
            message_id,
            finding_index: 1,
        };
        session.dismissed_review_findings.insert(second);
        assert_eq!(
            reviews.fixes(&session, ReviewFixRequest::Recent).as_deref(),
            Some("")
        );
        session.dismissed_review_findings.remove(&first);
        assert!(
            reviews
                .fixes(&session, ReviewFixRequest::Recent)
                .unwrap()
                .contains("Lock held")
        );
        session.begin_turn("Проведи ревью ещё раз");
        session.push_message(MessageRole::Assistant, "Больше замечаний нет.");
        session.finish_active_turn(TurnStatus::Completed);
        assert_eq!(
            reviews.fixes(&session, ReviewFixRequest::Recent).as_deref(),
            Some("")
        );
    }

    #[test]
    fn recognizes_short_fix_requests_without_intercepting_a_specific_edit() {
        for text in [
            "Исправляй!",
            "Исправляй, пожалуйста.",
            "Исправь открытые замечания",
        ] {
            assert!(review_fix_request(text).is_some(), "{text}");
        }
        assert_eq!(
            review_fix_request("Исправь название метода в src/cache.rs"),
            None
        );
    }

    #[test]
    fn unrelated_turn_ends_implicit_review_scope_but_explicit_requests_still_work() {
        let reviews = ChatReviews::default();
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        session.begin_turn("Review the cache");
        let message_id = session.push_message(MessageRole::Assistant, REPORT);
        session.finish_active_turn(TurnStatus::Completed);
        let request = review_fix_request("Исправляй").unwrap();
        assert!(reviews.fixes(&session, request).is_some());

        session.begin_turn("What happens under contention?");
        session.turns.last_mut().unwrap().review_thread = Some(ReviewThreadRef {
            message_id,
            finding_index: 0,
        });
        session.push_message(MessageRole::Assistant, "All readers wait for the disk.");
        session.finish_active_turn(TurnStatus::Completed);
        assert!(reviews.fixes(&session, request).is_some());

        session.begin_turn("Why is the export test failing?");
        session.push_message(
            MessageRole::Assistant,
            "The fixture expects the old header.",
        );
        session.finish_active_turn(TurnStatus::Completed);
        assert!(reviews.fixes(&session, request).is_none());
        let explicit = review_fix_request("Исправь все замечания").unwrap();
        assert!(
            reviews
                .fixes(&session, explicit)
                .unwrap()
                .contains("Lock held")
        );
    }

    #[test]
    fn review_replies_stay_in_the_transcript_in_order() {
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        session.begin_turn("Review the cache.");
        let message_id = session.push_message(MessageRole::Assistant, REPORT);
        session.finish_active_turn(TurnStatus::Completed);
        let thread = ReviewThreadRef {
            message_id,
            finding_index: 1,
        };
        session.begin_turn("What would you reuse?");
        session.turns.last_mut().unwrap().review_thread = Some(thread);
        session.push_message(MessageRole::Assistant, "Keep a buffer per worker.");
        session.finish_active_turn(TurnStatus::Completed);
        session.begin_turn("Run the tests.");

        let rows = folded_transcript_row_kinds(&session, &HashSet::new());
        let message_indices = rows
            .into_iter()
            .filter_map(|row| match row {
                TranscriptRowKind::Message(index) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(message_indices, vec![0, 1, 2, 3, 4]);
    }
}
