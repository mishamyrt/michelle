use crate::ui::icons::sf_symbols::font_icon;
use crate::ui::{IconSize, SfSymbolWeight, StyledTypography, TextStyle, UiColors, icon};
use gpui::{
    AnyElement, App, Div, ElementId, Hsla, InteractiveElement, Interactivity, ParentElement,
    Pixels, RenderOnce, SharedString, Stateful, StyleRefinement, Styled, Svg, Window, div,
    prelude::*, px, svg,
};

pub(crate) fn chip_icon(name: &'static str, color: Hsla) -> Svg {
    if let Some((path, width, height)) =
        font_icon(name, IconSize::Tiny.font_size(), SfSymbolWeight::Regular)
    {
        svg()
            .path(path)
            .w(px(width))
            .h(px(height))
            .flex_none()
            .text_color(color)
    } else {
        icon(name, 12.0, color).w(px(12.0)).h(px(12.0))
    }
}

/// A compact chip used as a dropdown-menu trigger. `selected` is driven by the
/// menu's open state and renders as a soft fill.
#[derive(IntoElement)]
pub struct MenuChip {
    base: Stateful<Div>,
    icon: Option<(&'static str, Hsla)>,
    label: SharedString,
    caret: bool,
    outlined: bool,
    selected: bool,
    disabled: bool,
    height: Option<Pixels>,
    background: Option<Hsla>,
}

impl MenuChip {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id).text_style(TextStyle::Body),
            icon: None,
            label: SharedString::default(),
            caret: true,
            outlined: false,
            selected: false,
            disabled: false,
            height: None,
            background: None,
        }
    }

    /// Override the chip's fixed height, for rows whose controls share a
    /// different one.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Fill behind an outlined chip. The default matches raised cards; a
    /// chip sitting directly on another surface passes that surface here so
    /// it doesn't read as a filled pill.
    pub fn background(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }

    pub fn icon(mut self, path: &'static str, color: Hsla) -> Self {
        self.icon = Some((path, color));
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn outlined(mut self) -> Self {
        self.outlined = true;
        self
    }

    pub fn caret(mut self, caret: bool) -> Self {
        self.caret = caret;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Soft fill marking the chip as the open menu's trigger.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Styled for MenuChip {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuChip {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for MenuChip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for MenuChip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = UiColors::current(cx);
        self.base
            .h(self
                .height
                .unwrap_or(if self.outlined { px(30.0) } else { px(22.0) }))
            .px(if self.outlined { px(10.0) } else { px(7.0) })
            .rounded(if self.outlined { px(7.0) } else { px(6.0) })
            .flex()
            .items_center()
            .gap(px(4.0))
            .cursor_default()
            .focus_visible(|style| style.border_1().border_color(theme.accent))
            .when(self.outlined, |element| {
                element
                    .border_1()
                    .border_color(theme.border_strong)
                    .bg(self.background.unwrap_or(theme.raised))
            })
            .when(self.selected, |element| element.bg(theme.overlay))
            .when(!self.disabled, |element| {
                element.hover(|element| element.bg(theme.overlay))
            })
            .when(self.disabled, |element| element.opacity(0.7))
            .when_some(self.icon, |element, (path, color)| {
                element.child(chip_icon(path, color))
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.text_secondary)
                    .child(self.label),
            )
            .when(self.caret, |element| {
                element.child(icon("chevron.down", 10.5, theme.text_ghost))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_icons_keep_native_symbol_proportions_and_fixed_brand_slots() {
        std::thread::spawn(|| {
            crate::assets::Assets::default()
                .prepare_system_icons()
                .unwrap();
            for name in ["lock", "laptopcomputer"] {
                let mut symbol = chip_icon(name, gpui::black());
                let size = &symbol.style().size;
                assert_ne!(
                    size.width, size.height,
                    "{name} must retain its native proportions"
                );
            }
            for name in ["icons/provider-openai.svg", "icons/provider-claude.svg"] {
                let mut brand = chip_icon(name, gpui::black());
                assert_eq!(brand.style().size.width, Some(px(12.0).into()));
                assert_eq!(brand.style().size.height, Some(px(12.0).into()));
            }
        })
        .join()
        .unwrap();
    }
}
