//! The usage meter under the composer: a circular context-window gauge that
//! opens a panel with the session's context occupancy and the account's
//! rate-limit lanes, mirroring Claude Code's `/usage` rows. Context numbers
//! stream in from every provider transport that reports them; plan lanes come
//! from the Claude OAuth endpoint (fetched off-thread in [`crate::usage`]),
//! OpenCode Go's usage endpoint, and Codex's own rate-limit
//! notifications. Frames read only snapshots stored on the entity.

use super::*;
use crate::ui::squircle::{SquircleStyled, squircle};
use crate::ui::{InlineNotice, ProgressBar, ProgressRing, Skeleton, StyledTypography, TextStyle};
use crate::usage::{PlanUsage, format_tokens, reset_label};

const USAGE_METER_MENU_ID: &str = "usage-meter";

use super::settings::plan_usage::PLAN_USAGE_PROVIDERS;

impl Michelle {
    /// Whether the footer shows the gauge. Always true with a session
    /// selected — an empty ring is the honest "nothing measured yet" state,
    /// and hiding it would make the control feel intermittent.
    pub(super) fn usage_meter_available(&self) -> bool {
        self.selected_session().is_some()
    }

    /// Primary modifier + U: toggle the usage panel as if its footer trigger were clicked.
    pub(super) fn toggle_usage_panel_action(
        &mut self,
        _: &ToggleUsagePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.is_some() || !self.usage_meter_available() {
            return;
        }
        let menus = self.shell_ui.menus.borrow();
        let Some(handle) = menus.get(USAGE_METER_MENU_ID).cloned() else {
            return;
        };
        // A keyboard toggle produces no mouse-down for another open menu's
        // dismiss-on-down-out to see, so close the rest here.
        let other_open: Vec<_> = menus
            .iter()
            .filter(|(id, other)| id.as_ref() != USAGE_METER_MENU_ID && other.is_open())
            .map(|(_, other)| other.clone())
            .collect();
        drop(menus);
        window.defer(cx, move |window, cx| {
            for menu in other_open {
                menu.close(window, cx);
            }
            crate::ui::menu::toggle_popover(&handle, MenuAlign::AboveRight, window, cx);
        });
    }

