//! Model picker state and option controls.
use super::*;
use crate::ui::{
    StyledTypography, TextStyle,
    squircle::{SquircleStyled, squircle},
};
pub(super) mod model;
mod render;
use model::ModelPickerRow;

pub(super) const MODEL_PICKER_MENU_ID: &str = "provider-model-picker";
pub(in crate::app) struct ModelPickerUi {
    pub(in crate::app) search: Entity<TextInput>,
    pub(in crate::app) highlight: Cell<Option<usize>>,
    pub(in crate::app) favorite_focus: FocusHandle,
    pub(in crate::app) rows: RefCell<Rc<Vec<ModelPickerRow>>>,
    pub(in crate::app) list_state: ListState,
    pub(in crate::app) scrollbar: Rc<ScrollbarState>,
    pub(in crate::app) empty_focus: FocusHandle,
}

impl ModelPickerUi {
    pub(in crate::app) fn new(
        model_search: Entity<TextInput>,
        favorite_focus: FocusHandle,
        model_picker_empty_focus: FocusHandle,
    ) -> Self {
        Self {
            search: model_search,
            highlight: Cell::new(None),
            favorite_focus,
            rows: RefCell::new(Rc::new(Vec::new())),
            list_state: ListState::new(0, ListAlignment::Top, px(100.0)),
            scrollbar: ScrollbarState::new(),
            empty_focus: model_picker_empty_focus,
        }
    }
}

impl Michelle {
    pub(super) fn render_model_traits_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = Theme::current(cx);
        let session = self.selected_session()?;
        let model = self.model_metadata_for_session(session)?;
        if model.reasoning_efforts.is_empty()
            && model.service_tiers.is_empty()
            && model.context_windows.is_empty()
        {
            return None;
        }

        let cursor_suffix = (session.provider == ProviderKind::Cursor)
            .then(|| self.model_for_session(session))
            .flatten()
            .and_then(|requested| {
                self.provider_probe(session.provider).and_then(|probe| {
                    michelle_protocol::model_catalog::cursor_catalog_model(&probe.models, requested)
                })
            })
            .map(|matched| matched.suffix)
            .unwrap_or_default();
        let suffix_effort = michelle_protocol::model_catalog::cursor_suffix_reasoning_effort(
            &cursor_suffix,
            &model.reasoning_efforts,
        );
        let suffix_tier = michelle_protocol::model_catalog::cursor_suffix_service_tier(
            &cursor_suffix,
            &model.service_tiers,
        );

        let selected_effort = session
            .reasoning_effort
            .as_deref()
            .filter(|selected| {
                model
                    .reasoning_efforts
                    .iter()
                    .any(|option| option.id == *selected)
            })
            .or(suffix_effort.as_deref())
            .or(model.default_reasoning_effort.as_deref())
            .or_else(|| {
                model
                    .reasoning_efforts
                    .first()
                    .map(|option| option.id.as_str())
            })
            .map(str::to_owned);
        let effort_label = selected_effort.as_deref().and_then(|selected| {
            model
                .reasoning_efforts
                .iter()
                .find(|option| option.id == selected)
                .map(|option| option.label.clone())
        });

        let selected_tier = session
            .service_tier
            .as_deref()
            .filter(|selected| {
                *selected == "default"
                    || model
                        .service_tiers
                        .iter()
                        .any(|option| option.id == *selected)
            })
            .or(suffix_tier.as_deref())
            .or(model.default_service_tier.as_deref())
            .unwrap_or("default")
            .to_owned();
        let tier_label = if selected_tier == "default" {
            tr!("models.standard")
        } else {
            model
                .service_tiers
                .iter()
                .find(|option| option.id == selected_tier)
                .map(|option| option.label.clone())
                .unwrap_or_else(|| selected_tier.clone())
        };
        let selected_window = session
            .context_window
            .as_deref()
            .filter(|selected| {
                model
                    .context_windows
                    .iter()
                    .any(|option| option.id == *selected)
            })
            .or(model.default_context_window.as_deref())
            .or_else(|| {
                model
                    .context_windows
                    .first()
                    .map(|option| option.id.as_str())
            })
            .map(str::to_owned);
        // A non-default window changes what the session costs and how much it
        // can hold, so it reads on the chip rather than only inside the menu.
        let window_label = selected_window
            .as_deref()
            .filter(|selected| model.default_context_window.as_deref() != Some(selected))
            .and_then(|selected| {
                model
                    .context_windows
                    .iter()
                    .find(|option| option.id == selected)
                    .map(|option| option.label.clone())
            });

