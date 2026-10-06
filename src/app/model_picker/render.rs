//! Searchable model menu; only visible rows are built.
use super::model::{next_model_picker_highlight, visible_picker_rows};
use super::*;
use crate::ui::{
    IconSize, SfSymbol, SymbolWeight,
    squircle::{SquircleStyled, squircle},
};

const ROW_HEIGHT: f32 = 28.0;
const VISIBLE_MODELS: usize = 5;

fn visible_row_count(rows: &[ModelPickerRow]) -> usize {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.is_model())
        .nth(VISIBLE_MODELS - 1)
        .map_or(rows.len(), |(index, _)| index + 1)
}

impl Michelle {
    pub(in crate::app) fn render_provider_model_control(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::current(cx);
        let session = self.selected_session();
        let provider = session.map(|session| session.provider).unwrap_or_default();
        let selected_model = session.and_then(|session| self.catalog_model_id_for_session(session));
        let selected_model_name = self.model_display_name(provider, selected_model);
        if !session.is_some_and(|session| session.can_choose_model(provider)) {
            return div()
                .h(px(24.0))
                .px(px(7.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(provider_mark(
                    provider,
                    10.5,
                    provider_color(&theme, provider).opacity(0.9),
                ))
                .child(
                    div()
                        .max_w(px(210.0))
                        .truncate()
                        .text_color(theme.text_secondary)
                        .child(SharedString::from(selected_model_name)),
                )
                .into_any_element();
        }

        let selected_model = selected_model.map(str::to_owned);
        let weak = cx.entity().downgrade();
        let search = self.model_picker_ui.search.clone();
        let search_focus = search.read(cx).focus_handle(cx);
        let favorite_focus = self.model_picker_ui.favorite_focus.clone();
        let empty_focus = self.model_picker_ui.empty_focus.clone();
        let no_providers = self.model_picker_has_no_providers();
        let handle = {
            let weak = weak.clone();
            let search = search.clone();
            let search_focus = search_focus.clone();
            let empty_focus = empty_focus.clone();
            self.menu_handle_with(MODEL_PICKER_MENU_ID, cx, move |open, window, cx| {
                let mut empty = false;
                let _ = weak.update(cx, |this, cx| {
                    if open {
                        empty = this.model_picker_has_no_providers();
                        this.model_picker_ui.highlight.set(None);
                        search.update(cx, |input, cx| input.clear(cx));
                        // The menu now shows every usable catalog, so refresh them together.
                        let locked = this
                            .selected_session()
                            .filter(|session| !session.messages.is_empty())
                            .map(|session| session.provider);
                        for kind in ProviderKind::ALL {
                            if model::picker_has_provider(
                                &this.settings.providers.probes,
                                &this.state.disabled_providers,
                                locked,
                                kind,
                            ) && (locked.is_none() || locked == Some(kind))
                            {
                                this.refresh_provider_model_discovery(kind);
                            }
                        }
                        this.reveal_selected_picker_model();
                    } else {
                        window.focus(&this.composer_ui.input.read(cx).focus(), cx);
                    }
                    cx.notify();
                });
                if open {
                    let focus = if empty {
                        empty_focus.clone()
                    } else {
                        search_focus.clone()
                    };
                    let weak = weak.clone();
                    // Deferred popovers join the focus tree after their first paint.
                    window.on_next_frame(move |window, _| {
                        window.on_next_frame(move |window, cx| {
                            window.focus(&focus, cx);
                            let _ = weak.update(cx, |this, _| this.reveal_selected_picker_model());
                        });
                    });
                }
            })
        };

        // The closed control appears on every composer frame; build catalogs only when open.
        let rows = if handle.is_open() {
            let rows = self.current_model_picker_rows(cx);
            self.sync_model_picker_rows(&rows);
            self.model_picker_ui.rows.borrow().clone()
        } else {
            Rc::new(Vec::new())
        };
        let highlight = self
            .model_picker_ui
            .highlight
            .get()
            .filter(|index| *index < rows.len());
        let favorites = if handle.is_open() {
            self.state.favorite_models.clone()
        } else {
            Vec::new()
        };
        let list_state = self.model_picker_ui.list_state.clone();
        let scrollbar_state = self.model_picker_ui.scrollbar.clone();
        let searching = !search.read(cx).content().trim().is_empty();
        let discovering = !self.settings.providers.model_discoveries_pending.is_empty();
        let trigger = if no_providers {
            MenuChip::new("composer-provider-model")
                .icon("exclamationmark.triangle", theme.warning)
                .label(tr!("models.no_providers"))
        } else {
            MenuChip::new("composer-provider-model")
                .icon(
                    presentation::provider_icon(provider),
                    provider_color(&theme, provider).opacity(0.9),
                )
                .label(selected_model_name)
        };

        popover(
            trigger
                .text_style(TextStyle::Caption)
                .caret(false)
                .selected(handle.is_open()),
            &handle,
            MenuAlign::AboveLeft,
            move |popover, window, cx| {
                if no_providers {
                    return model_picker_empty_state(
                        &theme,
                        &empty_focus,
                        popover.clone(),
                        weak.clone(),
                    );
                }
                let body = if rows.is_empty() {
                    div()
                        .h(px(64.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_style(TextStyle::Caption)
                        .text_color(theme.text_ghost)
                        .child(if searching {
                            tr!("models.none_found")
                        } else if discovering {
                            tr!("models.loading")
                        } else {
                            tr!("models.none_reported")
                        })
                        .into_any_element()
                } else {
                    let list_rows = rows.clone();
                    let weak = weak.clone();
                    let popover = popover.clone();
                    let favorites = favorites.clone();
                    let selected_model = selected_model.clone();
                    let favorite_focus = favorite_focus.clone();
                    div()
                        .id("model-picker-list")
                        .relative()
                        .w_full()
                        .h(px(visible_row_count(&rows) as f32 * ROW_HEIGHT))
                        .child(
                            list(list_state.clone(), move |index, _window, _cx| {
                                let Some(row) = list_rows.get(index) else {
                                    return div().into_any_element();
                                };
                                match row {
                                    ModelPickerRow::Header(kind) => div()
                                        .h(px(ROW_HEIGHT))
                                        .flex()
                                        .items_center()
                                        .text_style(TextStyle::Caption)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_tertiary)
                                        .child(
                                            kind.map(|kind| SharedString::from(kind.short_name()))
                                                .unwrap_or_else(|| tr!("models.favorites").into()),
                                        )
                                        .into_any_element(),
                                    ModelPickerRow::Model {
                                        provider: kind,
                                        model,
                                        in_favorites,
                                    } => {
                                        let favorite = favorites.iter().any(|favorite| {
                                            favorite.provider == *kind && favorite.model == model.id
                                        });
                                        model_picker_row(
                                            index,
                                            *kind,
                                            model,
                                            *in_favorites,
                                            favorite,
                                            *kind == provider
                                                && selected_model.as_deref()
                                                    == Some(model.id.as_str()),
                                            highlight == Some(index),
                                            &theme,
                                            &favorite_focus,
                                            weak.clone(),
                                            popover.clone(),
                                        )
                                    }
                                }
                            })
                            .size_full(),
                        )
                        .child(scrollbar::vertical(&list_state, &scrollbar_state))
                        .into_any_element()
                };
                let navigate = |key: &'static str| {
                    let weak = weak.clone();
                    move |cx: &mut App| {
                        let _ =
                            weak.update(cx, |this, cx| this.move_model_picker_highlight(key, cx));
                    }
                };
                let next = navigate("down");
                let previous = navigate("up");
                let tab_weak = weak.clone();
                let back_tab_weak = weak.clone();
                let confirm_weak = weak.clone();
                let confirm_popover = popover.clone();
                let key_weak = weak.clone();
                let key_focus = favorite_focus.clone();
                let key_search_focus = search_focus.clone();
                let search_focused = search.read(cx).is_visually_focused(window);
                div()
                    .id("model-picker")
                    .tab_group()
                    .tab_index(0)
                    .w(px(320.0))
                    .relative()
                    .p(px(5.0))
                    .rounded(px(12.0))
                    .text_style(TextStyle::Body)
                    .font_weight(FontWeight::MEDIUM)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        squircle()
                            .rounded(px(12.0))
                            .bg(theme.raised)
                            .border(px(0.5))
                            .border_color(theme.border_strong)
                            .border_inside()
                            .absolute_expand(),
                    )
                    .on_action(move |_: &SelectNextEntry, _, cx| next(cx))
                    .on_action(move |_: &SelectPreviousEntry, _, cx| previous(cx))
                    .on_action(move |_: &SelectNextTab, window, cx| {
                        let _ = tab_weak
                            .update(cx, |this, cx| this.focus_model_picker_favorite(window, cx));
                    })
                    .on_action(move |_: &SelectPreviousTab, window, cx| {
                        let _ = back_tab_weak
                            .update(cx, |this, cx| this.focus_model_picker_favorite(window, cx));
                    })
                    .on_action(move |_: &ConfirmEntry, window, cx| {
                        let _ =
                            confirm_weak.update(cx, |this, cx| this.choose_highlighted_model(cx));
                        confirm_popover.close(window, cx);
                        window.refresh();
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if !key_focus.is_focused(window) {
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        match key {
                            "tab" => window.focus(&key_search_focus, cx),
                            "up" | "down" | "home" | "end" => {
                                let _ = key_weak.update(cx, |this, cx| {
                                    this.move_model_picker_highlight(key, cx)
                                });
                            }
                            "enter" | "space" => {
                                let _ = key_weak.update(cx, |this, cx| {
                                    let row = this
                                        .model_picker_ui
                                        .rows
                                        .borrow()
                                        .get(
                                            this.model_picker_ui
                                                .highlight
                                                .get()
                                                .unwrap_or(usize::MAX),
                                        )
                                        .cloned();
                                    if let Some(ModelPickerRow::Model {
                                        provider, model, ..
                                    }) = row
                                    {
                                        this.toggle_favorite_model(provider, model.id, cx);
                                    }
                                });
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .id("model-picker-search")
                            .w_full()
                            .h(px(32.0))
                            .flex_none()
                            .px(px(10.0))
                            .rounded(px(9.0))
                            .border_2()
                            .border_color(if search_focused {
                                theme.accent
                            } else {
                                theme.border_strong
                            })
                            .bg(theme.raised)
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(icon("magnifyingglass", 14.0, theme.text_tertiary))
                            .child(div().min_w_0().flex_1().child(search.clone())),
                    )
                    .child(body)
                    .into_any_element()
            },
        )
    }

    pub(super) fn current_model_picker_rows(&self, cx: &App) -> Vec<ModelPickerRow> {
        let locked = self
            .selected_session()
            .filter(|session| !session.messages.is_empty())
            .map(|session| session.provider);
        visible_picker_rows(
            &self.settings.providers.probes,
            &self.state.favorite_models,
            &self.state.disabled_providers,
            locked,
            &self
                .model_picker_ui
                .search
                .read(cx)
                .content()
                .trim()
                .to_ascii_lowercase(),
        )
    }

    pub(super) fn sync_model_picker_rows(&self, rows: &[ModelPickerRow]) {
        let mut cached = self.model_picker_ui.rows.borrow_mut();
        if cached.as_slice() == rows {
            return;
        }
        let highlight = self
            .model_picker_ui
            .highlight
            .get()
            .and_then(|index| cached.get(index))
            .and_then(|row| {
                rows.iter()
                    .position(|candidate| candidate.same_model_row(row))
                    .or_else(|| rows.iter().position(|candidate| candidate.same_model(row)))
            });
        self.model_picker_ui.highlight.set(highlight);
        *cached = Rc::new(rows.to_vec());
        self.model_picker_ui
            .list_state
            .reset_with_uniform_height(rows.len(), px(ROW_HEIGHT));
        if let Some(index) = self
            .model_picker_ui
            .highlight
            .get()
            .filter(|index| *index < rows.len())
        {
            self.model_picker_ui.list_state.scroll_to_reveal_item(index);
        }
    }

    fn move_model_picker_highlight(&mut self, key: &str, cx: &mut Context<Self>) {
        let rows = self.model_picker_ui.rows.borrow().clone();
        if let Some(index) =
            next_model_picker_highlight(self.model_picker_ui.highlight.get(), &rows, key)
        {
            self.model_picker_ui.highlight.set(Some(index));
            self.model_picker_ui.list_state.scroll_to_reveal_item(index);
            cx.notify();
        }
    }

    fn focus_model_picker_favorite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_picker_ui.highlight.get().is_none() {
            self.move_model_picker_highlight("down", cx);
        }
        if self.model_picker_ui.highlight.get().is_some() {
            let focus = self.model_picker_ui.favorite_focus.clone();
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |window, cx| window.focus(&focus, cx));
            });
        }
    }

    pub(super) fn reveal_selected_picker_model(&self) {
        let session = self.selected_session();
        let provider = session.map(|session| session.provider).unwrap_or_default();
        let selected = session.and_then(|session| self.catalog_model_id_for_session(session));
        let locked = session
            .filter(|session| !session.messages.is_empty())
            .map(|session| session.provider);
        let rows = visible_picker_rows(
            &self.settings.providers.probes,
            &self.state.favorite_models,
            &self.state.disabled_providers,
            locked,
            "",
        );
        self.sync_model_picker_rows(&rows);
        let index = rows
            .iter()
            .position(|row| {
                matches!(row,
            ModelPickerRow::Model { provider: kind, model, .. }
                if *kind == provider && selected == Some(model.id.as_str()))
            })
            .unwrap_or(0);
        self.model_picker_ui.list_state.scroll_to_reveal_item(index);
    }

    fn choose_highlighted_model(&mut self, cx: &mut Context<Self>) {
        let rows = self.model_picker_ui.rows.borrow().clone();
        let index = self
            .model_picker_ui
            .highlight
            .get()
            .or_else(|| rows.iter().position(ModelPickerRow::is_model));
        if let Some(ModelPickerRow::Model {
            provider, model, ..
        }) = index.and_then(|index| rows.get(index))
        {
            self.choose_model(*provider, model.id.clone(), cx);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn model_picker_row(
    index: usize,
    provider: ProviderKind,
    model: &ProviderModel,
    in_favorites: bool,
    favorite: bool,
    selected: bool,
    highlighted: bool,
    theme: &Theme,
    favorite_focus: &FocusHandle,
    weak: WeakEntity<Michelle>,
    popover: ContextMenuHandle,
) -> AnyElement {
    let group = SharedString::from(format!("model-picker-row-{index}"));
    let favorite_model = model.id.clone();
    let favorite_weak = weak.clone();
    let hover_weak = weak.clone();
    let model_id = model.id.clone();
    div()
        .id(("model-row", index))
        .group(group.clone())
        .relative()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px(px(11.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .cursor_default()
        .text_style(TextStyle::Body)
        .font_weight(FontWeight::MEDIUM)
        .text_color(if highlighted {
            gpui::white()
        } else {
            theme.text
        })
        .hover(|style| style.text_color(gpui::white()))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                let _ = hover_weak.update(cx, |this, cx| {
                    if this.model_picker_ui.highlight.get() != Some(index) {
                        this.model_picker_ui.highlight.set(Some(index));
                        cx.notify();
                    }
                });
            }
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .when(!highlighted, |element| {
                    element
                        .opacity(0.0)
                        .group_hover(group.clone(), |style| style.opacity(1.0))
                })
                .child(
                    squircle()
                        .rounded(px(8.0))
                        .bg(theme.accent)
                        .absolute_expand(),
                ),
        )
        .child(
            div()
                .w(px(12.0))
                .h(px(16.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |element| {
                    element.child(
                        SfSymbol::new("checkmark")
                            .size(IconSize::Tiny)
                            .weight(SymbolWeight::Bold)
                            .w(px(10.0))
                            .h(px(10.0)),
                    )
                }),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .child(SharedString::from(model.name.clone())),
        )
        .child(
            div()
                .id(("favorite-model", index))
                .size(px(24.0))
                .flex_none()
                .rounded(px(5.0))
                .flex()
                .items_center()
                .justify_center()
                .when(highlighted, |element| {
                    element
                        .track_focus(favorite_focus)
                        .tab_index(0)
                        .tab_stop(true)
                })
                .when(in_favorites || !favorite, |element| {
                    element
                        .opacity(0.0)
                        .group_hover(group, |style| style.opacity(1.0))
                })
                .focus_visible(|style| style.opacity(1.0).border_1().border_color(gpui::white()))
                .child(
                    SfSymbol::new(if favorite { "star.fill" } else { "star" })
                        .size(IconSize::Small),
                )
                .tooltip(Tooltip::text(if favorite {
                    tr!("models.remove_favorite")
                } else {
                    tr!("models.add_favorite")
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = favorite_weak.update(cx, |this, cx| {
                        this.toggle_favorite_model(provider, favorite_model.clone(), cx)
                    });
                }),
        )
        .on_click(move |_, window, cx| {
            popover.close(window, cx);
            let _ = weak.update(cx, |this, cx| {
                this.choose_model(provider, model_id.clone(), cx)
            });
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_fits_five_models_in_addition_to_group_headings() {
        let model = || ModelPickerRow::Model {
            provider: ProviderKind::Codex,
            model: ProviderModel::new("model", "Model"),
            in_favorites: false,
        };
        let rows = [
            ModelPickerRow::Header(None),
            model(),
            model(),
            ModelPickerRow::Header(Some(ProviderKind::Codex)),
            model(),
            model(),
            model(),
            model(),
        ];
        assert_eq!(visible_row_count(&rows), 7);
        assert_eq!(visible_row_count(&rows[..3]), 3);
        assert_eq!(visible_row_count(&vec![model(); 8]), 5);
        assert_eq!(visible_row_count(&[]), 0);
    }
}