    /// The footer's circular context gauge plus its anchored panel. `None`
    /// while there is nothing to show for the selected session's provider.
    pub(super) fn render_usage_meter(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.usage_meter_available() {
            return None;
        }
        let session = self.selected_session()?;
        let provider = session.provider;
        let context = session.context_usage;
        let theme = Theme::current(cx);
        let plan = self.settings.plan.snapshots.get(&provider).cloned();
        let error = self.settings.plan.errors.get(&provider).cloned();
        // Fetchable but nothing cached yet: the panel shows a skeleton
        // whether the fetch is already in flight or lands on the next tick.
        let plan_loading = plan.is_none()
            && error.is_none()
            && !self.settings.plan.unconfigured.contains(&provider)
            && PLAN_USAGE_PROVIDERS.contains(&provider);

        let weak = cx.entity().downgrade();
        let handle = self.menu_handle_with(USAGE_METER_MENU_ID, cx, move |open, window, cx| {
            if open {
                let mut card_focus = None;
                let _ = weak.update(cx, |this, cx| {
                    // An opening panel wants fresh numbers; the pump honors
                    // the stale flag on its next tick once the backoff allows.
                    if let Some(provider) = this
                        .selected_session()
                        .map(|session| session.provider)
                        .filter(|provider| PLAN_USAGE_PROVIDERS.contains(provider))
                    {
                        this.settings.plan.stale.insert(provider);
                    }
                    this.maybe_refresh_plan_usage(cx);
                    card_focus = this
                        .shell_ui
                        .menus
                        .borrow()
                        .get(USAGE_METER_MENU_ID)
                        .map(|handle| handle.focus_handle().clone());
                    cx.notify();
                });
                // The card is deferred, so its focus handle joins the
                // dispatch tree only after the deferred draw — the same
                // two-frame wait the menus use. Focused, the card's menu
                // context is what lets `escape` dismiss it.
                if let Some(focus) = card_focus {
                    window.on_next_frame(move |window, _| {
                        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
                    });
                }
            } else {
                let mut composer_focus = None;
                let _ = weak.update(cx, |this, cx| {
                    composer_focus = Some(this.composer_ui.input.read(cx).focus());
                    cx.notify();
                });
                if let Some(focus) = composer_focus {
                    window.focus(&focus, cx);
                }
            }
        });

        let percent = context.and_then(context_percent);
        let fill = match percent {
            Some(percent) if percent >= 95.0 => theme.danger,
            Some(percent) if percent >= 80.0 => theme.warning,
            _ => theme.gauge,
        };
        let tooltip = match (&error, percent) {
            (Some(error), _) => SharedString::from(tr!("usage.refresh_failed", error = error)),
            (None, Some(percent)) => SharedString::from(tr!(
                "usage.context_used",
                percent = format!("{percent:.0}"),
                shortcut = crate::platform::primary_shortcut("⌘U", "Ctrl+U")
            )),
            (None, None) => SharedString::from(tr!(
                "usage.shortcut",
                shortcut = crate::platform::primary_shortcut("⌘U", "Ctrl+U")
            )),
        };

        let trigger = div()
            .id("usage-meter")
            .h(px(26.0))
            .px(px(5.0))
            .rounded(px(5.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .flex_none()
            .text_style(TextStyle::Caption)
            .text_color(theme.text_secondary)
            .cursor_default()
            .hover(|element| element.bg(theme.overlay))
            .when(handle.is_open(), |element| element.bg(theme.overlay_strong))
            .tooltip(Tooltip::text(tooltip))
            .child(
                ProgressRing::new(percent.map(|percent| percent / 100.0))
                    .track_color(theme.border_strong)
                    .fill_color(fill)
                    .minimum_fill(0.05),
            )
            .child(SharedString::from(context_usage_label(context)));

        Some(popover(
            trigger,
            &handle,
            MenuAlign::AboveRight,
            move |handle, _, cx| {
                usage_panel(
                    handle,
                    provider,
                    context,
                    plan.clone(),
                    error.as_deref(),
                    plan_loading,
                    cx,
                )
            },
        ))
    }
}

/// Fraction of the window in use, capped at 100%. A provider (notably
/// OpenCode) can report a lifetime/cumulative token counter that exceeds the
/// model's window; the context can never be more than fully used, so anything
/// past the window reads as full rather than an impossible percentage.
fn context_percent(usage: ContextUsage) -> Option<f64> {
    usage
        .window
        .filter(|window| *window > 0)
        .map(|window| (usage.tokens as f64 * 100.0 / window as f64).min(100.0))
}

fn context_usage_label(usage: Option<ContextUsage>) -> String {
    match usage {
        Some(usage) => match context_percent(usage) {
            Some(percent) => format!("{percent:.0}%"),
            None if usage.tokens > 0 => "—".to_owned(),
            None => "0%".to_owned(),
        },
        None => "0%".to_owned(),
    }
}

fn usage_panel(
    handle: &ContextMenuHandle,
    provider: ProviderKind,
    context: Option<ContextUsage>,
    plan: Option<PlanUsage>,
    error: Option<&str>,
    plan_loading: bool,
    cx: &App,
) -> AnyElement {
    let theme = Theme::current(cx);
    let now = unix_time() as i64;
    let mut panel = div()
        // Focused on open so the surrounding menu context sees `escape`.
        .track_focus(handle.focus_handle())
        .w(px(320.0))
        .p(px(14.0))
        .child(
            squircle()
                .rounded(px(12.0))
                .bg(theme.raised)
                .border(px(0.5))
                .border_color(theme.border_strong)
                .border_inside()
                .absolute_expand(),
        )
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .text_size(sp(12.5));

    // The context row always renders; a session with nothing measured yet
    // reads "0" over an empty track, exactly like the CLI's own panel.
    let usage = context.unwrap_or_default();
    let percent = context_percent(usage);
    let value = match (usage.window, percent) {
        (Some(window), Some(percent)) => format!(
            "{} / {} ({percent:.0}%)",
            format_tokens(usage.tokens.min(window)),
            format_tokens(window)
        ),
        // The transport reports occupancy but not the window size.
        _ => format_tokens(usage.tokens),
    };
    panel = panel.child(
        div()
            .flex()
            .flex_col()
            .gap(px(7.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(theme.text)
                            .child(tr!("usage.context_window")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(sp(12.5))
                            .text_color(theme.text_tertiary)
                            .child(SharedString::from(value)),
                    ),
            )
            .child(meter_bar(&theme, percent)),
    );
    if plan.is_some() || error.is_some() || plan_loading {
        panel = panel.child(div().h(px(1.0)).flex_none().bg(theme.border));
    }

    if let Some(plan) = plan {
        let header = match &plan.plan_label {
            Some(label) => tr!("usage.plan_limits_named", plan = label),
            None => tr!("usage.plan_limits"),
        };
        let usage_url = match provider {
            ProviderKind::Claude => Some("https://claude.ai/settings/usage"),
            ProviderKind::Codex => Some("https://chatgpt.com/codex/settings/usage"),
            _ => None,
        };
        let header_row = div().flex().items_center().gap(px(6.0)).child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(sp(12.5))
                .text_color(theme.text_tertiary)
                .child(SharedString::from(header)),
        );
        panel = panel.child(match usage_url {
            Some(url) => header_row
                .id("plan-usage-link")
                .cursor_default()
                .hover(|element| element.opacity(0.8))
                .tooltip(Tooltip::text(tr!("usage.open_account_settings")))
                .on_click(move |_, _, cx| cx.open_url(url))
                .child(icon("arrow.right", 10.0, theme.text_tertiary))
                .into_any_element(),
            None => header_row.into_any_element(),
        });
        for window in &plan.windows {
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(7.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                // A long lane label gives way — truncated
                                // with an ellipsis — rather than pushing the
                                // reset time and percent past the card edge.
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(SharedString::from(window.label.clone())),
                            )
                            .children(window.resets_at.map(|resets_at| {
                                div()
                                    .flex_none()
                                    .text_size(sp(12.5))
                                    .text_color(theme.text_tertiary)
                                    .child(SharedString::from(reset_label(resets_at, now)))
                            }))
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(sp(12.5))
                                    .text_color(theme.text_secondary)
                                    .child(SharedString::from(format!("{:.0}%", window.percent))),
                            ),
                    )
                    .child(meter_bar(&theme, Some(window.percent))),
            );
        }
    } else if plan_loading {
        panel = panel.child(plan_skeleton(&theme));
    } else if let Some(error) = error {
        panel = panel.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_tertiary)
                        .child(tr!("usage.plan_limits")),
                )
                .child(
                    InlineNotice::new(tr!("usage.unavailable", error = error))
                        .p(px(0.0))
                        .border_0()
                        .text_color(theme.text_secondary),
                ),
        );
    }

    panel.into_any_element()
}