        let fast = selected_tier == "fast" || tier_label.eq_ignore_ascii_case("fast");
        let trigger_label = match (
            effort_label.unwrap_or_else(|| tier_label.clone()),
            window_label,
        ) {
            (label, Some(window)) => format!("{label} · {window}"),
            (label, None) => label,
        };
        let reasoning_efforts = model.reasoning_efforts.clone();
        let default_effort = model.default_reasoning_effort.clone();
        let service_tiers = model.service_tiers.clone();
        let context_windows = model.context_windows.clone();
        let default_window = model.default_context_window.clone();
        let default_tier = model
            .default_service_tier
            .clone()
            .unwrap_or_else(|| "default".to_owned());
        let weak = cx.entity().downgrade();
        let handle = self.menu_handle("model-traits", cx);
        Some(dropdown_menu(
            MenuChip::new("model-traits")
                .text_style(TextStyle::Caption)
                .when(fast, |trigger| trigger.icon("bolt", theme.text_secondary))
                .label(trigger_label)
                .caret(false)
                .selected(handle.is_open()),
            "model-traits-menu",
            &handle,
            MenuAlign::AboveLeft,
            move |_| {
                let mut items = Vec::new();
                if !reasoning_efforts.is_empty() {
                    for option in reasoning_efforts.clone() {
                        let weak = weak.clone();
                        let effort = option.id;
                        let is_default = default_effort.as_deref() == Some(effort.as_str());
                        let selected = selected_effort.as_deref() == Some(effort.as_str());
                        items.push(
                            traits_choice(theme, option.label, is_default, selected).on_click(
                                move |_, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_reasoning_effort(effort.clone(), cx);
                                    });
                                },
                            ),
                        );
                    }
                }
                if !service_tiers.is_empty() {
                    if !reasoning_efforts.is_empty() {
                        items.push(MenuItem::Separator);
                    }
                    items.push(MenuItem::Header(tr!("models.service_tier").into()));
                    let weak_standard = weak.clone();
                    items.push(
                        traits_choice(
                            theme,
                            tr!("models.standard"),
                            default_tier == "default",
                            selected_tier == "default",
                        )
                        .on_click(move |_, cx| {
                            let _ = weak_standard.update(cx, |this, cx| {
                                this.set_service_tier("default".to_owned(), cx);
                            });
                        }),
                    );
                    for option in service_tiers.clone() {
                        let weak = weak.clone();
                        let tier = option.id;
                        let is_default = default_tier == tier;
                        let selected = selected_tier == tier;
                        items.push(
                            traits_choice(theme, option.label, is_default, selected).on_click(
                                move |_, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_service_tier(tier.clone(), cx);
                                    });
                                },
                            ),
                        );
                    }
                }
                if !context_windows.is_empty() {
                    if !reasoning_efforts.is_empty() || !service_tiers.is_empty() {
                        items.push(MenuItem::Separator);
                    }
                    items.push(MenuItem::Header(tr!("models.context_window").into()));
                    for option in context_windows.clone() {
                        let weak = weak.clone();
                        let window = option.id;
                        let is_default = default_window.as_deref() == Some(window.as_str());
                        let selected = selected_window.as_deref() == Some(window.as_str());
                        items.push(
                            traits_choice(theme, option.label, is_default, selected).on_click(
                                move |_, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_context_window(window.clone(), cx);
                                    });
                                },
                            ),
                        );
                    }
                }
                items
            },
        ))
    }

    pub(super) fn render_agent_preset_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self
            .selected_session()
            .filter(|session| session.provider == ProviderKind::DeepSeek)?;
        if session.has_started() || session.is_busy() {
            return None;
        }
        let presets = self
            .provider_probe(ProviderKind::DeepSeek)
            .map(|probe| probe.agent_presets.clone())
            .unwrap_or_default();
        if presets.is_empty() {
            return None;
        }
        let selected_id = self.agent_preset_for_session(session)?;
        let selected_label = self.agent_preset_label_for_session(session)?;
        let theme = Theme::current(cx);
        let weak = cx.entity().downgrade();
        let refresh_weak = weak.clone();
        let handle = self.menu_handle_with("agent-preset", cx, move |open, _, cx| {
            if open {
                let _ = refresh_weak.update(cx, |this, _| {
                    this.refresh_provider_model_discovery(ProviderKind::DeepSeek);
                });
            }
        });
        let trigger = MenuChip::new("agent-preset")
            .icon("cpu", theme.text_tertiary)
            .label(selected_label)
            .caret(false)
            .selected(handle.is_open());

        Some(dropdown_menu(
            trigger,
            "agent-preset-menu",
            &handle,
            MenuAlign::AboveLeft,
            move |_| {
                presets
                    .clone()
                    .into_iter()
                    .map(|preset| {
                        let weak = weak.clone();
                        let preset_id = preset.id.clone();
                        let selected = preset_id == selected_id;
                        let name = if preset.is_custom {
                            format!("{} · {}", preset.display_name(), tr!("agent_preset.custom"))
                        } else {
                            preset.display_name()
                        };
                        let description = preset
                            .display_description()
                            .unwrap_or_else(|| tr!("agent_preset.no_description"))
                            // GPUI wraps at Unicode line-break opportunities,
                            // but an underscored tool name is otherwise one
                            // indivisible word. The zero-width spaces preserve
                            // its visible spelling while allowing the menu to
                            // keep it inside the card.
                            .replace('_', "_\u{200b}");
                        MenuItem::custom(move |_, _| {
                            div()
                                .w(px(340.0))
                                .min_w_0()
                                .py(px(5.0))
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .w_full()
                                                .truncate()
                                                .text_size(sp(12.5))
                                                .font_weight(if selected {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .child(name.clone()),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .mt(px(2.0))
                                                .text_size(sp(12.5))
                                                .line_height(sp(14.0))
                                                .whitespace_normal()
                                                .overflow_hidden()
                                                .child(description.clone()),
                                        ),
                                )
                                .into_any_element()
                        })
                        .selected(selected)
                        .on_click(move |_, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.set_agent_preset(preset_id.clone(), cx);
                            });
                        })
                    })
                    .collect()
            },
        ))
    }

    /// Primary modifier + /: toggle the composer's model picker as if its chip were clicked.
    pub(super) fn toggle_model_picker_action(
        &mut self,
        _: &ToggleModelPicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.is_some() {
            return;
        }
        if !self
            .selected_session()
            .is_some_and(|session| session.can_choose_model(session.provider))
        {
            return;
        }
        self.toggle_composer_menu(MODEL_PICKER_MENU_ID, window, cx);
    }

    pub(super) fn toggle_model_traits_action(
        &mut self,
        _: &ToggleModelTraits,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.page.is_some() || !self.can_configure_model_traits() {
            return;
        }
        self.toggle_composer_menu("model-traits", window, cx);
    }
}
/// The picker's whole body when nothing can back a session: no agent CLI
/// found on this machine, and none left switched on.
///
/// The panel offers the settings page that fixes it. Its button carries focus
/// so keyboard dismissal and activation still work without the search field.
fn model_picker_empty_state(
    theme: &Theme,
    focus: &FocusHandle,
    popover: ContextMenuHandle,
    michelle: WeakEntity<Michelle>,
) -> AnyElement {
    let click_popover = popover.clone();
    let click_michelle = michelle.clone();
    div()
        .w(px(320.0))
        .relative()
        .rounded(px(18.0))
        .child(
            squircle()
                .rounded(px(18.0))
                .bg(theme.raised)
                .border(px(0.5))
                .border_color(theme.border_strong)
                .border_inside()
                .absolute_expand(),
        )
        .shadow_lg()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(9.0))
        .p(px(15.0))
        .child(
            div()
                .w(px(40.0))
                .h(px(40.0))
                .rounded(px(10.0))
                .bg(theme.overlay)
                .flex()
                .items_center()
                .justify_center()
                .child(icon("cpu", 19.0, theme.text_tertiary)),
        )
        .child(
            div()
                .text_size(sp(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(tr!("models.no_providers_title")),
        )
        .child(
            div()
                .text_size(sp(12.5))
                .line_height(sp(17.0))
                .text_center()
                .text_color(theme.text_secondary)
                .child(tr!("models.no_providers_description")),
        )
        .child(
            div()
                .id("model-picker-open-providers")
                .track_focus(focus)
                .tab_index(0)
                .tab_stop(true)
                .focus_visible(|style| style.border_color(theme.accent))
                .mt(px(3.0))
                .h(px(28.0))
                .px(px(11.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(theme.border_strong)
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_default()
                .text_size(sp(12.5))
                .text_color(theme.text_secondary)
                .hover(|element| element.bg(theme.overlay))
                .child(icon("gearshape", 11.0, theme.text_tertiary))
                .child(tr!("models.open_provider_settings"))
                .on_click(move |_, window, cx| {
                    open_provider_settings_from_picker(&click_michelle, &click_popover, window, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        open_provider_settings_from_picker(&michelle, &popover, window, cx);
                        cx.stop_propagation();
                    }
                }),
        )
        .into_any_element()
}

/// Dismiss the picker and land on the Providers page, for both the empty
/// state's click and its keyboard activation. Closing first matters: the
/// picker returns focus to the composer as it closes, which would otherwise
/// pull focus straight back out of the settings view.
fn open_provider_settings_from_picker(
    michelle: &WeakEntity<Michelle>,
    popover: &ContextMenuHandle,
    window: &mut Window,
    cx: &mut App,
) {
    popover.close(window, cx);
    let _ = michelle.update(cx, |this, cx| {
        this.open_settings_action(&OpenSettings, window, cx);
        this.open_settings_page(SettingsPage::Providers, cx);
    });
}

pub(super) fn subscribe_search(search_input: &Entity<TextInput>, cx: &mut Context<Michelle>) {
    cx.subscribe(
        search_input,
        |this: &mut Michelle, search, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Edited) {
                if search.read(cx).content().trim().is_empty() {
                    this.model_picker_ui.highlight.set(None);
                    this.reveal_selected_picker_model();
                } else {
                    let rows = this.current_model_picker_rows(cx);
                    this.model_picker_ui.highlight.set(None);
                    this.sync_model_picker_rows(&rows);
                    let first = rows.iter().position(ModelPickerRow::is_model);
                    this.model_picker_ui.highlight.set(first);
                    if let Some(index) = first {
                        this.model_picker_ui.list_state.scroll_to_reveal_item(index);
                    }
                }
                cx.notify();
            }
        },
    )
    .detach();
}

/// One choice in the model-traits menu: a label plus a badge marking the
/// provider's own default, so the current selection and the default read apart.
pub(in crate::app) fn traits_choice(
    theme: Theme,
    label: String,
    is_default: bool,
    selected: bool,
) -> MenuItem {
    MenuItem::custom(move |_, _| {
        div()
            .w(px(190.0))
            .min_w_0()
            .py(px(2.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(div().min_w_0().flex_1().truncate().child(label.clone()))
            .when(is_default, |element| {
                element.child(
                    div()
                        .h(px(18.0))
                        .px(px(5.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .text_style(TextStyle::CaptionEmphasized)
                        .text_color(theme.text_tertiary)
                        .child(tr!("common.default")),
                )
            })
            .into_any_element()
    })
    .selected(selected)
}
