use gpui::transparent_black;

use super::*;
use crate::ui::primitives::navigation::next_picker_highlight;
use crate::ui::{
    ActivationExt, Badge, Button, ButtonStyle, Divider, FormRow, IconButton, InfoRow, Label,
    Picker, SearchField, Section, StatusTone, Surface, Toggle, v_stack,
};

pub(super) mod model;
pub(super) mod permissions;
pub(super) mod plan_usage;
pub(super) mod providers;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SettingsPage {
    General,
    Providers,
    Skills,
    Usage,
    Daemon,
    ComputerUse,
    Appearance,
}

impl SettingsPage {
    /// Keep the Settings sidebar and command palette visibility in sync.
    pub(super) fn is_visible_in_navigation(self) -> bool {
        self != Self::ComputerUse || crate::computer_use::is_available()
    }
}

/// Focus, form fields and navigation for the Settings interface.
pub(super) struct SettingsUi {
    pub(in crate::app) page: Option<SettingsPage>,
    pub(in crate::app) search: Entity<TextInput>,
    pub(in crate::app) daemon_port_input: Entity<TextInput>,
    pub(in crate::app) daemon_origins_input: Entity<TextInput>,
    pub(in crate::app) daemon_token_revealed: bool,
    pub(in crate::app) focus: FocusHandle,
    pub(in crate::app) sidebar_focuses: RefCell<HashMap<SharedString, FocusHandle>>,
    pub(in crate::app) expanded_provider: Option<ProviderKind>,
    pub(in crate::app) provider_path_input: Entity<TextInput>,
    pub(in crate::app) scroll: ScrollHandle,
    pub(in crate::app) scrollbar: Rc<ScrollbarState>,
}

use crate::ui::SidebarItem;

const SETTINGS_CONTENT_MAX_WIDTH: f32 = 760.0;

/// The Usage page is a dashboard, not a form; it mirrors T3 Code's wide
/// two-column layout and needs the extra room for the chart.
const SETTINGS_USAGE_MAX_WIDTH: f32 = 1024.0;

/// Key context the settings sidebar declares around its search field.
const SETTINGS_SIDEBAR_CONTEXT: &str = "SettingsSidebar";

/// The search field while focused inside the sidebar. The field holds real
/// focus the whole time — the sidebar's selection is only drawn — so `up` and
/// `down` have to be claimed from under it, and only a binding can do that:
/// they arrive as actions, which consume the keystroke before the field sees
/// it.
const SETTINGS_SEARCH_CONTEXT: &str = "SettingsSidebar > TextInput";

/// The sidebar's rows in display order, each with the keyword haystack the
/// search field filters against.
const SETTINGS_PAGES: [(SettingsPage, &str, &str, &str); 7] = [
    (
        SettingsPage::General,
        "settings.general",
        "gearshape",
        "settings.general_keywords",
    ),
    (
        SettingsPage::Appearance,
        "settings.appearance",
        "circle.lefthalf.filled",
        "settings.appearance_keywords",
    ),
    (
        SettingsPage::Providers,
        "settings.providers",
        "cpu",
        "settings.providers_keywords",
    ),
    (
        SettingsPage::Skills,
        "settings.skills",
        "shippingbox",
        "settings.skills_keywords",
    ),
    (
        SettingsPage::Usage,
        "settings.usage",
        "chart.bar",
        "settings.usage_keywords",
    ),
    (
        SettingsPage::Daemon,
        "settings.daemon",
        "server.rack",
        "settings.daemon_keywords",
    ),
    (
        SettingsPage::ComputerUse,
        "settings.computer_use",
        "cursorarrow.rays",
        "settings.computer_use_keywords",
    ),
];

/// Bind the search field's list-navigation keys. Called once at startup.
pub fn init(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("down", SelectNextEntry, Some(SETTINGS_SEARCH_CONTEXT)),
        KeyBinding::new("up", SelectPreviousEntry, Some(SETTINGS_SEARCH_CONTEXT)),
    ]);
}