/// The owner keeps the plan silhouette; one shared pulse animates its blocks.
fn plan_skeleton(theme: &Theme) -> AnyElement {
    let bar = |width: f32| Skeleton::new().w(px(width)).bg(theme.overlay_strong);
    let row = |label_width: f32, value_width: f32| {
        div()
            .flex()
            .flex_col()
            .gap(px(9.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(bar(label_width))
                    .child(div().flex_1())
                    .child(bar(value_width)),
            )
            .child(
                Skeleton::new()
                    .h(px(3.0))
                    .rounded_full()
                    .bg(theme.overlay_strong),
            )
    };
    Skeleton::pulse(
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(bar(132.0))
            .child(row(96.0, 64.0))
            .child(row(120.0, 64.0)),
    )
    .into_any_element()
}

/// Quota thresholds and the minimum visible sliver remain presentation policy.
fn meter_bar(theme: &Theme, percent: Option<f64>) -> ProgressBar {
    let fill = match percent {
        Some(percent) if percent >= 95.0 => theme.danger,
        Some(percent) if percent >= 80.0 => theme.warning,
        _ => theme.gauge,
    };
    ProgressBar::new(percent.map(|percent| percent / 100.0))
        .track_color(theme.overlay_strong)
        .fill_color(fill)
        .minimum_fill(if percent.is_some_and(|percent| percent > 0.0) {
            0.015
        } else {
            0.0
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn quota_bars_keep_thresholds_and_the_minimum_visible_positive_fill(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Harness {
            percent: Option<f64>,
        }
        impl Render for Harness {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                crate::ui::squircle::test_support::clear(window, cx);
                div()
                    .w(px(100.0))
                    .child(meter_bar(&Theme::dark(), self.percent))
            }
        }
        let theme = Theme::dark();
        let (view, cx) = cx.add_window_view(|_, _| Harness { percent: None });
        for (percent, color, width) in [
            (Some(0.1), theme.gauge, 1.5),
            (Some(79.0), theme.gauge, 79.0),
            (Some(80.0), theme.warning, 80.0),
            (Some(95.0), theme.danger, 95.0),
            (Some(120.0), theme.danger, 100.0),
        ] {
            view.update(cx, |view, cx| {
                view.percent = percent;
                cx.notify();
            });
            cx.update(|window, cx| {
                let quad = crate::ui::squircle::test_support::painted_surfaces(window, cx)
                    .into_iter()
                    .find(|quad| quad.background.as_solid() == Some(color))
                    .unwrap();
                assert!((quad.bounds.size.width.0 / window.scale_factor() - width).abs() < 0.01);
            });
        }
        for percent in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            view.update(cx, |view, cx| {
                view.percent = percent;
                cx.notify();
            });
            cx.update(|window, cx| {
                assert!(
                    !crate::ui::squircle::test_support::painted_surfaces(window, cx)
                        .iter()
                        .any(|quad| {
                            quad.bounds.size.width.0 > 0.0
                                && quad.background.as_solid().is_some_and(|color| {
                                    [theme.gauge, theme.warning, theme.danger].contains(&color)
                                })
                        })
                )
            });
        }
    }

    #[test]
    fn context_usage_label_distinguishes_empty_unknown_and_measured_context() {
        assert_eq!(context_usage_label(None), "0%");
        assert_eq!(context_usage_label(Some(ContextUsage::default())), "0%");
        assert_eq!(
            context_usage_label(Some(ContextUsage {
                tokens: 250_000,
                window: None,
            })),
            "—"
        );
        assert_eq!(
            context_usage_label(Some(ContextUsage {
                tokens: 250_000,
                window: Some(1_000_000),
            })),
            "25%"
        );
    }

    #[test]
    fn context_percent_caps_at_full_when_provider_over_reports() {
        // OpenCode reports lifetime/cumulative session counters that can run
        // far past the model's window; the meter must read "full", never an
        // impossible percentage like 472%.
        let usage = ContextUsage {
            tokens: 4_718_002,
            window: Some(1_000_000),
        };
        assert_eq!(context_percent(usage), Some(100.0));
    }

    #[test]
    fn context_percent_reports_partial_usage_verbatim() {
        let usage = ContextUsage {
            tokens: 250_000,
            window: Some(1_000_000),
        };
        assert_eq!(context_percent(usage), Some(25.0));
    }

    #[test]
    fn context_percent_is_none_without_a_positive_window() {
        assert_eq!(
            context_percent(ContextUsage {
                tokens: 4_718_002,
                window: None,
            }),
            None
        );
        assert_eq!(
            context_percent(ContextUsage {
                tokens: 4_718_002,
                window: Some(0),
            }),
            None
        );
    }
}
