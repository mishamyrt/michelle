use crate::ui::sp;
use gpui::{AnyElement, Hsla, Svg, img, prelude::*, svg};

/// A monochrome system symbol (or brand mark), tinted via text color. Sized in
/// `sp` so icons keep pace with the chrome text they sit beside when the UI
/// font size setting moves.
pub fn icon(name: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(name)
        .w(sp(size))
        .h(sp(size))
        .flex_none()
        .text_color(color)
}

/// File symbols follow the theme; language and tool marks keep authored colors.
pub fn file_icon(name: &'static str, size: f32, color: Hsla) -> AnyElement {
    if name.starts_with("icons/") {
        img(name)
            .w(sp(size))
            .h(sp(size))
            .flex_none()
            .into_any_element()
    } else {
        icon(name, size, color).into_any_element()
    }
}
