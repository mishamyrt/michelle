use super::model_picker::MODEL_PICKER_MENU_ID;
use super::*;
use crate::ui::components::menu_chip::chip_icon;
use crate::ui::squircle::{SquircleStyled, squircle};
use crate::ui::{StyledTypography, TextStyle};

pub(super) mod attachments;
pub(super) mod drafts;
pub(super) mod model;
pub(super) mod sources;

pub(super) struct ComposerUi {
    pub(in crate::app) input: Entity<ComposerInput>,
    pub(in crate::app) autocomplete: autocomplete::AutocompleteUi,
}

const COMPUTER_USE_PREVIEW_WIDTH: f32 = 304.0;
const COMPUTER_USE_PREVIEW_HEIGHT: f32 = 172.0;
const COMPUTER_USE_PREVIEW_RADIUS: f32 = 12.0;
const COMPUTER_USE_PREVIEW_INNER_RADIUS: f32 = COMPUTER_USE_PREVIEW_RADIUS - 1.0;

struct ComputerUsePreviewDrag {
    cursor_offset: Cell<gpui::Point<Pixels>>,
}

fn clamp_computer_use_preview_position(
    position: gpui::Point<Pixels>,
    size: gpui::Size<Pixels>,
    window: &Window,
) -> gpui::Point<Pixels> {
    let inset = window.client_inset().unwrap_or_default() + px(8.0);
    let viewport = window.viewport_size();
    point(
        position
            .x
            .clamp(inset, (viewport.width - size.width - inset).max(inset)),
        position
            .y
            .clamp(inset, (viewport.height - size.height - inset).max(inset)),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComposerSubmitAction {
    Send,
    Preparing,
    Stop,
}

pub(super) fn composer_submit_action(
    status: Option<SessionStatus>,
    preparing: bool,
) -> ComposerSubmitAction {
    if preparing {
        ComposerSubmitAction::Preparing
    } else if status.is_some_and(SessionStatus::is_busy) {
        ComposerSubmitAction::Stop
    } else {
        ComposerSubmitAction::Send
    }
}

impl Michelle {
    pub(super) fn toggle_composer_menu(
        &self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_model_picker = id == MODEL_PICKER_MENU_ID;
        let menus = self.shell_ui.menus.borrow();
        let Some(handle) = menus.get(id).cloned() else {
            return;
        };
        // A keyboard toggle produces no mouse-down for another open menu's
        // dismiss-on-down-out to see, so close the rest here.
        let other_open: Vec<_> = menus
            .iter()
            .filter(|(other_id, other)| other_id.as_ref() != id && other.is_open())
            .map(|(_, other)| other.clone())
            .collect();
        drop(menus);
        // The picker's toggle observers update this entity, so the toggle has
        // to run after this listener releases it.
        window.defer(cx, move |window, cx| {
            for menu in other_open {
                menu.close(window, cx);
            }
            if is_model_picker {
                crate::ui::menu::toggle_popover(&handle, MenuAlign::AboveLeft, window, cx);
            } else {
                crate::ui::menu::toggle_dropdown(&handle, MenuAlign::AboveLeft, window, cx);
            }
        });
    }

    // ── Permission ─────────────────────────────────────────────────────────

    pub(super) fn render_permission(&self, cx: &mut Context<Self>) -> Option<Div> {
        if let Some(input) = self.selected_runtime()?.pending_user_input.clone() {
            return Some(self.render_user_input(input, cx));
        }
        if let Some(permission) = self.selected_runtime()?.pending_computer_approval.as_ref() {
            return Some(self.render_computer_permission(permission, cx));
        }
        let permission = self.selected_runtime()?.pending_permission.as_ref()?;
        let theme = Theme::current(cx);
        let request_id = permission.request_id.clone();
        let mut buttons = div().flex().items_center().gap(px(8.0)).mt(px(10.0));
        for option in &permission.options {
            let request_id = request_id.clone();
            let option_id = option.id.clone();
            let allow = option.allow;
            buttons = buttons.child(
                div()
                    .id(SharedString::from(format!(
                        "permission-{}-{}",
                        permission.request_id, option.id
                    )))
                    .h(px(28.0))
                    .px(px(13.0))
                    .rounded(px(7.0))
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(allow, |element| {
                        element
                            .bg(theme.inverse)
                            .text_color(theme.on_inverse)
                            .hover(|element| element.opacity(0.9))
                    })
                    .when(!allow, |element| {
                        element
                            .border_1()
                            .border_color(theme.border_strong)
                            .text_color(theme.text_secondary)
                            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
                    })
                    .active(|element| element.opacity(0.8))
                    .child(SharedString::from(option.label.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.respond_permission(request_id.clone(), option_id.clone(), cx);
                    })),
            );
        }
        Some(
            div().px(px(20.0)).pb(px(8.0)).child(
                div()
                    .w_full()
                    .max_w(px(CONTENT_MAX_WIDTH))
                    .mx_auto()
                    .p(px(12.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(theme.raised)
                    .shadow_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(icon("exclamationmark.triangle", 13.0, theme.warning))
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(SharedString::from(permission.title.clone())),
                            ),
                    )
                    .child(
                        div()
                            .id("permission-detail")
                            .mt(px(8.0))
                            .max_h(px(92.0))
                            .overflow_y_scroll()
                            .p(px(8.0))
                            .rounded(px(7.0))
                            .bg(theme.inset)
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_size(sp(12.5))
                            .line_height(sp(16.0))
                            .text_color(theme.text_secondary)
                            .whitespace_normal()
                            .child(SharedString::from(permission.detail.clone())),
                    )
                    .child(buttons),
            ),
        )
    }

    fn render_user_input(&self, pending: PendingUserInput, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let Some(question) = pending.current_question().cloned() else {
            return div();
        };
        let selected = pending
            .selections
            .get(&question.id)
            .cloned()
            .unwrap_or_default();
        let has_custom = pending
            .custom_answers
            .get(&question.id)
            .is_some_and(|answer| !answer.trim().is_empty());
        let can_continue = has_custom || !selected.is_empty();
        let is_last = pending.question_index + 1 == pending.questions.len();
        let request_id = pending.request_id.clone();
        let question_index = pending.question_index;
        let mut options = div().mt(px(9.0)).flex().flex_col().gap(px(4.0));
        for (index, option) in question.options.iter().enumerate() {
            let is_selected = selected.iter().any(|answer| answer == &option.label);
            let click_label = option.label.clone();
            let key_label = option.label.clone();
            let focus = self.transcript_control_focus(
                format!("user-input-{request_id}-{question_index}-option-{index}"),
                cx,
            );
            options = options.child(
                div()
                    .id(SharedString::from(format!(
                        "user-input-{request_id}-{question_index}-option-{index}"
                    )))
                    .track_focus(&focus)
                    .tab_index(0)
                    .tab_stop(true)
                    .min_h(px(36.0))
                    .px(px(10.0))
                    .py(px(5.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if is_selected {
                        theme.accent.opacity(0.34)
                    } else {
                        theme.border.opacity(0.0)
                    })
                    .bg(if is_selected {
                        theme.accent.opacity(0.08)
                    } else {
                        theme.overlay
                    })
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_default()
                    .focus_visible(|style| style.border_color(theme.accent))
                    .when(!is_selected, |row| {
                        row.hover(|style| style.border_color(theme.border).bg(theme.overlay_strong))
                    })
                    .active(|style| style.opacity(0.85))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(sp(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(SharedString::from(option.label.clone())),
                            )
                            .children(option.description.as_ref().map(|description| {
                                div()
                                    .mt(px(1.0))
                                    .text_size(sp(12.5))
                                    .line_height(sp(15.0))
                                    .text_color(theme.text_secondary)
                                    .whitespace_normal()
                                    .child(SharedString::from(description.clone()))
                            })),
                    )
                    .when(is_selected, |row| {
                        row.child(icon("checkmark", 12.0, theme.accent))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_user_input_option(click_label.clone(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.select_user_input_option(key_label.clone(), cx);
                            cx.stop_propagation();
                        }
                    })),
            );
        }

        let next_focus = self.transcript_control_focus(
            format!("user-input-{request_id}-{question_index}-continue"),
            cx,
        );
        let back = (question_index > 0).then(|| {
            let focus = self.transcript_control_focus(
                format!("user-input-{request_id}-{question_index}-back"),
                cx,
            );
            div()
                .id(SharedString::from(format!(
                    "user-input-{request_id}-{question_index}-back"
                )))
                .track_focus(&focus)
                .tab_index(0)
                .tab_stop(true)
                .h(px(26.0))
                .px(px(8.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .cursor_default()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_tertiary)
                .focus_visible(|style| style.border_1().border_color(theme.accent))
                .hover(|style| style.bg(theme.overlay).text_color(theme.text_secondary))
                .active(|style| style.opacity(0.8))
                .child(tr!("user_input.back"))
                .on_click(cx.listener(|this, _, _, cx| this.previous_user_input(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.previous_user_input(cx);
                        cx.stop_propagation();
                    }
                }))
        });
        let continue_button = div()
            .id(SharedString::from(format!(
                "user-input-{request_id}-{question_index}-continue"
            )))
            .track_focus(&next_focus)
            .tab_index(0)
            .tab_stop(can_continue)
            .h(px(26.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .flex()
            .items_center()
            .cursor_default()
            .text_size(sp(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .bg(if can_continue {
                theme.inverse
            } else {
                theme.overlay
            })
            .text_color(if can_continue {
                theme.on_inverse
            } else {
                theme.text_ghost
            })
            .when(can_continue, |button| {
                button
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .hover(|style| style.opacity(0.9))
                    .active(|style| style.opacity(0.8))
                    .on_click(cx.listener(|this, _, _, cx| this.advance_user_input(cx)))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.advance_user_input(cx);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(if is_last {
                tr!("user_input.submit")
            } else {
                tr!("user_input.next")
            });

        let progress = (pending.questions.len() > 1).then(|| {
            div()
                .h(px(18.0))
                .px(px(6.0))
                .rounded(px(5.0))
                .bg(theme.overlay)
                .flex()
                .items_center()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_tertiary)
                .child(tr!(
                    "user_input.progress",
                    current = question_index + 1,
                    total = pending.questions.len()
                ))
        });

        div().flex_none().px(px(20.0)).pb(px(8.0)).child(
            div()
                .id(SharedString::from(format!("user-input-{request_id}")))
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH))
                .mx_auto()
                .px(px(14.0))
                .pt(px(12.0))
                .pb(px(10.0))
                .rounded(px(13.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.composer)
                .tab_index(0)
                .tab_group()
                .tab_stop(false)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_tertiary)
                                .child(SharedString::from(question.header.clone())),
                        )
                        .children(progress),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(sp(13.0))
                        .line_height(sp(18.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .whitespace_normal()
                        .child(SharedString::from(question.question.clone())),
                )
                .children((!question.options.is_empty()).then_some(options))
                .child(
                    div()
                        .mt(px(if question.options.is_empty() {
                            9.0
                        } else {
                            4.0
                        }))
                        .h(px(34.0))
                        .px(px(10.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(if has_custom {
                            theme.accent.opacity(0.34)
                        } else {
                            theme.border.opacity(0.0)
                        })
                        .bg(if has_custom {
                            theme.accent.opacity(0.06)
                        } else {
                            theme.overlay
                        })
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .text_size(sp(12.5))
                        .line_height(sp(16.0))
                        .child(icon(
                            "pencil",
                            11.0,
                            if has_custom {
                                theme.accent
                            } else {
                                theme.text_ghost
                            },
                        ))
                        .child(self.session_ui.user_input_answer.clone()),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .flex()
                        .items_center()
                        .children(back)
                        .child(div().flex_1())
                        .child(continue_button),
                ),
        )
    }

    fn render_computer_permission(
        &self,
        permission: &PendingComputerApproval,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = Theme::current(cx);
        let target = &permission.target;
        let mut buttons = div().mt(px(12.0)).flex().items_center().gap(px(8.0));
        let mut options = vec![
            ("task", tr!("computer_use.allow_for_task"), true),
            ("deny", tr!("common.deny"), false),
        ];
        if target.persistable() {
            options.insert(1, ("always", tr!("computer_use.always_allow_app"), false));
        }
        for (decision, label, primary) in options {
            buttons = buttons.child(
                div()
                    .id(SharedString::from(format!(
                        "computer-permission-{}-{decision}",
                        permission.request.call_id
                    )))
                    .h(px(29.0))
                    .px(px(13.0))
                    .rounded(px(7.0))
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(sp(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(primary, |element| {
                        element
                            .bg(theme.inverse)
                            .text_color(theme.on_inverse)
                            .hover(|element| element.opacity(0.9))
                    })
                    .when(!primary, |element| {
                        element
                            .border_1()
                            .border_color(theme.border_strong)
                            .text_color(theme.text_secondary)
                            .hover(|element| element.bg(theme.overlay).text_color(theme.text))
                    })
                    .active(|element| element.opacity(0.8))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.respond_computer_permission(decision, cx);
                    })),
            );
        }

        div().px(px(20.0)).pb(px(8.0)).child(
            div()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH))
                .mx_auto()
                .p(px(13.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(theme.warning.opacity(0.5))
                .bg(theme.raised)
                .shadow_md()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .child(icon("globe", 14.0, theme.warning))
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(tr!("computer_use.allow_control", app = &target.app_name)),
                        ),
                )
                .child(
                    div()
                        .mt(px(7.0))
                        .text_size(sp(12.5))
                        .line_height(sp(14.0))
                        .text_color(theme.text_secondary)
                        .child(tr!("computer_use.screenshot_shared")),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .p(px(9.0))
                        .rounded(px(8.0))
                        .bg(theme.inset)
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .truncate()
                                .child(SharedString::from(target.window_title.clone())),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(SharedString::from(permission.request.summary())),
                        )
                        .when(permission.sensitive, |element| {
                            element.child(
                                div()
                                    .mt(px(5.0))
                                    .text_size(sp(12.5))
                                    .text_color(theme.warning)
                                    .child(tr!("computer_use.sensitive_action")),
                            )
                        }),
                )
                .child(
                    div()
                        .mt(px(7.0))
                        .text_size(sp(12.5))
                        .text_color(theme.text_tertiary)
                        .child(if target.persistable() {
                            tr!("computer_use.bundle_id", id = &target.bundle_id)
                        } else {
                            tr!("computer_use.no_bundle_id")
                        }),
                )
                .child(buttons),
        )
    }

    pub(super) fn render_computer_use_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let previews = self
            .selected_runtime()?
            .computer_use_previews
            .iter()
            .filter(|state| {
                state.visible
                    && state.target.is_some()
                    && state.phase != ComputerUsePhase::AwaitingApproval
            })
            .collect::<Vec<_>>();
        if previews.is_empty() {
            return None;
        }
        let theme = Theme::current(cx);
        let stack_x_offset = 14.0;
        let stack_y_offset = 24.0;
        let deepest_x_offset = (previews.len().saturating_sub(1) as f32) * stack_x_offset;
        let deepest_y_offset = (previews.len().saturating_sub(1) as f32) * stack_y_offset;
        let stack_size = gpui::size(
            px(COMPUTER_USE_PREVIEW_WIDTH + deepest_x_offset),
            px(COMPUTER_USE_PREVIEW_HEIGHT + deepest_y_offset),
        );
        let position = clamp_computer_use_preview_position(
            self.session_ui
                .computer_use_preview_position
                .unwrap_or_else(|| {
                    let viewport = window.viewport_size();
                    point(
                        viewport.width
                            - px(self.shell_ui.right_panel_rendered_width + 16.0)
                            - stack_size.width,
                        viewport.height - px(82.0) - stack_size.height,
                    )
                }),
            stack_size,
            window,
        );
        let top_index = previews.len() - 1;
        let cards = previews
            .into_iter()
            .enumerate()
            .filter_map(|(index, state)| {
                let target = state.target.as_ref()?;
                let window_id = target.window_id;
                let app_name = target.app_name.clone();
                let screenshot = state
                    .frames
                    .current
                    .as_ref()
                    .map(|frame| frame.image.clone());
                let is_top = index == top_index;
                let depth = (top_index - index) as f32;
                let x_offset = depth * stack_x_offset;
                let y_offset = depth * stack_y_offset;
                let card_offset = point(
                    px(deepest_x_offset - x_offset),
                    px(deepest_y_offset - y_offset),
                );
                let focus =
                    self.transcript_control_focus(format!("computer-use-preview-{window_id}"), cx);
                let mouse_focus = focus.clone();
                let close_focus = self.transcript_control_focus(
                    format!("computer-use-preview-close-{window_id}"),
                    cx,
                );
                let group_name = SharedString::from(format!("computer-use-preview-{window_id}"));
                let keyboard_controls = window.last_input_was_keyboard()
                    && (focus.contains_focused(window, cx) || close_focus.is_focused(window));

                Some(
                    div()
                        .id(SharedString::from(format!(
                            "computer-use-preview-{window_id}"
                        )))
                        .track_focus(&focus)
                        .tab_index(0)
                        .tab_stop(true)
                        .group(group_name.clone())
                        .absolute()
                        .right(px(x_offset))
                        .bottom(px(y_offset))
                        .w(px(COMPUTER_USE_PREVIEW_WIDTH))
                        .h(px(COMPUTER_USE_PREVIEW_HEIGHT))
                        .rounded(px(COMPUTER_USE_PREVIEW_RADIUS))
                        .overflow_hidden()
                        .border_1()
                        .border_color(if is_top {
                            theme.border_strong
                        } else {
                            theme.border
                        })
                        .bg(theme.raised)
                        .shadow(vec![
                            gpui::BoxShadow::new(
                                px(0.0),
                                px(4.0),
                                gpui::black().opacity(if theme.is_dark { 0.32 } else { 0.16 }),
                            )
                            .blur_radius(px(16.0)),
                        ])
                        .occlude()
                        .cursor(gpui::CursorStyle::OpenHand)
                        .focus_visible(|style| style.border_color(theme.accent))
                        .on_drag(
                            ComputerUsePreviewDrag {
                                cursor_offset: Cell::default(),
                            },
                            move |drag, offset, _, cx| {
                                drag.cursor_offset.set(offset + card_offset);
                                cx.new(|_| gpui::EmptyView)
                            },
                        )
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.focus(&mouse_focus, cx);
                            cx.stop_propagation();
                        })
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                            if !focus.is_focused(window) {
                                return;
                            }
                            let step = px(if event.keystroke.modifiers.shift {
                                40.0
                            } else {
                                10.0
                            });
                            let delta = match event.keystroke.key.as_str() {
                                "left" => point(-step, px(0.0)),
                                "right" => point(step, px(0.0)),
                                "up" => point(px(0.0), -step),
                                "down" => point(px(0.0), step),
                                _ => return,
                            };
                            this.session_ui.computer_use_preview_position =
                                Some(clamp_computer_use_preview_position(
                                    position + delta,
                                    stack_size,
                                    window,
                                ));
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .child(
                            // GPUI overflow clipping is rectangular. Round
                            // each painted layer inside the card's 1px border.
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(px(COMPUTER_USE_PREVIEW_INNER_RADIUS))
                                .overflow_hidden()
                                .bg(theme.inset)
                                .when_some(screenshot, |element, screenshot| {
                                    element.child(
                                        img(screenshot)
                                            .w_full()
                                            .h_full()
                                            .rounded(px(COMPUTER_USE_PREVIEW_INNER_RADIUS))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                })
                                .when(state.frames.current.is_none(), |element| {
                                    element.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(sp(12.0))
                                            .text_color(theme.text_secondary)
                                            .child(tr!("computer_use.preparing_preview")),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w_full()
                                .h(px(64.0))
                                .rounded_t(px(COMPUTER_USE_PREVIEW_INNER_RADIUS))
                                .pt(px(6.0))
                                .pl(px(6.0))
                                .pr(px(10.0))
                                .flex()
                                .items_start()
                                .gap(px(6.0))
                                .opacity(if keyboard_controls { 1.0 } else { 0.0 })
                                .group_hover(group_name, |style| style.opacity(1.0))
                                .bg(linear_gradient(
                                    180.0,
                                    linear_color_stop(gpui::black().opacity(0.65), 0.5),
                                    linear_color_stop(gpui::transparent_black(), 1.0),
                                ))
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "computer-use-preview-close-{window_id}"
                                        )))
                                        .track_focus(&close_focus)
                                        .aria_label(tr!("common.close"))
                                        .tab_index(0)
                                        .tab_stop(true)
                                        .size(px(26.0))
                                        .flex_none()
                                        .rounded_full()
                                        .border_1()
                                        .border_color(gpui::transparent_black())
                                        .focus_visible(|style| style.border_color(gpui::white()))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_default()
                                        .hover(|style| style.bg(gpui::white().opacity(0.16)))
                                        .active(|style| style.bg(gpui::white().opacity(0.24)))
                                        .tooltip(Tooltip::text(tr!("common.close")))
                                        .child(icon("xmark", 11.0, gpui::white()))
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            window.focus(&close_focus, cx);
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.dismiss_computer_use(window_id, cx);
                                            window.focus(&this.composer_focus(cx), cx);
                                        })),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .pt(px(5.0))
                                        .text_size(sp(12.0))
                                        .line_height(sp(16.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(gpui::white())
                                        .truncate()
                                        .child(SharedString::from(app_name)),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.bring_computer_use_to_front(window_id, cx);
                        })),
                )
            })
            .collect::<Vec<_>>();

        let stack = div()
            .id("computer-use-previews")
            .tab_group()
            .tab_stop(false)
            .relative()
            .w(stack_size.width)
            .h(stack_size.height)
            .children(cards)
            .on_drag_move::<ComputerUsePreviewDrag>(cx.listener(
                move |this, event: &gpui::DragMoveEvent<ComputerUsePreviewDrag>, window, cx| {
                    let position = event.event.position - event.drag(cx).cursor_offset.get();
                    this.session_ui.computer_use_preview_position = Some(
                        clamp_computer_use_preview_position(position, stack_size, window),
                    );
                    // Native drag dispatch already refreshes the window; avoid
                    // an additional root notify for every pointer movement.
                },
            ));

        Some(
            gpui::deferred(
                gpui::anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.0))
                    .child(stack),
            )
            .into_any_element(),
        )
    }

    // ── Composer ───────────────────────────────────────────────────────────

    pub(super) fn render_access_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let selected_mode = self
            .selected_session()
            .map(|session| session.runtime_mode)
            .unwrap_or_default();
        let weak = cx.entity().downgrade();
        let handle = self.menu_handle("runtime-mode", cx);
        dropdown_menu(
            MenuChip::new("runtime-mode")
                .text_style(TextStyle::Caption)
                .icon(selected_mode.icon(), theme.text_secondary)
                .label(selected_mode.label())
                .caret(false)
                .selected(handle.is_open()),
            "runtime-mode-menu",
            &handle,
            MenuAlign::AboveLeft,
            move |_| {
                RuntimeMode::ACCESS_OPTIONS
                    .into_iter()
                    .map(|option| {
                        let weak = weak.clone();
                        let selected = option == selected_mode;
                        MenuItem::new(option.label(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| this.set_runtime_mode(option, cx));
                        })
                        .selected(selected)
                        .tooltip(option.description())
                    })
                    .collect()
            },
        )
    }

    /// The thread-goal chip: present only while the provider reports a goal,
    /// it pairs a target icon with the status phrase (and budget consumption)
    /// and opens the goal dialog. `/goal` is the keyboard route to the same
    /// surface.
    pub(super) fn render_goal_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self.selected_session()?;
        let goal = session.thread_goal.as_ref()?;
        let session_id = session.id;
        let theme = Theme::current(cx);
        let color = super::goal_dialog::goal_status_color(goal.status, &theme);
        // Elapsed pursuit time accrues only while a turn actually runs,
        // matching how the provider accounts it.
        let live_elapsed_seconds = (goal.status == crate::model::ThreadGoalStatus::Active
            && session.is_busy())
        .then(|| self.goal_model.observed_at.get(&session_id))
        .flatten()
        .map_or(0, |observed| observed.elapsed().as_secs() as i64);
        let label = super::goal_dialog::goal_chip_label(goal, live_elapsed_seconds);
        let objective = goal.objective.clone();
        let weak = cx.entity().downgrade();
        Some(
            div()
                .id("composer-goal")
                .h(px(24.0))
                .px(px(7.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_default()
                .text_style(TextStyle::Caption)
                .text_color(color)
                .child(chip_icon("scope", color))
                .child(div().max_w(px(220.0)).truncate().child(label))
                .hover(|element| element.bg(theme.overlay))
                .tooltip(Tooltip::text(objective))
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.request_goal_dialog(session_id, None, false, cx);
                    });
                })
                .into_any_element(),
        )
    }

    /// Stage files dropped onto the composer as attachment chips. The mention
    /// each chip will submit takes the autocomplete's form: relative to the
    /// project root when the file is inside it, absolute otherwise,
    /// directories with a trailing slash.
    pub(super) fn stage_dropped_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.stage_attachment_paths(paths.paths(), cx) {
            return;
        }
        let focus = self.composer_ui.input.read(cx).focus();
        window.focus(&focus, cx);
    }

    /// The staged-attachment chips above the input: a thumbnail tile per
    /// image, a file-type icon and basename for everything else, each with a
    /// floating remove button — T3 Code's attachment row in graphite.
    fn render_composer_attachments(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut row = div()
            .px(px(16.0))
            .pt(px(2.0))
            .pb(px(8.0))
            .flex()
            .flex_wrap()
            .gap(px(8.0));
        for (index, attachment) in self.composer_model.attachments.iter().enumerate() {
            let menu = self.menu_handle(format!("composer-attachment-{index}-menu"), cx);
            let icon_path = if attachment.is_dir {
                "folder"
            } else {
                super::right_panel::file_icon_for_path(&attachment.mention)
            };
            let mut tile = div()
                .id(SharedString::from(format!("composer-attachment-{index}")))
                .relative()
                .w(px(64.0))
                .h(px(64.0))
                .rounded(px(8.0))
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .bg(theme.inset)
                .track_focus(menu.trigger_focus_handle())
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .tooltip(Tooltip::text(format!("@{}", attachment.mention)));
            let attachment_image = attachment.client_preview_image.clone().or_else(|| {
                attachment
                    .is_image
                    .then(|| {
                        attachment.blob_reference.as_deref().and_then(|reference| {
                            self.image_for_reference(
                                reference,
                                Some(&attachment.path),
                                Some(attachment.name.as_ref()),
                                cx,
                            )
                        })
                    })
                    .flatten()
            });
            let can_reveal = !self.daemon.is_remote();
            if attachment.is_image {
                if let Some(attachment_image) = attachment_image.as_ref() {
                    let preview_image = attachment_image.clone();
                    let preview_name = attachment.name.clone();
                    tile = tile.child(
                        div()
                            .id(SharedString::from(format!(
                                "composer-attachment-{index}-preview"
                            )))
                            .size_full()
                            .cursor_default()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_image_preview(
                                    preview_image.clone(),
                                    preview_name.clone(),
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            }))
                            .child(
                                img(attachment_image.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    );
                } else {
                    tile = tile.child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("photo", 16.0, theme.text_ghost)),
                    );
                }
            } else {
                tile = tile.child(
                    div()
                        .size_full()
                        .px(px(5.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(5.0))
                        .child(icon(icon_path, 16.0, theme.text_tertiary))
                        .child(
                            div().w_full().flex().justify_center().child(
                                div()
                                    .max_w_full()
                                    .truncate()
                                    .text_size(px(12.5))
                                    .text_color(theme.text_tertiary)
                                    .child(attachment.name.clone()),
                            ),
                        ),
                );
            }
            let key_menu = menu.clone();
            let key_image = attachment_image.clone();
            let key_name = attachment.name.clone();
            let is_image = attachment.is_image;
            tile = tile.on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if is_image
                    && matches!(key, "enter" | "space")
                    && let Some(key_image) = key_image.as_ref()
                {
                    this.open_image_preview(key_image.clone(), key_name.clone(), window, cx);
                    cx.stop_propagation();
                } else if key == "f10" && event.keystroke.modifiers.shift {
                    key_menu.open_context_menu(window, cx);
                    cx.stop_propagation();
                }
            }));
            let tile = tile.child(
                div()
                    .id(SharedString::from(format!(
                        "composer-attachment-remove-{index}"
                    )))
                    .absolute()
                    .top(px(3.0))
                    .right(px(3.0))
                    .w(px(16.0))
                    .h(px(16.0))
                    .tab_index(0)
                    .rounded(px(5.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_default()
                    .bg(theme.canvas.opacity(0.8))
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .hover(|element| element.bg(theme.canvas.opacity(0.95)))
                    .active(|element| element.opacity(0.8))
                    .child(icon("xmark", 9.0, theme.text_secondary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if index < this.composer_model.attachments.len() {
                            this.composer_model.attachments.remove(index);
                            this.schedule_composer_draft_save(cx);
                            cx.notify();
                        }
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            if index < this.composer_model.attachments.len() {
                                this.composer_model.attachments.remove(index);
                                this.schedule_composer_draft_save(cx);
                                cx.notify();
                            }
                            cx.stop_propagation();
                        }
                    })),
            );
            let reveal_path = attachment.path.clone();
            row = row.child(context_menu(
                tile,
                SharedString::from(format!("composer-attachment-{index}-context-menu")),
                &menu,
                move |_| image_preview::attachment_menu_items(reveal_path.clone(), can_reveal),
            ));
        }
        row
    }

    /// The pending follow-up queue between the transcript and the composer: a
    /// single card tucked against the composer's top edge, one row per queued
    /// message. A row pulls its text back into the composer on click and
    /// carries steer/remove/more controls on the right.
    pub(super) fn render_queued_messages(&self, cx: &mut Context<Self>) -> Option<Div> {
        let session_id = self.state.selected_session?;
        let session = self.selected_session()?;
        if session.queued_messages.is_empty() {
            return None;
        }
        let theme = Theme::current(cx);
        let steerable = self.session_can_steer(session);
        let mut list = div().flex().flex_col().py(px(4.0));
        for message in &session.queued_messages {
            let message_id = message.id;
            let content = if message.visible_content().trim().is_empty() {
                message
                    .attachments
                    .iter()
                    .map(|attachment| attachment.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                message.visible_content().to_owned()
            };
            let steer_control = steerable.then(|| {
                div()
                    .id(SharedString::from(format!(
                        "queued-message-steer-{message_id}"
                    )))
                    .h(px(24.0))
                    .px(px(7.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .cursor_default()
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .hover(|element| element.bg(theme.overlay_strong))
                    .active(|element| element.opacity(0.8))
                    .text_size(px(12.5))
                    .text_color(theme.text_secondary)
                    .child(icon("arrow.turn.down.right", 11.0, theme.text_secondary))
                    .child(tr!("composer.steer"))
                    .tooltip(Tooltip::text(tr!("composer.steer_current")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.steer_queued_message(session_id, message_id, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.steer_queued_message(session_id, message_id, cx);
                            cx.stop_propagation();
                        }
                    }))
            });
            let menu_handle = self.menu_handle(format!("queued-message-menu-{message_id}"), cx);
            let menu_open = menu_handle.is_open();
            let weak = cx.entity().downgrade();
            let more_control = dropdown_menu(
                div()
                    .id(SharedString::from(format!(
                        "queued-message-more-{message_id}"
                    )))
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_default()
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .when(menu_open, |element| element.bg(theme.overlay_strong))
                    .hover(|element| element.bg(theme.overlay_strong))
                    .active(|element| element.opacity(0.8))
                    .child(icon("ellipsis", 12.5, theme.text_secondary)),
                SharedString::from(format!("queued-message-more-menu-{message_id}")),
                &menu_handle,
                MenuAlign::BelowRight,
                move |_| {
                    let edit_weak = weak.clone();
                    let remove_weak = weak.clone();
                    vec![
                        MenuItem::new(tr!("composer.edit_in_composer"), move |window, cx| {
                            let _ = edit_weak.update(cx, |this, cx| {
                                this.edit_queued_message(session_id, message_id, window, cx);
                            });
                        })
                        .icon("pencil"),
                        MenuItem::new(tr!("composer.remove_followup"), move |_, cx| {
                            let _ = remove_weak.update(cx, |this, cx| {
                                this.remove_queued_message(session_id, message_id, cx);
                            });
                        })
                        .icon("trash"),
                    ]
                },
            );
            list = list.child(
                div()
                    .id(SharedString::from(format!("queued-message-{message_id}")))
                    .h(px(30.0))
                    .pl(px(12.0))
                    .pr(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .cursor_default()
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(theme.accent))
                    .hover(|element| element.bg(theme.overlay))
                    .tooltip(Tooltip::text(tr!("composer.edit_in_composer")))
                    .child(icon("text.badge.plus", 12.0, theme.text_tertiary))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(theme.text)
                            .child(SharedString::from(content)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .children(steer_control)
                            .child(
                                div()
                                    .id(SharedString::from(format!(
                                        "queued-message-remove-{message_id}"
                                    )))
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_default()
                                    .tab_index(0)
                                    .focus_visible(|style| {
                                        style.border_1().border_color(theme.accent)
                                    })
                                    .hover(|element| element.bg(theme.overlay_strong))
                                    .active(|element| element.opacity(0.8))
                                    .child(icon("trash", 12.0, theme.text_secondary))
                                    .tooltip(Tooltip::text(tr!("composer.remove_followup")))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.remove_queued_message(session_id, message_id, cx);
                                    }))
                                    .on_key_down(cx.listener(
                                        move |this, event: &KeyDownEvent, _, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "enter" | "space"
                                            ) {
                                                this.remove_queued_message(
                                                    session_id, message_id, cx,
                                                );
                                                cx.stop_propagation();
                                            }
                                        },
                                    )),
                            )
                            .child(more_control),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_queued_message(session_id, message_id, window, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.edit_queued_message(session_id, message_id, window, cx);
                            cx.stop_propagation();
                        }
                    })),
            );
        }
        Some(
            div().flex_none().px(px(20.0)).child(
                div()
                    .w_full()
                    .max_w(px(CONTENT_MAX_WIDTH))
                    .mx_auto()
                    .px(px(14.0))
                    .child(
                        div()
                            .rounded_tl(px(12.0))
                            .rounded_tr(px(12.0))
                            .border_t_1()
                            .border_l_1()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.composer)
                            // Row hover fills are full-width rectangles; clip
                            // them to the card's rounded corners.
                            .overflow_hidden()
                            .child(list),
                    ),
            ),
        )
    }

    pub(super) fn render_composer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let session = self.selected_session();
        let preparing = session.is_some_and(|session| {
            self.sessions
                .runtime
                .submission_preparations
                .contains(&session.id)
                || self
                    .sessions
                    .runtime
                    .response_fork_preparations
                    .contains_key(&session.id)
        });
        let submit_action =
            composer_submit_action(session.map(|session| session.status), preparing);
        let escape_stop_armed = session.is_some_and(|session| {
            self.session_ui
                .escape_stop_confirmation
                .is_armed_for(EscapeStopTarget::for_session(session), Instant::now())
        });
        let has_draft = !self
            .composer_ui
            .input
            .read(cx)
            .content(cx)
            .trim()
            .is_empty()
            || !self.composer_model.attachments.is_empty();
        // With no provider to run it, a draft has nowhere to go. The button
        // reads as unavailable and the submission path refuses too, so
        // `enter` cannot slip past a disabled control.
        let no_providers = self.model_picker_has_no_providers();
        let can_send = has_draft
            && !no_providers
            && !self
                .composer_ui
                .input
                .read(cx)
                .has_pending_attachment_pastes();
        let (autocomplete, autocomplete_actionable) =
            match self.render_composer_autocomplete(window, cx) {
                Some((element, actionable)) => (Some(element), actionable),
                None => (None, false),
            };
        let autocomplete_loading = autocomplete.is_some() && !autocomplete_actionable;
        // Files dragged in from the OS light the card up as a drop target.
        // The wash uses the same squircle outline as the resting background.
        let drop_wash = theme.composer.blend(theme.overlay_strong);
        let drop_ring = theme.accent.opacity(0.7);
        div().flex_none().px(px(16.0)).child(
            div()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH))
                .mx_auto()
                .relative()
                .group("composer-card")
                .child(
                    squircle()
                        .rounded(px(15.0))
                        .bg(theme.composer)
                        .border(px(0.5))
                        .border_color(theme.border)
                        .border_inside()
                        .absolute_expand(),
                )
                .when(cx.has_active_drag(), |card| {
                    card.child(
                        squircle()
                            .rounded(px(15.0))
                            .bg(drop_wash)
                            .border(px(1.0))
                            .border_color(drop_ring)
                            .border_inside()
                            .absolute_expand()
                            .opacity(0.0)
                            .group_drag_over::<ExternalPaths>("composer-card", |style| {
                                style.opacity(1.0)
                            }),
                    )
                })
                // Horizontal insets live on each row (and inside the field's
                // scroll viewport, via `padding_x`) rather than on the card,
                // so the field's overlay scrollbar can hug the card's edge.
                .py(px(10.0))
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    this.stage_dropped_files(paths, window, cx);
                }))
                // Anchor for the bounds probe the autocomplete popup aligns to.
                .child(super::autocomplete::composer_card_bounds_probe(
                    self.composer_ui.autocomplete.card_bounds_cell(),
                ))
                // Only while the popup has selectable rows: the key context
                // routes arrows, `enter`, `tab` and `escape` here as actions,
                // out from under the focused field. The loading state takes
                // only Escape, so it can dismiss without swallowing input.
                .when(autocomplete_actionable, |card| {
                    card.key_context("ComposerAutocomplete")
                        .on_action(cx.listener(|this, _: &SelectNextEntry, window, cx| {
                            this.move_autocomplete_highlight("down", window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &SelectPreviousEntry, window, cx| {
                            this.move_autocomplete_highlight("up", window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &ConfirmEntry, window, cx| {
                            this.accept_autocomplete(None, window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &DismissMenu, _, cx| {
                            this.dismiss_autocomplete(cx);
                        }))
                })
                .when(autocomplete_loading, |card| {
                    card.key_context("ComposerAutocompleteLoading")
                        .on_action(cx.listener(|this, _: &DismissMenu, _, cx| {
                            this.dismiss_autocomplete(cx);
                        }))
                })
                .children(autocomplete)
                .when(!self.composer_model.attachments.is_empty(), |card| {
                    card.child(self.render_composer_attachments(cx))
                })
                .child(div().pt(px(2.0)).child(self.composer_ui.input.clone()))
                .child(
                    div()
                        .mt(px(8.0))
                        .relative()
                        .top(px(2.0))
                        .pl(px(9.0))
                        .pr(px(12.0))
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .text_size(px(12.5))
                        .line_height(px(14.0))
                        .child(self.render_provider_model_control(cx))
                        .children(self.render_model_traits_control(cx))
                        .children(self.render_agent_preset_control(cx))
                        .child(self.render_access_control(cx))
                        .children(self.render_goal_control(cx))
                        .child(div().flex_1())
                        .child(match submit_action {
                            ComposerSubmitAction::Preparing => div()
                                .id("send-or-stop")
                                .w(px(26.0))
                                .h(px(26.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_default()
                                .bg(theme.overlay_strong)
                                .child(motion::spinner(15.0, theme.text_secondary))
                                .tooltip(Tooltip::text(tr!("composer.preparing_task"))),
                            ComposerSubmitAction::Stop => div()
                                .id("working-actions")
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .id("send-or-stop")
                                        .w(px(26.0))
                                        .h(px(26.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_default()
                                        .bg(theme.overlay_strong)
                                        .hover(|element| element.bg(theme.danger_soft))
                                        .active(|element| element.opacity(0.8))
                                        .when(escape_stop_armed, |element| {
                                            element.child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.text)
                                                    .child("Esc"),
                                            )
                                        })
                                        .when(!escape_stop_armed, |element| {
                                            element.child(icon("stop", 18.0, theme.text))
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cancel_turn(cx);
                                        })),
                                )
                                .when(can_send, |element| {
                                    element.child(
                                        div()
                                            .id("queue-follow-up")
                                            .w(px(26.0))
                                            .h(px(26.0))
                                            .rounded_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_default()
                                            .bg(theme.inverse)
                                            .hover(|element| element.opacity(0.9))
                                            .active(|element| element.opacity(0.8))
                                            .child(icon("arrow.up", 16.0, theme.on_inverse))
                                            .tooltip(Tooltip::text(tr!("composer.queue_followup")))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let prompt = this
                                                    .composer_ui
                                                    .input
                                                    .read(cx)
                                                    .content(cx)
                                                    .to_owned();
                                                if let Some(submission) =
                                                    this.submission_with_attachments(&prompt, cx)
                                                {
                                                    this.composer_ui
                                                        .input
                                                        .update(cx, |input, cx| input.clear(cx));
                                                    this.submit_composer_submission(submission, cx);
                                                }
                                            })),
                                    )
                                }),
                            ComposerSubmitAction::Send => div()
                                .id("send-or-stop")
                                .w(px(26.0))
                                .h(px(26.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if can_send {
                                    theme.inverse
                                } else {
                                    theme.overlay_strong
                                })
                                .when(can_send, |element| {
                                    element
                                        .cursor_default()
                                        .hover(|element| element.opacity(0.9))
                                        .active(|element| element.opacity(0.8))
                                })
                                .child(icon(
                                    "arrow.up",
                                    16.0,
                                    if can_send {
                                        theme.on_inverse
                                    } else {
                                        theme.text_ghost
                                    },
                                ))
                                // Says why the button is dead, for the case
                                // the draft is ready and the machine is not.
                                .when(no_providers, |element| {
                                    element.tooltip(Tooltip::text(tr!("composer.no_providers")))
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let prompt =
                                        this.composer_ui.input.read(cx).content(cx).to_owned();
                                    if let Some(submission) =
                                        this.submission_with_attachments(&prompt, cx)
                                    {
                                        this.composer_ui
                                            .input
                                            .update(cx, |input, cx| input.clear(cx));
                                        this.submit_composer_submission(submission, cx);
                                    }
                                })),
                        }),
                ),
        )
    }

    pub(super) fn render_workspace_footer(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let projectless_selected = self.selected_project().is_some_and(Project::is_projectless);
        let can_configure_workspace = self
            .selected_session()
            .is_some_and(|session| !session.has_started() && !session.is_busy());

        let workspace = self
            .selected_session()
            .map(|session| session.workspace.clone())
            .unwrap_or_default();
        let workspace_label = match &workspace {
            SessionWorkspace::Local => SharedString::from(tr!("workspace.local")),
            SessionWorkspace::NewWorktree { .. } => {
                SharedString::from(tr!("workspace.new_worktree"))
            }
            SessionWorkspace::Worktree { branch, .. } => SharedString::from(branch.clone()),
        };
        let workspace_icon = if workspace.is_local() {
            "folder.fill"
        } else {
            "folder.fill.badge.plus"
        };
        let worktree_handle = self.menu_handle("workspace-worktree", cx);
        let worktree_trigger = MenuChip::new("workspace-worktree")
            .text_style(TextStyle::Caption)
            .icon(workspace_icon, theme.text_secondary)
            .label(workspace_label)
            .caret(false)
            .disabled(!can_configure_workspace)
            .selected(can_configure_workspace && worktree_handle.is_open())
            .max_w(px(180.0));
        let worktree_selector = if can_configure_workspace {
            let local_selected = workspace.is_local();
            let worktree_selected = workspace.is_worktree();
            let weak = cx.entity().downgrade();
            dropdown_menu(
                worktree_trigger,
                "workspace-worktree-menu",
                &worktree_handle,
                MenuAlign::AboveLeft,
                move |_| {
                    let local = weak.clone();
                    let worktree = weak.clone();
                    vec![
                        MenuItem::new(tr!("workspace.local"), move |_, cx| {
                            let _ = local.update(cx, |this, cx| {
                                this.select_workspace(SessionWorkspace::Local, cx);
                            });
                        })
                        .selected(local_selected),
                        MenuItem::new(tr!("workspace.new_worktree"), move |_, cx| {
                            let _ = worktree.update(cx, |this, cx| {
                                this.select_workspace(
                                    SessionWorkspace::NewWorktree { base_branch: None },
                                    cx,
                                );
                            });
                        })
                        .selected(worktree_selected)
                        .disabled(projectless_selected),
                    ]
                },
            )
        } else {
            worktree_trigger.into_any_element()
        };

        let branch_selector = self.render_branch_selector(cx);

        let usage_meter = self.render_usage_meter(cx);
        div()
            .flex_none()
            .px(px(16.0))
            .pb(px(8.0))
            .pt(px(4.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(CONTENT_MAX_WIDTH))
                    .mx_auto()
                    .h(px(28.0))
                    // Menu chips add 7px, aligning the icons with the input.
                    .pl(px(9.0))
                    .pr(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .tab_index(0)
                    .tab_group()
                    .tab_stop(false)
                    .text_style(TextStyle::Caption)
                    .child(worktree_selector)
                    .children(branch_selector)
                    .child(div().flex_1())
                    .children(usage_meter),
            )
    }
}

pub(super) fn subscribe_input(input: &Entity<ComposerInput>, cx: &mut Context<Michelle>) {
    cx.subscribe(
        input,
        |this: &mut Michelle, _, event: &ComposerEvent, cx| this.handle_composer_event(event, cx),
    )
    .detach();
    cx.subscribe(
        input,
        |this: &mut Michelle, _, event: &ComposerAttachmentPaste, cx| {
            this.stage_pasted_attachments(event.0.clone(), cx);
        },
    )
    .detach();
}
