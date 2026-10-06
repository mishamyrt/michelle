//! Notification overlay and text selection.
use super::*;
use model::ToastTone;
pub(super) mod model;

const TOAST_ANIMATION_DURATION: Duration = Duration::from_millis(150);

pub(super) struct NotificationsUi {
    pub(in crate::app) selection: TranscriptSelection,
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
        &self,
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
        let text_ctx = MarkdownCtx::new(
            format!("toast-{generation}"),
            &palette,
            self.scaled_markdown_metrics(MarkdownMetrics::COMPACT),
            self.notifications_ui.selection.clone(),
        );
        let message = md::render::plain_text(
            message,
            md::render::SANS_FAMILY,
            FontWeight::NORMAL,
            theme.text,
            &text_ctx,
        );
        let dismiss = div()
            .id(SharedString::from(format!("dismiss-toast-{generation}")))
            .tab_index(0)
            .size(px(26.0))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_default()
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .tooltip(Tooltip::text(tr!("common.dismiss_notification")))
            .child(icon("xmark", 12.0, theme.text_tertiary))
            .on_click(cx.listener(|this, _, _, cx| {
                this.hide_toast();
                cx.notify();
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape") {
                    this.hide_toast();
                    cx.notify();
                    cx.stop_propagation();
                }
            }));

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
                div()
                    .id(SharedString::from(format!("toast-{generation}")))
                    .occlude()
                    .max_w(px(560.0))
                    .min_w_0()
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(theme.raised)
                    .shadow_lg()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(sp(12.5))
                    .line_height(sp(16.0))
                    .text_color(theme.text)
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_toast_hovered(*hovering, cx);
                    }))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(md::render::frame_reset(
                        self.notifications_ui.selection.clone(),
                    ))
                    .child(icon(status_icon, 14.0, status_color))
                    .child(div().flex_1().min_w_0().whitespace_normal().child(message))
                    .child(dismiss)
                    .child(self.toast_selection_input()),
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