/// The sidebar rows the query leaves visible, in display order. `query` must
/// already be trimmed and lowercased; when it is empty every page matches.
pub(super) fn visible_settings_pages(
    query: &str,
) -> impl Iterator<Item = (SettingsPage, String, &'static str)> + '_ {
    SETTINGS_PAGES
        .into_iter()
        .filter(|(page, ..)| page.is_visible_in_navigation())
        .filter_map(move |(page, label_key, icon, keywords_key)| {
            let label = crate::i18n::translate(label_key);
            let keywords = crate::i18n::translate(keywords_key).to_lowercase();
            (query.is_empty() || keywords.contains(query)).then_some((page, label, icon))
        })
}

fn settings_navigation_target(
    pages: &[SettingsPage],
    current: SettingsPage,
    key: &str,
) -> Option<SettingsPage> {
    match key {
        "home" => pages.first().copied(),
        "end" => pages.last().copied(),
        _ => {
            let current = pages.iter().position(|page| *page == current);
            next_picker_highlight(current, pages.len(), key).map(|index| pages[index])
        }
    }
}

impl Michelle {
    /// Switch the settings view to `page`, warming the Usage scan when that
    /// is where the user is heading.
    pub(super) fn open_settings_page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        // Secrets are revealed only for the current visit to the page. This
        // also masks the token again when the Daemon row is reselected.
        self.settings_ui.daemon_token_revealed = false;
        self.settings_ui.page = Some(page);
        // Each page starts at its own top; a scroll position carried over
        // from the previous page would land mid-content.
        self.settings_ui.scroll.set_offset(gpui::Point::default());
        if page == SettingsPage::Usage {
            self.ensure_usage_history(false, cx);
        }
        if page == SettingsPage::Skills {
            self.ensure_skills_catalog(false, cx);
        }
        cx.notify();
    }

    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);

        div()
            .key_context("Michelle")
            .track_focus(&self.settings_ui.focus)
            .on_action(|_: &CloseWindow, window, _| crate::platform::hide_window(window))
            .on_action(cx.listener(Self::new_session_action))
            .on_action(cx.listener(Self::new_project_action))
            .on_action(cx.listener(Self::open_settings_action))
            .on_action(cx.listener(Self::toggle_sidebar_action))
            .on_action(cx.listener(Self::toggle_right_panel_action))
            .on_action(cx.listener(Self::open_terminal_action))
            .on_action(cx.listener(Self::toggle_command_palette_action))
            .on_action(cx.listener(Self::toggle_fps_counter_action))
            .on_action(cx.listener(Self::navigate_back_action))
            .on_action(cx.listener(Self::navigate_forward_action))
            .on_action(cx.listener(Self::focus_composer_action))
            .on_action(cx.listener(Self::focus_sidebar_action))
            .on_action(cx.listener(Self::cancel_turn_action))
            .capture_any_mouse_down(cx.listener(Self::navigation_mouse_down))
            .size_full()
            .flex()
            .bg(theme.canvas)
            .text_color(theme.text)
            .font_family(".SystemUIFont")
            .text_style(TextStyle::BODY)
            .child(self.render_settings_sidebar(cx))
            .child(self.render_settings_content(cx))
            .into_any_element()
    }

    fn render_settings_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let current_page = self.settings_ui.page.unwrap_or(SettingsPage::General);
        let query = self.settings_search_query(cx);
        let mut navigation = div().flex().flex_col().gap(px(3.0));
        let focus_for = |id: SharedString, cx: &mut App| {
            self.settings_ui
                .sidebar_focuses
                .borrow_mut()
                .entry(id)
                .or_insert_with(|| cx.focus_handle())
                .clone()
        };
        let back_focus = focus_for("settings-back".into(), cx);

        for (page, label, icon_path) in visible_settings_pages(&query) {
            let selected = current_page == page;
            let id = SharedString::from(format!("settings-tab-{page:?}"));
            let focus = focus_for(id.clone(), cx);
            navigation = navigation.child(
                SidebarItem::new(id, icon_path, label)
                    .selected(selected)
                    .track_focus(&focus)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let query = this.settings_search_query(cx);
                        let pages = visible_settings_pages(&query)
                            .map(|(page, ..)| page)
                            .collect::<Vec<_>>();
                        let Some(next) =
                            settings_navigation_target(&pages, page, &event.keystroke.key)
                        else {
                            return;
                        };
                        this.open_settings_page(next, cx);
                        let id = SharedString::from(format!("settings-tab-{next:?}"));
                        let focus = this.settings_ui.sidebar_focuses.borrow().get(&id).cloned();
                        if let Some(focus) = focus {
                            window.focus(&focus, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .on_activation(cx, move |this, _, cx| {
                        this.open_settings_page(page, cx);
                    }),
            );
        }

        div()
            .key_context(SETTINGS_SIDEBAR_CONTEXT)
            .on_action(cx.listener(|this, _: &SelectNextEntry, _, cx| {
                this.cycle_settings_page("down", cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousEntry, _, cx| {
                this.cycle_settings_page("up", cx);
            }))
            .w(px(DEFAULT_SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(transparent_black())
            .child(self.render_settings_sidebar_titlebar(cx))
            .child(
                div().px(px(12.0)).child(
                    SidebarItem::new("settings-back", "arrow.left", tr!("settings.back"))
                        .track_focus(&back_focus)
                        .on_activation(cx, |this, window, cx| {
                            this.settings_ui.page = None;
                            let focus_handle = this.composer_focus(cx);
                            window.focus(&focus_handle, cx);
                            cx.notify();
                        }),
                ),
            )
            .child(div().px(px(12.0)).pt(px(8.0)).child(SearchField::new(
                "settings-search-field",
                self.settings_ui.search.clone(),
            )))
            .child(div().h(px(18.0)))
            .child(div().px(px(12.0)).tab_group().child(navigation))
    }

    /// The search field's content, normalized the way the page filter expects.
    fn settings_search_query(&self, cx: &App) -> String {
        self.settings_ui
            .search
            .read(cx)
            .content()
            .trim()
            .to_lowercase()
    }

    /// Step the selected page through the rows the search leaves visible,
    /// wrapping at both ends. The field keeps focus so typing keeps narrowing
    /// the list; the landing page renders immediately, so there is no separate
    /// confirm step. A selection filtered out by the query re-enters the list
    /// from whichever end matches the key.
    fn cycle_settings_page(&mut self, key: &str, cx: &mut Context<Self>) {
        let query = self.settings_search_query(cx);
        let pages = visible_settings_pages(&query)
            .map(|(page, ..)| page)
            .collect::<Vec<_>>();
        let current_page = self.settings_ui.page.unwrap_or(SettingsPage::General);
        let Some(next) = settings_navigation_target(&pages, current_page, key) else {
            return;
        };
        self.open_settings_page(next, cx);
    }

    fn render_settings_sidebar_titlebar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("settings-sidebar-titlebar")
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .child(
                self.window_drag_region(
                    div()
                        .id("settings-sidebar-traffic-light-drag-region")
                        .w(px(TRAFFIC_LIGHT_CLEARANCE))
                        .h_full()
                        .flex_none(),
                    cx,
                ),
            )
            .child(
                self.render_settings_drag_region("settings-sidebar-titlebar-drag-region", cx)
                    .flex_1(),
            )
    }

    fn render_settings_content(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let page = self.settings_ui.page.unwrap_or(SettingsPage::General);
        // The Skills page is a mail-style split that owns the whole content
        // column — no page title, no titlebar strip, no width cap, no card.
        // Window dragging stays with the sidebar's own titlebar region.
        if page == SettingsPage::Skills {
            return div()
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(theme.sidebar_border)
                .bg(theme.surface)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_skills_settings(cx)),
                );
        }
        // The Monthly and Projects list views own their own scrolling, so
        // their pages fill the viewport instead of riding the shared scroll
        // container.
        let fills_viewport = page == SettingsPage::Usage
            && matches!(
                self.usage_ui.view,
                UsageViewMode::Monthly | UsageViewMode::Projects
            );
        // The titlebar strip is transparent; once content slides under it, a
        // hairline marks the boundary so the clip edge reads as a header
        // rather than a glitch.
        let content_scrolled = !fills_viewport && self.settings_ui.scroll.offset().y < px(-1.0);

        let inner = div()
            .w_full()
            .max_w(px(match page {
                SettingsPage::Usage => SETTINGS_USAGE_MAX_WIDTH,
                _ => SETTINGS_CONTENT_MAX_WIDTH,
            }))
            .mx_auto()
            .when(fills_viewport, |element| {
                element.h_full().min_h_0().flex().flex_col()
            })
            .child(
                div()
                    .pt(px(2.0))
                    .flex_none()
                    .text_size(sp(18.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(match page {
                        SettingsPage::General => tr!("settings.general"),
                        SettingsPage::Providers => tr!("settings.providers"),
                        SettingsPage::Skills => tr!("settings.skills"),
                        SettingsPage::Usage => tr!("settings.usage"),
                        SettingsPage::Daemon => tr!("settings.daemon"),
                        SettingsPage::ComputerUse => tr!("settings.computer_use"),
                        SettingsPage::Appearance => tr!("settings.appearance"),
                    }),
            )
            .child(match page {
                SettingsPage::General => self.render_general_settings(cx),
                SettingsPage::Providers => self.render_providers_settings(cx),
                SettingsPage::Skills => self.render_skills_settings(cx),
                SettingsPage::Usage => self.render_usage_settings(cx),
                SettingsPage::Daemon => self.render_daemon_settings(cx),
                SettingsPage::ComputerUse => self.render_computer_use_settings(cx),
                SettingsPage::Appearance => self.render_appearance_settings(cx),
            });

        div()
            .flex_1()
            .h_full()
            .min_w_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme.sidebar_border)
            .bg(theme.surface)
            .child(
                self.render_settings_drag_region("settings-content-titlebar", cx)
                    .flex()
                    .items_center()
                    .justify_end()
                    .when(content_scrolled, |element| {
                        element.border_b_1().border_color(theme.border)
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("settings-content-scroll")
                            .size_full()
                            .when(!fills_viewport, |element| {
                                element
                                    .overflow_y_scroll()
                                    .track_scroll(&self.settings_ui.scroll)
                                    .pb(px(48.0))
                            })
                            .when(fills_viewport, |element| {
                                element.min_h_0().flex().flex_col()
                            })
                            .px(px(32.0))
                            .child(inner),
                    )
                    .when(!fills_viewport, |element| {
                        element.child(scrollbar::vertical(
                            &self.settings_ui.scroll,
                            &self.settings_ui.scrollbar,
                        ))
                    }),
            )
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        v_stack()
            .child(
                Surface::new().mt(px(15.0)).px(px(20.0)).py(px(14.0)).child(
                    Section::new(tr!("settings.local_by_default"))
                        .description(tr!("settings.local_by_default_description")),
                ),
            )
            .child(
                Surface::new().mt(px(15.0)).child(
                    FormRow::new(tr!("settings.render_math"))
                        .description(tr!("settings.render_math_description"))
                        .control(
                            Toggle::new("render-math-toggle", self.state.render_math)
                                .on_change(cx, |this, enabled, _, cx| {
                                    this.set_render_math(enabled, cx)
                                }),
                        ),
                ),
            )
            .into_any_element()
    }

    fn render_daemon_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        if self.daemon.is_remote() {
            return Surface::new()
                .mt(px(15.0))
                .px(px(20.0))
                .py(px(16.0))
                .child(
                    Section::new(tr!("daemon.external_title"))
                        .description(tr!("daemon.external_description")),
                )
                .into_any_element();
        }
        let enabled = self.state.daemon_exposure.enabled;
        let pending = self.settings.daemon_reconfigure_pending;
        let fields_dirty = self.daemon_exposure_fields_dirty(cx);
        let port = self.state.daemon_exposure.port;
        let websocket_url = format!("ws://{}:{port}", self.settings.daemon_hostname);
        let token = self.state.daemon_exposure.token.clone();
        let token_revealed = self.settings_ui.daemon_token_revealed;
        let compact_button = |id: &'static str, label: String| {
            Button::new(id, label)
                .button_style(ButtonStyle::Ghost)
                .h(px(27.0))
                .px(px(9.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border_strong)
                .text_size(px(12.5))
                .text_color(theme.text_secondary)
                .gap(px(5.0))
                .hover(|style| style.bg(theme.overlay))
        };
        let apply_button = compact_button(
            "apply-daemon-settings",
            if pending {
                tr!("daemon.restarting")
            } else {
                tr!("daemon.apply")
            },
        )
        .h(px(29.0))
        .px(px(11.0))
        .rounded(px(7.0))
        .disabled(pending || !fields_dirty)
        .on_activation(cx, |this, _, cx| this.apply_daemon_exposure_fields(cx));
        let url_copied = self.control_was_copied("daemon-url");
        let copy_url = websocket_url.clone();
        let copy_url_button = compact_button(
            "copy-daemon-url",
            if url_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            },
        )
        .icon(icon(
            if url_copied {
                "checkmark"
            } else {
                "doc.on.doc"
            },
            11.0,
            theme.text_tertiary,
        ))
        .on_activation(cx, move |this, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
            this.show_control_copied("daemon-url", cx);
        });
        let token_copied = self.control_was_copied("daemon-token");
        let copy_token = token.clone();
        let copy_token_button = compact_button(
            "copy-daemon-token",
            if token_copied {
                tr!("common.copied")
            } else {
                tr!("common.copy")
            },
        )
        .icon(icon(
            if token_copied {
                "checkmark"
            } else {
                "doc.on.doc"
            },
            11.0,
            theme.text_tertiary,
        ))
        .on_activation(cx, move |this, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_token.clone()));
            this.show_control_copied("daemon-token", cx);
        });
        let reveal_token_button = IconButton::new(
            "reveal-daemon-token",
            icon(
                if token_revealed { "eye.slash" } else { "eye" },
                12.0,
                theme.text_tertiary,
            ),
        )
        .w(px(27.0))
        .h(px(27.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border_strong)
        .tooltip(if token_revealed {
            tr!("daemon.hide_token")
        } else {
            tr!("daemon.reveal_token")
        })
        .on_activation(cx, |this, _, cx| {
            this.settings_ui.daemon_token_revealed = !this.settings_ui.daemon_token_revealed;
            cx.notify();
        });
        let regenerate_button =
            compact_button("regenerate-daemon-token", tr!("daemon.regenerate_token"))
                .disabled(pending)
                .on_activation(cx, |this, _, cx| this.regenerate_daemon_token(cx));
        let mono_value = |value: String| {
            div()
                .min_w_0()
                .truncate()
                .font_family(".SystemUIFontMonospaced")
                .text_size(px(12.5))
                .text_color(theme.text)
                .child(SharedString::from(value))
        };
        v_stack()
            .mt(px(15.0))
            .w_full()
            .gap(px(12.0))
            .child(
                Surface::new().child(
                    FormRow::new(tr!("daemon.expose_title"))
                        .label_content(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .child(
                                    Label::new(tr!("daemon.expose_title"))
                                        .text_style(TextStyle::HEADLINE),
                                )
                                .child(
                                    Badge::new(if pending {
                                        tr!("daemon.status_restarting")
                                    } else if enabled {
                                        tr!("daemon.status_exposed")
                                    } else {
                                        tr!("daemon.status_local")
                                    })
                                    .tone(if enabled {
                                        StatusTone::Success
                                    } else {
                                        StatusTone::Neutral
                                    }),
                                ),
                        )
                        .description(tr!("daemon.expose_description"))
                        .min_h(px(66.0))
                        .py(px(13.0))
                        .control(
                            Toggle::new("daemon-exposure-toggle", enabled)
                                .disabled(pending)
                                .on_change(cx, |this, enabled, _, cx| {
                                    this.set_daemon_exposure_enabled(enabled, cx)
                                }),
                        ),
                ),
            )
            .when(enabled, |column| {
                column.child(
                    Surface::new()
                        .px(px(20.0))
                        .py(px(15.0))
                        .child(
                            Section::new(tr!("daemon.connection_title"))
                                .description(tr!("daemon.connection_description"))
                                .gap(px(4.0)),
                        )
                        .child(
                            FormRow::new(tr!("daemon.port"))
                                .description(tr!("daemon.port_description"))
                                .mt(px(14.0))
                                .min_h(px(0.0))
                                .p(px(0.0))
                                .items_start()
                                .control(
                                    div().flex_1().min_w_0().flex().justify_end().child(
                                        TextField::new(
                                            "daemon-port-field",
                                            self.settings_ui.daemon_port_input.clone(),
                                        )
                                        .w(px(150.0)),
                                    ),
                                ),
                        )
                        .child(
                            FormRow::new(tr!("daemon.allowed_origins"))
                                .description(tr!("daemon.allowed_origins_description"))
                                .mt(px(14.0))
                                .min_h(px(0.0))
                                .p(px(0.0))
                                .items_start()
                                .control(
                                    div().flex_1().min_w_0().flex().justify_end().child(
                                        TextField::new(
                                            "daemon-origins-field",
                                            self.settings_ui.daemon_origins_input.clone(),
                                        )
                                        .w_full()
                                        .max_w(px(360.0)),
                                    ),
                                ),
                        )
                        .child(div().mt(px(13.0)).flex().justify_end().child(apply_button)),
                )
            })
            .when(enabled, |column| {
                column.child(
                    Surface::new()
                        .px(px(20.0))
                        .py(px(15.0))
                        .child(
                            Section::new(tr!("daemon.credentials_title"))
                                .description(tr!("daemon.credentials_description"))
                                .gap(px(4.0)),
                        )
                        .child(
                            InfoRow::new(tr!("daemon.websocket_url"), mono_value(websocket_url))
                                .label_width(px(80.0))
                                .mt(px(13.0))
                                .items_center()
                                .gap(px(10.0))
                                .child(copy_url_button),
                        )
                        .child(
                            InfoRow::new(
                                tr!("daemon.token"),
                                mono_value(if token_revealed {
                                    token
                                } else {
                                    "••••••••••••••••••••••••••••••••".to_owned()
                                }),
                            )
                            .label_width(px(80.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .items_center()
                            .gap(px(10.0))
                            .child(reveal_token_button)
                            .child(copy_token_button)
                            .child(regenerate_button),
                        )
                        .child(
                            div()
                                .mt(px(7.0))
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(8.0))
                                .bg(theme.inset)
                                .w_full()
                                .min_w_0()
                                .flex()
                                .gap(px(8.0))
                                .child(icon("exclamationmark.triangle", 13.0, theme.warning))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.5))
                                        .line_height(px(16.0))
                                        .text_color(theme.text_secondary)
                                        .child(tr!("daemon.security_warning")),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_scheme = self.state.theme;
        let selected_language = self.state.language;
        let weak = cx.entity().downgrade();
        let scheme_handle = self.menu_handle("color-scheme-selector", cx);
        let scheme_selector = Picker::new(
            "color-scheme-selector",
            selected_scheme.label(),
            "color-scheme-selector-menu",
            &scheme_handle,
            move |_| {
                ThemePreference::ALL
                    .into_iter()
                    .map(|preference| {
                        let weak = weak.clone();
                        MenuItem::new(preference.label(), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_color_scheme(preference, window, cx);
                            });
                        })
                        .selected(preference == selected_scheme)
                    })
                    .collect()
            },
        )
        .w(px(116.0));

        let theme_selector = (self.settings.themes.len() > 1).then(|| {
            let selected_theme = self.selected_theme();
            let selected_file = selected_theme.file.clone();
            let themes = self.settings.themes.clone();
            let weak = cx.entity().downgrade();
            let handle = self.menu_handle("theme-selector", cx);
            Picker::new(
                "theme-selector",
                selected_theme.name.clone(),
                "theme-selector-menu",
                &handle,
                move |_| {
                    themes
                        .iter()
                        .map(|theme| {
                            let weak = weak.clone();
                            let file = theme.file.clone();
                            MenuItem::new(theme.name.clone(), move |window, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.set_theme(file.clone(), window, cx);
                                });
                            })
                            .selected(theme.file == selected_file)
                        })
                        .collect()
                },
            )
            .min_w(px(116.0))
            .max_w(px(260.0))
        });

        let selected_content_font_size = self.state.content_font_size;
        let weak = cx.entity().downgrade();
        let content_font_size_handle = self.menu_handle("content-font-size-selector", cx);
        let content_font_size_selector = Picker::new(
            "content-font-size-selector",
            font_size_label(selected_content_font_size),
            "content-font-size-selector-menu",
            &content_font_size_handle,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_content_font_size(size, cx);
                            });
                        })
                        .selected(size == selected_content_font_size)
                    })
                    .collect()
            },
        )
        .w(px(116.0));

        let selected_code_font_size = self.state.code_font_size;
        let weak = cx.entity().downgrade();
        let code_font_size_handle = self.menu_handle("code-font-size-selector", cx);
        let code_font_size_selector = Picker::new(
            "code-font-size-selector",
            font_size_label(selected_code_font_size),
            "code-font-size-selector-menu",
            &code_font_size_handle,
            move |_| {
                FONT_SIZES
                    .into_iter()
                    .map(|size| {
                        let weak = weak.clone();
                        MenuItem::new(font_size_label(size), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_code_font_size(size, cx);
                            });
                        })
                        .selected(size == selected_code_font_size)
                    })
                    .collect()
            },
        )
        .w(px(116.0));

        let weak = cx.entity().downgrade();
        let language_handle = self.menu_handle("language-selector", cx);
        let language_selector = Picker::new(
            "language-selector",
            selected_language.label(),
            "language-selector-menu",
            &language_handle,
            move |_| {
                crate::i18n::AppLanguage::ALL
                    .into_iter()
                    .map(|language| {
                        let weak = weak.clone();
                        MenuItem::new(language.label(), move |window, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_language(language, window, cx);
                            });
                        })
                        .selected(language == selected_language)
                    })
                    .collect()
            },
        )
        .w(px(116.0));

        Surface::new()
            .mt(px(15.0))
            .when_some(theme_selector, |surface, selector| {
                surface
                    .child(
                        FormRow::new(tr!("settings.theme"))
                            .description(tr!("settings.theme_description"))
                            .control(selector),
                    )
                    .child(Divider::horizontal().inset(px(20.0)))
            })
            .child(
                FormRow::new(tr!("settings.color_scheme"))
                    .description(tr!("settings.color_scheme_description"))
                    .control(scheme_selector),
            )
            .child(Divider::horizontal().inset(px(20.0)))
            .child(
                FormRow::new(tr!("language.title"))
                    .description(tr!("language.description"))
                    .control(language_selector),
            )
            .child(Divider::horizontal().inset(px(20.0)))
            .child(
                FormRow::new(tr!("settings.content_font_size"))
                    .description(tr!("settings.content_font_size_description"))
                    .control(content_font_size_selector),
            )
            .child(Divider::horizontal().inset(px(20.0)))
            .child(
                FormRow::new(tr!("settings.code_font_size"))
                    .description(tr!("settings.code_font_size_description"))
                    .control(code_font_size_selector),
            )
            .into_any_element()
    }

    fn render_providers_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let checking = self.settings.providers.detection_remaining > 0;
        let checked_label = self
            .settings
            .providers
            .detection_checked_at
            .filter(|_| !checking)
            .map(|checked_at| detection_checked_label(checked_at.elapsed()));

        let refresh = Button::new(
            "refresh-providers",
            if checking {
                tr!("common.checking")
            } else {
                tr!("common.refresh")
            },
        )
        .button_style(ButtonStyle::Ghost)
        .icon(icon("arrow.clockwise", 11.0, theme.text_tertiary))
        .loading(checking)
        .disabled(checking)
        .h(px(28.0))
        .px(px(11.0))
        .rounded(px(7.0))
        .border_color(theme.border_strong)
        .text_color(theme.text_secondary)
        .hover(|element| element.bg(theme.overlay))
        .on_activation(cx, |this, _, cx| {
            this.refresh_provider_detection(None);
            cx.notify();
        });

        let mut rows = div().mt(px(4.0)).flex().flex_col();
        let provider_count = ProviderKind::ALL.len();
        for (index, kind) in ProviderKind::ALL.into_iter().enumerate() {
            let probe = self.provider_probe(kind);
            let installed = probe.is_some_and(|probe| probe.installed);
            let binary_path = probe
                .filter(|probe| probe.installed)
                .and_then(|probe| probe.path.as_deref())
                .map(|path| abbreviate_home_path(path, self.shell_model.home_directory.as_deref()));
            let model_count = probe.map(|probe| probe.models.len()).unwrap_or(0);
            let version = self
                .settings
                .providers
                .versions
                .get(&kind)
                .and_then(|version| version.clone());
            let disabled = self.state.disabled_providers.contains(&kind);

            let dot_color = if !installed {
                theme.text_ghost
            } else if disabled {
                theme.warning
            } else {
                theme.success
            };

            let detail: AnyElement = if installed {
                let mut parts = Vec::new();
                if let Some(path) = binary_path {
                    parts.push(path);
                }
                if disabled {
                    parts.push(tr!("providers.disabled_for_new_tasks"));
                } else if model_count > 0 {
                    parts.push(if model_count == 1 {
                        tr!("providers.model_count_one", count = model_count)
                    } else {
                        tr!("providers.model_count_many", count = model_count)
                    });
                }
                div()
                    .truncate()
                    .child(SharedString::from(parts.join("  ·  ")))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .items_baseline()
                    .child(SharedString::from(tr!(
                        "providers.not_detected_as",
                        command = kind.command()
                    )))
                    .into_any_element()
            };

            let toggle_on = !disabled;
            let toggle = Toggle::new(
                SharedString::from(format!("provider-enabled-{}", kind.id())),
                toggle_on,
            )
            .on_change(cx, move |this, enabled, _, cx| {
                this.set_provider_enabled(kind, enabled, cx)
            });

            let expanded = self.settings_ui.expanded_provider == Some(kind);
            let expand_button = IconButton::new(
                SharedString::from(format!("provider-expand-{}", kind.id())),
                icon(
                    if expanded {
                        "chevron.down"
                    } else {
                        "chevron.right"
                    },
                    13.0,
                    theme.text_tertiary,
                ),
            )
            .on_activation(cx, move |this, window, cx| {
                this.toggle_provider_expanded(kind, window, cx)
            });

            let header = FormRow::new(kind.display_name())
                .label_content(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(7.0))
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if installed {
                                    theme.text
                                } else {
                                    theme.text_secondary
                                })
                                .child(kind.display_name()),
                        )
                        .when_some(version, |element, version| {
                            element.child(
                                div()
                                    .font_family(crate::md::render::MONO_FAMILY)
                                    .text_size(sp(12.5))
                                    .text_color(theme.text_tertiary)
                                    .child(SharedString::from(format!("v{version}"))),
                            )
                        }),
                )
                .description_content(
                    div()
                        .text_size(sp(12.5))
                        .text_color(theme.text_tertiary)
                        .child(detail),
                )
                .leading(
                    div()
                        .relative()
                        .w(px(30.0))
                        .h(px(30.0))
                        .flex_none()
                        .rounded(px(7.0))
                        .bg(theme.overlay)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(provider_mark(
                            kind,
                            16.0,
                            provider_color(&theme, kind).opacity(if installed { 1.0 } else { 0.5 }),
                        ))
                        .child(
                            div()
                                .absolute()
                                .bottom(px(-2.0))
                                .right(px(-2.0))
                                .w(px(10.0))
                                .h(px(10.0))
                                .rounded_full()
                                .border_2()
                                .border_color(theme.raised)
                                .bg(dot_color),
                        ),
                )
                .min_h(px(0.0))
                .p(px(0.0))
                .gap(px(12.0))
                .control(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(expand_button)
                        .when(installed, |controls| controls.child(toggle)),
                );

            rows = rows.child(
                div()
                    .py(px(11.0))
                    .flex()
                    .flex_col()
                    .when(index + 1 != provider_count, |element| {
                        element.border_b_1().border_color(theme.border)
                    })
                    .child(header)
                    .when(expanded, |element| {
                        element.child(self.render_provider_expanded_settings(kind, theme, cx))
                    }),
            );
        }

        Surface::new()
            .mt(px(15.0))
            .px(px(20.0))
            .py(px(14.0))
            .child(
                FormRow::new(tr!("providers.coding_agents"))
                    .description(tr!("providers.description"))
                    .min_h(px(0.0))
                    .p(px(0.0))
                    .items_start()
                    .gap(px(20.0))
                    .control(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(6.0))
                            .child(refresh)
                            .when_some(checked_label, |element, label| {
                                element.child(Label::new(label).text_color(theme.text_ghost))
                            }),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    /// The expanded row's settings body: the binary override for this
    /// provider, with the detection result as its caption.
    fn render_provider_expanded_settings(
        &self,
        kind: ProviderKind,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Section {
        let override_value = self.state.provider_binary_overrides.get(&kind).cloned();
        let full_path = self
            .provider_probe(kind)
            .filter(|probe| probe.installed)
            .and_then(|probe| probe.path.as_ref())
            .map(|path| path.display().to_string());

        let caption = match (&override_value, full_path) {
            (Some(_), Some(path)) => tr!("providers.using_override", path = path),
            (Some(_), None) => tr!("providers.invalid_override"),
            (None, Some(path)) => tr!("providers.detected_at", path = path),
            (None, None) => tr!("providers.searches_path", command = kind.command()),
        };

        let reset = Button::new(
            SharedString::from(format!("provider-path-reset-{}", kind.id())),
            tr!("common.reset"),
        )
        .button_style(ButtonStyle::Ghost)
        .h(px(29.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(theme.border_strong)
        .flex_none()
        .text_size(px(12.5))
        .text_color(theme.text_secondary)
        .hover(|style| style.bg(theme.overlay))
        .on_activation(cx, |this, _, cx| {
            this.settings_ui
                .provider_path_input
                .update(cx, |input, cx| input.clear(cx));
            this.apply_provider_path_override(cx);
        });

        Section::new(tr!("providers.binary_path"))
            .description(tr!(
                "providers.binary_path_description",
                provider = kind.short_name()
            ))
            .mt(px(10.0))
            .pl(px(42.0))
            .child(
                div()
                    .mt(px(3.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        TextField::new(
                            SharedString::from(format!("provider-path-field-{}", kind.id())),
                            self.settings_ui.provider_path_input.clone(),
                        )
                        .flex_1()
                        .max_w(px(430.0)),
                    )
                    .when(override_value.is_some(), |element| element.child(reset)),
            )
            .child(
                div()
                    .text_size(sp(12.5))
                    .text_color(theme.text_ghost)
                    .child(SharedString::from(caption)),
            )
    }

    fn toggle_provider_expanded(
        &mut self,
        provider: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Commit any pending edit for the previously expanded provider before
        // the input is handed to another row.
        self.apply_provider_path_override(cx);
        if self.settings_ui.expanded_provider == Some(provider) {
            self.settings_ui.expanded_provider = None;
        } else {
            self.settings_ui.expanded_provider = Some(provider);
            let override_value = self
                .state
                .provider_binary_overrides
                .get(&provider)
                .cloned()
                .unwrap_or_default();
            self.settings_ui
                .provider_path_input
                .update(cx, |input, cx| input.set_content(override_value, cx));
            let focus = self.settings_ui.provider_path_input.read(cx).focus();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn render_computer_use_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let enabled = self.state.computer_use_enabled;
        let permissions = self.settings.permissions.snapshot.clone();
        let pending = self.settings.permissions.request_pending;
        let helper_name = crate::computer_use::helper_display_name();
        let mut allowed_apps = v_stack().gap(px(1.0));
        if self.state.computer_use_allowed_apps.is_empty() {
            allowed_apps = allowed_apps.child(
                Label::new(tr!("computer_use.no_always_allowed_apps"))
                    .py(px(12.0))
                    .text_color(theme.text_tertiary),
            );
        } else {
            for (index, grant) in self.state.computer_use_allowed_apps.iter().enumerate() {
                let key = grant.key();
                let is_last = index + 1 == self.state.computer_use_allowed_apps.len();
                let app_icon = self.computer_use_app_icon(&grant.bundle_id, cx);
                allowed_apps =
                    allowed_apps.child(
                        FormRow::new(grant.app_name.clone())
                            .description(grant.bundle_id.clone())
                            .min_h(px(0.0))
                            .px(px(0.0))
                            .py(px(9.0))
                            .gap(px(10.0))
                            .when(!is_last, |row| row.border_b_1().border_color(theme.border))
                            .leading(div().size(px(32.0)).flex_none().rounded(px(7.0)).when_some(
                                app_icon,
                                |tile, app_icon| {
                                    tile.child(img(app_icon).size_full().rounded(px(7.0)))
                                },
                            ))
                            .control(
                                Button::new(
                                    SharedString::from(format!("revoke-computer-app-{key}")),
                                    tr!("common.revoke"),
                                )
                                .button_style(ButtonStyle::Ghost)
                                .h(px(25.0))
                                .px(px(9.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .text_size(px(12.5))
                                .text_color(theme.text_secondary)
                                .flex_none()
                                .hover(|style| style.bg(theme.overlay).text_color(theme.danger))
                                .on_activation(cx, move |this, _, cx| {
                                    this.revoke_computer_app(&key, cx)
                                }),
                            ),
                    );
            }
        }
        v_stack()
            .mt(px(15.0))
            .w_full()
            .gap(px(12.0))
            .child(
                Surface::new().child(
                    FormRow::new(tr!("computer_use.allow_apps"))
                        .description(tr!("computer_use.availability"))
                        .py(px(14.0))
                        .gap(px(20.0))
                        .control(
                            Toggle::new("computer-use-enabled", enabled)
                                .on_change(cx, |this, enabled, _, cx| {
                                    this.set_computer_use_enabled(enabled, cx)
                                }),
                        ),
                ),
            )
            .when(cfg!(target_os = "macos"), |column| {
                column.child(
                    Surface::new()
                        .px(px(20.0))
                        .py(px(14.0))
                        .child(
                            Section::new(tr!("computer_use.macos_access"))
                                .description(tr!(
                                    "computer_use.helper_access",
                                    helper = helper_name
                                ))
                                .gap(px(4.0)),
                        )
                        .child(permission_status_row(
                            tr!("computer_use.screen_recording"),
                            tr!("computer_use.screen_recording_description"),
                            permissions.screen_recording,
                            "screen-recording-settings",
                            theme,
                            cx,
                        ))
                        .child(permission_status_row(
                            tr!("computer_use.accessibility"),
                            tr!("computer_use.accessibility_description"),
                            permissions.accessibility,
                            "accessibility-settings",
                            theme,
                            cx,
                        ))
                        .child(
                            div().mt(px(11.0)).flex().items_center().gap(px(8.0)).child(
                                Button::new(
                                    "recheck-computer-permissions",
                                    if pending {
                                        tr!("common.checking")
                                    } else {
                                        tr!("common.recheck")
                                    },
                                )
                                .button_style(ButtonStyle::Ghost)
                                .h(px(28.0))
                                .px(px(11.0))
                                .rounded(px(7.0))
                                .border_1()
                                .border_color(theme.border_strong)
                                .text_size(px(12.5))
                                .text_color(theme.text_secondary)
                                .disabled(pending)
                                .on_activation(cx, |this, _, cx| {
                                    this.request_computer_permissions(false, cx)
                                }),
                            ),
                        ),
                )
            })
            .when(!cfg!(target_os = "macos"), |column| {
                column.child(Surface::new().px(px(20.0)).py(px(14.0)).child(
                    Section::new(tr!("computer_use.desktop_access")).description(
                        if cfg!(target_os = "windows") {
                            tr!("computer_use.windows_access_description")
                        } else {
                            tr!("computer_use.linux_access_description")
                        },
                    ),
                ))
            })
            .child(
                Surface::new()
                    .px(px(20.0))
                    .py(px(14.0))
                    .child(
                        Section::new(tr!("computer_use.always_allowed_apps"))
                            .description(tr!("computer_use.always_allowed_apps_description"))
                            .gap(px(4.0)),
                    )
                    .child(allowed_apps),
            )
            .into_any_element()
    }

    fn render_settings_drag_region(
        &self,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let region = div().id(id);
        // Windows drags from the hit test rather than a mouse-move handler.
        #[cfg(target_os = "windows")]
        let region = region.window_control_area(gpui::WindowControlArea::Drag);

        region
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
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
}

/// Sizes offered by the font-size dropdowns. A hand-edited `app.json` may
/// hold values outside this list; they render as-is and simply select
/// nothing here.
const FONT_SIZES: [f32; 8] = [11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0];

fn font_size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0} px")
    } else {
        format!("{size} px")
    }
}

/// "Checked …" caption for the Providers page. Recomputed whenever the page
/// redraws; precision beyond the minute is noise here.
fn detection_checked_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 90 {
        tr!("providers.checked_just_now")
    } else if seconds < 3600 {
        tr!("providers.checked_minutes_ago", count = seconds / 60)
    } else {
        tr!("providers.checked_hours_ago", count = seconds / 3600)
    }
}

/// Keep the full binary path, abbreviating only the user's home directory.
fn abbreviate_home_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(relative) if relative.as_os_str().is_empty() => "~".to_owned(),
        Some(relative) => format!("~/{}", relative.display()),
        None => path.display().to_string(),
    }
}

fn permission_status_row(
    name: String,
    description: String,
    granted: bool,
    id: &'static str,
    theme: Theme,
    cx: &mut Context<Michelle>,
) -> FormRow {
    let status = if granted {
        div()
            .id(id)
            .h(px(25.0))
            .px(px(4.0))
            .rounded(px(6.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.0))
            .text_size(px(12.5))
            .text_color(theme.success)
            .child(icon("checkmark", 12.0, theme.success))
            .child(tr!("computer_use.access_granted"))
            .into_any_element()
    } else {
        Button::new(id, tr!("computer_use.grant_access"))
            .button_style(ButtonStyle::Ghost)
            .h(px(25.0))
            .px(px(9.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong)
            .text_size(px(12.5))
            .text_color(theme.text_secondary)
            .flex_none()
            .hover(|style| style.bg(theme.overlay).text_color(theme.text))
            .on_activation(cx, |this, _, cx| {
                this.request_computer_permissions(true, cx)
            })
            .into_any_element()
    };
    FormRow::new(name)
        .description(description)
        .control(status)
        .mt(px(10.0))
        .pt(px(10.0))
        .px(px(0.0))
        .pb(px(0.0))
        .min_h(px(0.0))
        .border_t_1()
        .border_color(theme.border)
        .gap(px(10.0))
}

#[cfg(test)]
mod tests {
    use super::{SettingsPage, abbreviate_home_path, settings_navigation_target};
    use std::path::Path;

    #[test]
    fn navigation_stays_in_filtered_pages_and_supports_home_end() {
        use SettingsPage::{Appearance, General, Skills};

        let pages = [Appearance, Skills];
        for (current, key, expected) in [
            (Appearance, "down", Some(Skills)),
            (Skills, "down", Some(Appearance)),
            (Appearance, "up", Some(Skills)),
            (General, "down", Some(Appearance)),
            (General, "up", Some(Skills)),
            (Skills, "home", Some(Appearance)),
            (Appearance, "end", Some(Skills)),
            (Appearance, "enter", None),
            (Appearance, "space", None),
        ] {
            assert_eq!(settings_navigation_target(&pages, current, key), expected);
        }
        for key in ["up", "down", "home", "end"] {
            assert_eq!(settings_navigation_target(&[], General, key), None);
        }
    }

    #[test]
    fn provider_paths_abbreviate_only_the_home_prefix() {
        let home = Path::new("/Users/example");

        assert_eq!(
            abbreviate_home_path(Path::new("/Users/example/.local/bin/amp"), Some(home)),
            "~/.local/bin/amp"
        );
        assert_eq!(
            abbreviate_home_path(Path::new("/opt/homebrew/bin/codex"), Some(home)),
            "/opt/homebrew/bin/codex"
        );
    }
}
