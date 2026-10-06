use gpui::{
    AbsoluteLength, AlignContent, AlignItems, Background, BorderStyle, CornersRefinement,
    CursorStyle, DefiniteLength, Display, FlexDirection, FlexWrap, Font, FontStyle, FontWeight,
    GridPlacement, GridTemplate, GridTemplateMinSize, Hsla, JustifyContent, Length, Pixels,
    Refineable, SharedString, SizeRefinement, StrikethroughStyle, StyleRefinement, TextAlign,
    TextOverflow, TextStyleRefinement, UnderlineStyle, WhiteSpace, px, relative, rems,
};

use super::BorderMode;

const ELLIPSIS: SharedString = SharedString::new_static("…");

/// Style properties specific to squircle elements.
///
/// This struct extends GPUI's standard `StyleRefinement` with squircle-specific
/// properties like corner smoothing and custom border modes.
#[derive(Default, Clone)]
pub struct SquircleStyleRefinement {
    /// Standard GPUI style refinement (size, layout, etc.).
    pub inner: StyleRefinement,
    /// Corner radii for each corner of the squircle.
    pub corner_radii: CornersRefinement<Pixels>,
    /// Controls how smooth the corners are (0.0 to 1.0, default 0.6).
    pub corner_smoothing: Option<Pixels>,
    /// Whether to preserve smoothing when corners would overlap.
    pub preserve_smoothing: Option<bool>,
    /// Width of the border stroke.
    pub border_width: Option<Pixels>,
    /// How the border is positioned relative to the bounds.
    pub border_mode: Option<BorderMode>,
    /// Color/background for the border.
    pub border_color: Option<Background>,
    /// Fill color/background for the squircle.
    pub background: Option<Background>,
    /// Background color when the element is hovered.
    pub hover_bg: Option<Background>,
    /// Background color when the element is active (pressed).
    pub active_bg: Option<Background>,
}

impl SquircleStyleRefinement {
    /// Merges another style refinement into this one, with `other` taking precedence.
    pub fn refined(mut self, other: SquircleStyleRefinement) -> Self {
        self.inner = self.inner.refined(other.inner);

        if let Some(corner_smoothing) = other.corner_smoothing {
            self.corner_smoothing = Some(corner_smoothing);
        }

        if let Some(preserve_smoothing) = other.preserve_smoothing {
            self.preserve_smoothing = Some(preserve_smoothing);
        }

        if let Some(border_width) = other.border_width {
            self.border_width = Some(border_width);
        }

        if let Some(border_mode) = other.border_mode {
            self.border_mode = Some(border_mode);
        }

        if let Some(border_color) = other.border_color {
            self.border_color = Some(border_color);
        }

        if let Some(background) = other.background {
            self.background = Some(background);
        }

        if let Some(hover_bg) = other.hover_bg {
            self.hover_bg = Some(hover_bg);
        }

        if let Some(active_bg) = other.active_bg {
            self.active_bg = Some(active_bg);
        }

        let corner_radii = other.corner_radii;

        if let Some(top_left) = corner_radii.top_left {
            self.corner_radii.top_left = Some(top_left);
        }

        if let Some(top_right) = corner_radii.top_right {
            self.corner_radii.top_right = Some(top_right);
        }

        if let Some(bottom_left) = corner_radii.bottom_left {
            self.corner_radii.bottom_left = Some(bottom_left);
        }

        if let Some(bottom_right) = corner_radii.bottom_right {
            self.corner_radii.bottom_right = Some(bottom_right);
        }

        self
    }
}

