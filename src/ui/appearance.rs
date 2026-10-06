//! Resolved control colors stay in memory; selecting and loading themes belongs
//! to the application. A generation invalidates theme-sensitive caches.

use super::UiColors;
use gpui::{App, Global};

#[derive(Clone, Copy)]
struct ActiveColors(UiColors, u64);
impl Global for ActiveColors {}

pub fn colors(cx: &App) -> UiColors {
    cx.try_global::<ActiveColors>()
        .map_or_else(UiColors::dark, |active| active.0)
}

pub fn generation(cx: &App) -> u64 {
    cx.try_global::<ActiveColors>().map_or(0, |active| active.1)
}

/// Publish control colors when the host applies a theme, before refreshing it.
pub fn set_colors(colors: UiColors, cx: &mut App) {
    let generation = generation(cx).wrapping_add(1);
    cx.set_global(ActiveColors(colors, generation));
}

impl UiColors {
    pub fn current(cx: &App) -> Self {
        colors(cx)
    }
    pub fn generation(cx: &App) -> u64 {
        generation(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn applying_colors_publishes_colors_and_invalidates_cached_generations(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            assert_eq!(generation(cx), 0);
            assert_eq!(colors(cx), UiColors::dark());
            set_colors(UiColors::light(), cx);
            let previous = generation(cx);
            assert_eq!(colors(cx), UiColors::light());
            set_colors(UiColors::dark(), cx);
            assert_eq!(colors(cx), UiColors::dark());
            assert_ne!(generation(cx), previous);
        });
    }
}
