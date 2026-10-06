//! Surface geometry for tests: GPUI exposes quads, but not painted paths.

use gpui::{App, Background, Bounds, Corners, Global, Pixels, Quad, Window, WindowId};
use std::collections::HashMap;

#[derive(Default)]
struct Surfaces(HashMap<WindowId, Vec<Quad>>);
impl Global for Surfaces {}

pub(crate) fn clear(window: &Window, cx: &mut App) {
    if !cx.has_global::<Surfaces>() {
        cx.set_global(Surfaces::default());
    }
    cx.global_mut::<Surfaces>()
        .0
        .entry(window.window_handle().window_id())
        .or_default()
        .clear();
}

pub(super) fn record(
    window: &Window,
    cx: &mut App,
    bounds: Bounds<Pixels>,
    radii: Corners<Pixels>,
    background: Background,
) {
    if !cx.has_global::<Surfaces>() {
        return;
    }
    let scale = window.scale_factor();
    cx.global_mut::<Surfaces>()
        .0
        .entry(window.window_handle().window_id())
        .or_default()
        .push(Quad {
            order: 0,
            bounds: bounds.scale(scale),
            content_mask: gpui::ContentMask {
                bounds: window.content_mask().bounds.scale(scale),
            },
            background,
            border_color: gpui::transparent_black(),
            corner_radii: radii.scale(scale),
            border_widths: Default::default(),
            border_style: Default::default(),
        });
}

pub(crate) fn painted_surfaces(window: &Window, cx: &App) -> Vec<Quad> {
    let mut quads = window.painted_quads();
    if let Some(surfaces) = cx.try_global::<Surfaces>() {
        quads.extend(
            surfaces
                .0
                .get(&window.window_handle().window_id())
                .into_iter()
                .flatten()
                .cloned(),
        );
    }
    quads
}
