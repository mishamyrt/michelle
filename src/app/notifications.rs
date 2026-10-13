//! Notification overlay and text selection.
use super::*;
use crate::ui::{ActivationExt, ControlSize, IconButton, Toast};
use model::ToastTone;
pub(super) mod model;

const TOAST_ANIMATION_DURATION: Duration = Duration::from_millis(150);

pub(super) struct NotificationsUi {
    pub(in crate::app) selection: TranscriptSelection,
    pub(in crate::app) dismiss_focus: Option<FocusHandle>,
}

impl Michelle {
    /// Arm the dismiss timer and build the floating toast layer, if a toast
    /// is active. Every full-window surface (workspace and settings alike)
    /// must include this, or a toast raised there stays invisible until the
    /// user navigates away.
    pub(in crate::app) fn render_active_toast(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.start_toast_dismiss_timer(cx);
        let toast = self
            .notifications
            .toast
            .as_ref()
            .map(|toast| (toast.message.clone(), toast.tone, toast.id));
        toast.map(|(message, tone, generation)| {
            self.render_toast(message, tone, generation, cx)
                .into_any_element()
        })
    }

    pub(in crate::app) fn render_toast(
        &mut self,
        message: String,
        tone: ToastTone,
        generation: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::current(cx);
        let (status_icon, status_color) = match tone {
            ToastTone::Alert => ("exclamationmark.triangle", theme.danger),
            ToastTone::Success => ("checkmark", theme.success),
        };
        let palette = MarkdownPalette::from_theme(&theme);
        let text_style = TextStyle::BODY;
        let text_ctx = MarkdownCtx::new(
            format!("toast-{generation}"),
            &palette,
            MarkdownMetrics {
                text_size: text_style.size().into(),
                line_height: text_style.line_height().into(),
                ..MarkdownMetrics::COMPACT
            },
            self.notifications_ui.selection.clone(),
        );
        let message = md::render::plain_text(
            message,
            md::render::SANS_FAMILY,
            FontWeight::NORMAL,
            theme.text,
            &text_ctx,
        );
        let dismiss_focus = self
            .notifications_ui
            .dismiss_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let dismiss = IconButton::new(
            SharedString::from(format!("dismiss-toast-{generation}")),
            icon("xmark", 12.0, theme.text_tertiary),
        )
        .control_size(ControlSize::Small)
        .track_focus(&dismiss_focus)
        .tooltip(tr!("common.dismiss_notification"))
        .on_activation(cx, |this, _, cx| {
            this.hide_toast();
            cx.notify();
        });

        div()
            .id(SharedString::from(format!("toast-layer-{generation}")))
            .absolute()
            .left_0()
            .top(px(TOOLBAR_HEIGHT + 8.0))
            .w_full()
            .px(px(20.0))
            .flex()
            .justify_center()
            .child(
                Toast::new(SharedString::from(format!("toast-{generation}")), message)
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_toast_hovered(*hovering, cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified() && event.keystroke.key == "escape"
                        {
                            this.hide_toast();
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }))
                    .child(md::render::frame_reset(
                        self.notifications_ui.selection.clone(),
                    ))
                    .icon(icon(status_icon, 14.0, status_color))
                    .dismiss_button(dismiss)
                    .overlay(self.toast_selection_input()),
            )
            // Keep the toast top-centered just beneath Michelle's header.
            // GPUI's animation path honors the system reduce-motion preference
            // and resolves immediately.
            .with_animation(
                SharedString::from(format!("toast-enter-{generation}")),
                Animation::new(TOAST_ANIMATION_DURATION).with_easing(ease_out_quint()),
                |element, delta| {
                    element
                        .top(px(TOOLBAR_HEIGHT + 8.0 * delta))
                        .opacity(0.4 + 0.6 * delta)
                },
            )
    }
    pub(in crate::app) fn toast_selection_input(&self) -> impl IntoElement {
        let selection = self.notifications_ui.selection.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| md::render::install_selection_input(window, &selection),
        )
        .absolute()
        .w(px(0.0))
        .h(px(0.0))
    }
}