/// Trait for applying squircle-specific styles to elements.
///
/// This trait provides a fluent API for styling squircles, similar to GPUI's
/// built-in `Styled` trait but with additional squircle-specific methods.
pub trait Styled
where
    Self: Sized,
{
    gpui::visibility_style_methods!();
    gpui::margin_style_methods!();
    gpui::padding_style_methods!();
    gpui::position_style_methods!();
    gpui::overflow_style_methods!();
    gpui::cursor_style_methods!();
    gpui::box_shadow_style_methods!();

    /// Returns a mutable reference to the inner GPUI style refinement.
    fn style(&mut self) -> &mut StyleRefinement;

    /// Returns a mutable reference to the squircle-specific style refinement.
    fn outer_style(&mut self) -> &mut SquircleStyleRefinement;

    /// Positions the border outside the squircle's bounds.
    fn border_outside(mut self) -> Self {
        self.outer_style().border_mode = Some(BorderMode::Outside);
        self
    }

    /// Positions the border inside the squircle's bounds.
    fn border_inside(mut self) -> Self {
        self.outer_style().border_mode = Some(BorderMode::Inside);
        self
    }

    /// Positions the border centered on the squircle's edge (default).
    fn border_center(mut self) -> Self {
        self.outer_style().border_mode = Some(BorderMode::Center);
        self
    }

    /// Sets the background fill color of the squircle.
    fn bg(mut self, color: impl Into<Background>) -> Self {
        self.outer_style().background = Some(color.into());
        self
    }

    /// Sets the background color when the squircle is hovered.
    fn bg_hover(mut self, color: impl Into<Background>) -> Self {
        self.outer_style().hover_bg = Some(color.into());
        self
    }

    /// Sets the background color when the squircle is active (pressed).
    fn bg_active(mut self, color: impl Into<Background>) -> Self {
        self.outer_style().active_bg = Some(color.into());
        self
    }

    /// Sets the border width in pixels.
    fn border(mut self, px: Pixels) -> Self {
        self.outer_style().border_width = Some(px);
        self
    }

    /// Sets the border color.
    fn border_color(mut self, color: impl Into<Background>) -> Self {
        self.outer_style().border_color = Some(color.into());
        self
    }

    /// Sets a 1px border on all sides.
    fn border_1(mut self) -> Self {
        self.outer_style().border_width = Some(px(1.));
        self
    }

    /// Sets the border width in pixels.
    fn border_1p5(mut self) -> Self {
        self.outer_style().border_width = Some(px(1.5));
        self
    }

    /// Sets the border width in pixels.
    fn border_2(mut self) -> Self {
        self.outer_style().border_width = Some(px(2.));
        self
    }

    /// Sets uniform corner radius for all corners.
    fn rounded(mut self, px: Pixels) -> Self {
        self.outer_style().corner_radii = CornersRefinement {
            top_left: Some(px),
            top_right: Some(px),
            bottom_right: Some(px),
            bottom_left: Some(px),
        };
        self
    }

    /// Sets the top-left corner radius.
    fn rounded_tl(mut self, px: Pixels) -> Self {
        self.outer_style().corner_radii.top_left = Some(px);
        self
    }

    /// Sets the top-right corner radius.
    fn rounded_tr(mut self, px: Pixels) -> Self {
        self.outer_style().corner_radii.top_right = Some(px);
        self
    }

    /// Sets the bottom-left corner radius.
    fn rounded_bl(mut self, px: Pixels) -> Self {
        self.outer_style().corner_radii.bottom_left = Some(px);
        self
    }

    /// Sets the bottom-right corner radius.
    fn rounded_br(mut self, px: Pixels) -> Self {
        self.outer_style().corner_radii.bottom_right = Some(px);
        self
    }

    /// Sets the corner smoothing factor (0.0 to 1.0, where 1.0 is maximum smoothing).
    fn rounded_smoothing(mut self, px: Pixels) -> Self {
        self.outer_style().corner_smoothing = Some(px);
        self
    }

    /// Controls whether to preserve smoothing when corners would overlap.
    fn rounded_preserve_smoothing(mut self, preserve: bool) -> Self {
        self.outer_style().preserve_smoothing = Some(preserve);
        self
    }

    // Sets the width of the element. [Docs](https://tailwindcss.com/docs/width)
    fn w(mut self, width: impl Into<Length>) -> Self {
        self.style().size.width = Some(width.into());
        self
    }

    // Sets the height of the element. [Docs](https://tailwindcss.com/docs/height)
    fn h(mut self, height: impl Into<Length>) -> Self {
        self.style().size.height = Some(height.into());
        self
    }

    // Sets the width and height of the element.
    fn size(mut self, size: impl Into<gpui::Length>) -> Self {
        let size = size.into();
        self.style().size = SizeRefinement {
            width: Some(size),
            height: Some(size),
        };
        self
    }

    /// Sets the element to fill its parent (width: 100%, height: 100%).
    fn size_full(mut self) -> Self {
        let full: Length = relative(1.).into();
        self.style().size = SizeRefinement {
            width: Some(full),
            height: Some(full),
        };
        self
    }

    // Sets the minimum width of the element. [Docs](https://tailwindcss.com/docs/min-width)
    fn min_w(mut self, width: impl Into<Length>) -> Self {
        self.style().min_size.width = Some(width.into());
        self
    }

    // Sets the minimum height of the element. [Docs](https://tailwindcss.com/docs/min-height)
    fn min_h(mut self, height: impl Into<Length>) -> Self {
        self.style().min_size.height = Some(height.into());
        self
    }

    // Sets the maximum width of the element. [Docs](https://tailwindcss.com/docs/max-width)
    fn max_w(mut self, width: impl Into<Length>) -> Self {
        self.style().max_size.width = Some(width.into());
        self
    }

    // Sets the maximum height of the element. [Docs](https://tailwindcss.com/docs/max-height)
    fn max_h(mut self, height: impl Into<Length>) -> Self {
        self.style().max_size.height = Some(height.into());
        self
    }

    // Sets the gap between rows and columns in flex layouts. [Docs](https://tailwindcss.com/docs/gap)
    fn gap(mut self, gap: impl Into<DefiniteLength>) -> Self {
        let gap = gap.into();

        self.style().gap = SizeRefinement {
            width: Some(gap),
            height: Some(gap),
        };
        self
    }

    // Sets the gap between columns in flex layouts. [Docs](https://tailwindcss.com/docs/gap#changing-row-and-column-gaps-independently)
    fn gap_x(mut self, gap: impl Into<DefiniteLength>) -> Self {
        self.style().gap.width = Some(gap.into());
        self
    }

    // SSets the gap between rows in flex layouts. [Docs](https://tailwindcss.com/docs/gap#changing-row-and-column-gaps-independently)
    fn gap_y(mut self, gap: impl Into<DefiniteLength>) -> Self {
        self.style().gap.height = Some(gap.into());
        self
    }

    /// Sets the display type of the element to `block`.
    /// [Docs](https://tailwindcss.com/docs/display)
    fn block(mut self) -> Self {
        self.style().display = Some(Display::Block);
        self
    }

    /// Sets the display type of the element to `flex`.
    /// [Docs](https://tailwindcss.com/docs/display)
    fn flex(mut self) -> Self {
        self.style().display = Some(Display::Flex);
        self
    }

    /// Sets the display type of the element to `grid`.
    /// [Docs](https://tailwindcss.com/docs/display)
    fn grid(mut self) -> Self {
        self.style().display = Some(Display::Grid);
        self
    }

    /// Sets the whitespace of the element to `normal`.
    /// [Docs](https://tailwindcss.com/docs/whitespace#normal)
    fn whitespace_normal(mut self) -> Self {
        self.text_style().white_space = Some(WhiteSpace::Normal);
        self
    }

    /// Sets the whitespace of the element to `nowrap`.
    /// [Docs](https://tailwindcss.com/docs/whitespace#nowrap)
    fn whitespace_nowrap(mut self) -> Self {
        self.text_style().white_space = Some(WhiteSpace::Nowrap);
        self
    }

    /// Sets the truncate overflowing text with an ellipsis (…) if needed.
    /// [Docs](https://tailwindcss.com/docs/text-overflow#ellipsis)
    fn text_ellipsis(mut self) -> Self {
        self.text_style().text_overflow = Some(TextOverflow::Truncate(ELLIPSIS));
        self
    }

    /// Sets the text overflow behavior of the element.
    fn text_overflow(mut self, overflow: TextOverflow) -> Self {
        self.text_style().text_overflow = Some(overflow);
        self
    }

    /// Set the text alignment of the element.
    fn text_align(mut self, align: TextAlign) -> Self {
        self.text_style().text_align = Some(align);
        self
    }

    /// Sets the text alignment to left
    fn text_left(self) -> Self {
        self.text_align(TextAlign::Left)
    }

    /// Sets the text alignment to center
    fn text_center(self) -> Self {
        self.text_align(TextAlign::Center)
    }

    /// Sets the text alignment to right
    fn text_right(self) -> Self {
        self.text_align(TextAlign::Right)
    }

    /// Sets the truncate to prevent text from wrapping and truncate overflowing text with an ellipsis (…) if needed.
    /// [Docs](https://tailwindcss.com/docs/text-overflow#truncate)
    fn truncate(self) -> Self {
        self.overflow_hidden().whitespace_nowrap().text_ellipsis()
    }

    /// Sets number of lines to show before truncating the text.
    /// [Docs](https://tailwindcss.com/docs/line-clamp)
    fn line_clamp(mut self, lines: usize) -> Self {
        let text_style = self.text_style();
        text_style.line_clamp = Some(lines);
        self.overflow_hidden()
    }

    /// Sets the flex direction of the element to `column`.
    /// [Docs](https://tailwindcss.com/docs/flex-direction#column)
    fn flex_col(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Column);
        self
    }

    /// Sets the flex direction of the element to `column-reverse`.
    /// [Docs](https://tailwindcss.com/docs/flex-direction#column-reverse)
    fn flex_col_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::ColumnReverse);
        self
    }

    /// Sets the flex direction of the element to `row`.
    /// [Docs](https://tailwindcss.com/docs/flex-direction#row)
    fn flex_row(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::Row);
        self
    }

    /// Sets the flex direction of the element to `row-reverse`.
    /// [Docs](https://tailwindcss.com/docs/flex-direction#row-reverse)
    fn flex_row_reverse(mut self) -> Self {
        self.style().flex_direction = Some(FlexDirection::RowReverse);
        self
    }

    /// Sets the element to allow a flex item to grow and shrink as needed, ignoring its initial size.
    /// [Docs](https://tailwindcss.com/docs/flex#flex-1)
    fn flex_1(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(relative(0.).into());
        self
    }

    /// Sets the element to allow a flex item to grow and shrink, taking into account its initial size.
    /// [Docs](https://tailwindcss.com/docs/flex#auto)
    fn flex_auto(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    /// Sets the element to allow a flex item to shrink but not grow, taking into account its initial size.
    /// [Docs](https://tailwindcss.com/docs/flex#initial)
    fn flex_initial(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(1.);
        self.style().flex_basis = Some(Length::Auto);
        self
    }

    /// Sets the element to prevent a flex item from growing or shrinking.
    /// [Docs](https://tailwindcss.com/docs/flex#none)
    fn flex_none(mut self) -> Self {
        self.style().flex_grow = Some(0.);
        self.style().flex_shrink = Some(0.);
        self
    }

    /// Sets the initial size of flex items for this element.
    /// [Docs](https://tailwindcss.com/docs/flex-basis)
    fn flex_basis(mut self, basis: impl Into<Length>) -> Self {
        self.style().flex_basis = Some(basis.into());
        self
    }

    /// Sets the element to allow a flex item to grow to fill any available space.
    /// [Docs](https://tailwindcss.com/docs/flex-grow)
    fn flex_grow(mut self) -> Self {
        self.style().flex_grow = Some(1.);
        self
    }

    /// Sets the element to allow a flex item to shrink if needed.
    /// [Docs](https://tailwindcss.com/docs/flex-shrink)
    fn flex_shrink(mut self) -> Self {
        self.style().flex_shrink = Some(1.);
        self
    }

    /// Sets the element to prevent a flex item from shrinking.
    /// [Docs](https://tailwindcss.com/docs/flex-shrink#dont-shrink)
    fn flex_shrink_0(mut self) -> Self {
        self.style().flex_shrink = Some(0.);
        self
    }

    /// Sets the element to allow flex items to wrap.
    /// [Docs](https://tailwindcss.com/docs/flex-wrap#wrap-normally)
    fn flex_wrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::Wrap);
        self
    }

    /// Sets the element wrap flex items in the reverse direction.
    /// [Docs](https://tailwindcss.com/docs/flex-wrap#wrap-reversed)
    fn flex_wrap_reverse(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::WrapReverse);
        self
    }

    /// Sets the element to prevent flex items from wrapping, causing inflexible items to overflow the container if necessary.
    /// [Docs](https://tailwindcss.com/docs/flex-wrap#dont-wrap)
    fn flex_nowrap(mut self) -> Self {
        self.style().flex_wrap = Some(FlexWrap::NoWrap);
        self
    }

    /// Sets the element to align flex items to the start of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-items#start)
    fn items_start(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexStart);
        self
    }

    /// Sets the element to align flex items to the end of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-items#end)
    fn items_end(mut self) -> Self {
        self.style().align_items = Some(AlignItems::FlexEnd);
        self
    }

    /// Sets the element to align flex items along the center of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-items#center)
    fn items_center(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Center);
        self
    }

    /// Sets the element to align flex items along the baseline of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-items#baseline)
    fn items_baseline(mut self) -> Self {
        self.style().align_items = Some(AlignItems::Baseline);
        self
    }

    /// Sets the element to justify flex items against the start of the container's main axis.
    /// [Docs](https://tailwindcss.com/docs/justify-content#start)
    fn justify_start(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Start);
        self
    }

    /// Sets the element to justify flex items against the end of the container's main axis.
    /// [Docs](https://tailwindcss.com/docs/justify-content#end)
    fn justify_end(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::End);
        self
    }

    /// Sets the element to justify flex items along the center of the container's main axis.
    /// [Docs](https://tailwindcss.com/docs/justify-content#center)
    fn justify_center(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::Center);
        self
    }

    /// Sets the element to justify flex items along the container's main axis
    /// such that there is an equal amount of space between each item.
    /// [Docs](https://tailwindcss.com/docs/justify-content#space-between)
    fn justify_between(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    /// Sets the element to justify items along the container's main axis such
    /// that there is an equal amount of space on each side of each item.
    /// [Docs](https://tailwindcss.com/docs/justify-content#space-around)
    fn justify_around(mut self) -> Self {
        self.style().justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    /// Sets the element to pack content items in their default position as if no align-content value was set.
    /// [Docs](https://tailwindcss.com/docs/align-content#normal)
    fn content_normal(mut self) -> Self {
        self.style().align_content = None;
        self
    }

    /// Sets the element to pack content items in the center of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-content#center)
    fn content_center(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Center);
        self
    }

    /// Sets the element to pack content items against the start of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-content#start)
    fn content_start(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexStart);
        self
    }

    /// Sets the element to pack content items against the end of the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-content#end)
    fn content_end(mut self) -> Self {
        self.style().align_content = Some(AlignContent::FlexEnd);
        self
    }

    /// Sets the element to pack content items along the container's cross axis
    /// such that there is an equal amount of space between each item.
    /// [Docs](https://tailwindcss.com/docs/align-content#space-between)
    fn content_between(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceBetween);
        self
    }

    /// Sets the element to pack content items along the container's cross axis
    /// such that there is an equal amount of space on each side of each item.
    /// [Docs](https://tailwindcss.com/docs/align-content#space-around)
    fn content_around(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceAround);
        self
    }

    /// Sets the element to pack content items along the container's cross axis
    /// such that there is an equal amount of space between each item.
    /// [Docs](https://tailwindcss.com/docs/align-content#space-evenly)
    fn content_evenly(mut self) -> Self {
        self.style().align_content = Some(AlignContent::SpaceEvenly);
        self
    }

    /// Sets the element to allow content items to fill the available space along the container's cross axis.
    /// [Docs](https://tailwindcss.com/docs/align-content#stretch)
    fn content_stretch(mut self) -> Self {
        self.style().align_content = Some(AlignContent::Stretch);
        self
    }

    /// Sets the border style of the element.
    fn border_dashed(mut self) -> Self {
        self.style().border_style = Some(BorderStyle::Dashed);
        self
    }

    /// Returns a mutable reference to the text style that has been configured on this element.
    fn text_style(&mut self) -> &mut TextStyleRefinement {
        let style: &mut StyleRefinement = self.style();
        &mut style.text
    }

    /// Sets the text color of this element.
    ///
    /// This value cascades to its child elements.
    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_style().color = Some(color.into());
        self
    }

    /// Sets the font weight of this element
    ///
    /// This value cascades to its child elements.
    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.text_style().font_weight = Some(weight);
        self
    }

    /// Sets the background color of this element.
    ///
    /// This value cascades to its child elements.
    fn text_bg(mut self, bg: impl Into<Hsla>) -> Self {
        self.text_style().background_color = Some(bg.into());
        self
    }

    /// Sets the text size of this element.
    ///
    /// This value cascades to its child elements.
    fn text_size(mut self, size: impl Into<AbsoluteLength>) -> Self {
        self.text_style().font_size = Some(size.into());
        self
    }

    /// Sets the text size to 'extra small'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_xs(mut self) -> Self {
        self.text_style().font_size = Some(rems(0.75).into());
        self
    }

    /// Sets the text size to 'small'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_sm(mut self) -> Self {
        self.text_style().font_size = Some(rems(0.875).into());
        self
    }

    /// Sets the text size to 'base'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_base(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.0).into());
        self
    }

    /// Sets the text size to 'large'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_lg(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.125).into());
        self
    }

    /// Sets the text size to 'extra large'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.25).into());
        self
    }

    /// Sets the text size to 'extra extra large'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_2xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.5).into());
        self
    }

    /// Sets the text size to 'extra extra extra large'.
    /// [Docs](https://tailwindcss.com/docs/font-size#setting-the-font-size)
    fn text_3xl(mut self) -> Self {
        self.text_style().font_size = Some(rems(1.875).into());
        self
    }

    /// Sets the font style of the element to italic.
    /// [Docs](https://tailwindcss.com/docs/font-style#italicizing-text)
    fn italic(mut self) -> Self {
        self.text_style().font_style = Some(FontStyle::Italic);
        self
    }

    /// Sets the font style of the element to normal (not italic).
    /// [Docs](https://tailwindcss.com/docs/font-style#displaying-text-normally)
    fn not_italic(mut self) -> Self {
        self.text_style().font_style = Some(FontStyle::Normal);
        self
    }

    /// Sets the text decoration to underline.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-line#underling-text)
    fn underline(mut self) -> Self {
        let style = self.text_style();
        style.underline = Some(UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    /// Sets the decoration of the text to have a line through it.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-line#adding-a-line-through-text)
    fn line_through(mut self) -> Self {
        let style = self.text_style();
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        });
        self
    }

    /// Removes the text decoration on this element.
    ///
    /// This value cascades to its child elements.
    fn text_decoration_none(mut self) -> Self {
        self.text_style().underline = None;
        self
    }

    /// Sets the color for the underline on this element
    fn text_decoration_color(mut self, color: impl Into<Hsla>) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.color = Some(color.into());
        self
    }

    /// Sets the text decoration style to a solid line.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-style)
    fn text_decoration_solid(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = false;
        self
    }

    /// Sets the text decoration style to a wavy line.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-style)
    fn text_decoration_wavy(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.wavy = true;
        self
    }

    /// Sets the text decoration to be 0px thick.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_0(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(0.);
        self
    }

    /// Sets the text decoration to be 1px thick.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_1(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(1.);
        self
    }

    /// Sets the text decoration to be 2px thick.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_2(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(2.);
        self
    }

    /// Sets the text decoration to be 4px thick.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_4(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(4.);
        self
    }

    /// Sets the text decoration to be 8px thick.
    /// [Docs](https://tailwindcss.com/docs/text-decoration-thickness)
    fn text_decoration_8(mut self) -> Self {
        let style = self.text_style();
        let underline = style.underline.get_or_insert_with(Default::default);
        underline.thickness = px(8.);
        self
    }

    /// Sets the font family of this element and its children.
    fn font_family(mut self, family_name: impl Into<SharedString>) -> Self {
        self.text_style().font_family = Some(family_name.into());
        self
    }

    /// Sets the font of this element and its children.
    fn font(mut self, font: Font) -> Self {
        let Font {
            family,
            features,
            fallbacks,
            weight,
            style,
        } = font;

        let text_style = self.text_style();
        text_style.font_family = Some(family);
        text_style.font_features = Some(features);
        text_style.font_weight = Some(weight);
        text_style.font_style = Some(style);
        text_style.font_fallbacks = fallbacks;

        self
    }

    /// Sets the line height of this element and its children.
    fn line_height(mut self, line_height: impl Into<DefiniteLength>) -> Self {
        self.text_style().line_height = Some(line_height.into());
        self
    }

    /// Sets the opacity of this element and its children.
    fn opacity(mut self, opacity: f32) -> Self {
        self.style().opacity = Some(opacity);
        self
    }

    /// Sets the grid columns of this element.
    fn grid_cols(mut self, cols: u16) -> Self {
        self.style().grid_cols = Some(GridTemplate {
            repeat: cols,
            min_size: GridTemplateMinSize::Zero,
        });
        self
    }

    /// Sets the grid rows of this element.
    fn grid_rows(mut self, rows: u16) -> Self {
        self.style().grid_rows = Some(GridTemplate {
            repeat: rows,
            min_size: GridTemplateMinSize::Zero,
        });
        self
    }

    /// Sets the column start of this element.
    fn col_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Line(start);
        self
    }

    /// Sets the column start of this element to auto.
    fn col_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.start = GridPlacement::Auto;
        self
    }

    /// Sets the column end of this element.
    fn col_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Line(end);
        self
    }

    /// Sets the column end of this element to auto.
    fn col_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column.end = GridPlacement::Auto;
        self
    }

    /// Sets the column span of this element.
    fn col_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    /// Sets the row span of this element.
    fn col_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.column = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    /// Sets the row start of this element.
    fn row_start(mut self, start: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Line(start);
        self
    }

    /// Sets the row start of this element to "auto"
    fn row_start_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.start = GridPlacement::Auto;
        self
    }

    /// Sets the row end of this element.
    fn row_end(mut self, end: i16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Line(end);
        self
    }

    /// Sets the row end of this element to "auto"
    fn row_end_auto(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row.end = GridPlacement::Auto;
        self
    }

    /// Sets the row span of this element.
    fn row_span(mut self, span: u16) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Span(span)..GridPlacement::Span(span);
        self
    }

    /// Sets the row span of this element.
    fn row_span_full(mut self) -> Self {
        let grid_location = self.style().grid_location_mut();
        grid_location.row = GridPlacement::Line(1)..GridPlacement::Line(-1);
        self
    }

    /// Draws a debug border around this element.
    #[cfg(debug_assertions)]
    fn debug(mut self) -> Self {
        self.style().debug = Some(true);
        self
    }

    /// Draws a debug border on all conforming elements below this element.
    #[cfg(debug_assertions)]
    fn debug_below(mut self) -> Self {
        self.style().debug_below = Some(true);
        self
    }
}
