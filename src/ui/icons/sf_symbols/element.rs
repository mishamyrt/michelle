use super::{SfSymbolWeight, font_icon};
use crate::ui::{IconSize, sp};
use gpui::{
    App, Hsla, InteractiveElement, Interactivity, RenderOnce, StyleRefinement, Styled, Svg, Window,
    prelude::*, svg,
};

/// An SF Symbol sized like a font, with its native width and height computed
/// during startup. Like chrome text, the font size follows the UI size setting.
/// The preloaded symbol scales proportionally; no AppKit calls run here.
pub fn sf_icon(name: &'static str, font_size: f32, color: Hsla) -> SfIcon {
    SfIcon {
        base: svg().flex_none().text_color(color),
        name,
        font_size,
        weight: SfSymbolWeight::Regular,
    }
}

#[derive(IntoElement)]
pub struct SfIcon {
    base: Svg,
    name: &'static str,
    font_size: f32,
    weight: SfSymbolWeight,
}

/// The builder name used by new screens; `SfIcon` remains source-compatible.
pub type SfSymbol = SfIcon;

impl SfIcon {
    /// Inherits its container's foreground color unless explicitly overridden.
    pub fn new(name: &'static str) -> Self {
        Self {
            base: svg().flex_none(),
            name,
            font_size: IconSize::Regular.font_size(),
            weight: SfSymbolWeight::Regular,
        }
    }

    pub fn size(mut self, size: IconSize) -> Self {
        self.font_size = size.font_size();
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.base = self.base.text_color(color);
        self
    }

    pub fn weight(mut self, weight: SfSymbolWeight) -> Self {
        self.weight = weight;
        self
    }

    pub(super) fn into_svg(mut self, inherited_color: Hsla) -> Svg {
        let (path, width, height) = font_icon(self.name, self.font_size, self.weight)
            .unwrap_or_else(|| (self.name.into(), self.font_size, self.font_size));
        // Resolve each weight's native bounds, retaining explicit caller sizing.
        let style = self.base.style();
        // GPUI's SVG painter requires a local color rather than reading the text stack.
        style.text.color.get_or_insert(inherited_color);
        style.size.width.get_or_insert(sp(width).into());
        style.size.height.get_or_insert(sp(height).into());
        self.base.path(path)
    }
}

impl Styled for SfIcon {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for SfIcon {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for SfIcon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.into_svg(window.text_style().color)
    }
}
