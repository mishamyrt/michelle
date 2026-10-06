use gpui::{AnyElement, Hsla, Svg, img, prelude::*, px, svg};

/// A monochrome symbol or brand mark, tinted via text color and sized in
/// fixed logical pixels.
pub fn icon(name: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(name)
        .w(px(size))
        .h(px(size))
        .flex_none()
        .text_color(color)
}

/// File symbols follow the theme; language and tool marks keep authored colors.
pub fn file_icon(name: &'static str, size: f32, color: Hsla) -> AnyElement {
    if name.starts_with("icons/") {
        img(name)
            .w(px(size))
            .h(px(size))
            .flex_none()
            .into_any_element()
    } else {
        icon(name, size, color).into_any_element()
    }
}
