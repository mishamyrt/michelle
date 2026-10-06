//! Locally maintained GPUI squircle component.
//!
//! Adapted from brendon-felix/gpui_squircle at
//! b7fb4d4b02f003c73378e6ca12a826d9b9c6b640 (MIT). See README.md and LICENSE.
//!
//! A squircle component for [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).
//!
//! Squircles are rounded rectangles with smooth, continuous curvature that
//! look more natural than standard CSS-style rounded corners. They are
//! commonly used in Apple's design language.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::ui::squircle::{squircle, SquircleStyled};
//!
//! squircle()
//!     .rounded(px(50.))
//!     .bg(gpui::red())
//!     .absolute_expand()
//! ```

// This vendored primitive is available for UI work before its first consumer.
#![allow(dead_code)]

use gpui::{
    AnyElement, App, Background, Bounds, CornersRefinement, Element, ElementId, GlobalElementId,
    Hitbox, InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId,
    ParentElement, PathBuilder, Pixels, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, point, px,
};
mod geometry;
use geometry::SquirclePath;

mod style;
pub use style::{SquircleStyleRefinement, Styled as SquircleStyled};

/// Internal options for building and painting squircle paths.
struct BuildAndPaintOptions {
    builder: PathBuilder,
    background: Background,
}

impl BuildAndPaintOptions {
    fn fill(background: Background) -> Self {
        Self {
            builder: PathBuilder::fill(),
            background,
        }
    }

    fn stroke(background: Background, border_width: f32) -> Self {
        Self {
            builder: PathBuilder::stroke(px(border_width)),
            background,
        }
    }
}

/// Determines how the border is positioned relative to the squircle's bounds.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum BorderMode {
    /// Border is centered on the edge (default behavior).
    #[default]
    Center,
    /// Border is drawn outside the squircle's bounds.
    Outside,
    /// Border is drawn inside the squircle's bounds.
    Inside,
}

/// A squircle element for GPUI.
///
/// Squircles provide smooth, continuous corner curvature unlike standard
/// rounded rectangles. Use [`squircle()`] to create a new instance.
pub struct Squircle {
    pub style: SquircleStyleRefinement,
    interactivity: Interactivity,
    children: Vec<AnyElement>,
}

impl SquircleStyled for Squircle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style.inner
    }

    fn outer_style(&mut self) -> &mut SquircleStyleRefinement {
        &mut self.style
    }
}

impl Squircle {
    fn new() -> Self {
        Self {
            style: SquircleStyleRefinement::default(),
            interactivity: Interactivity::new(),
            children: Vec::new(),
        }
    }

    /// Makes the squircle fill its parent element completely.
    ///
    /// This sets absolute positioning with all edges at zero, making it
    /// useful as a background layer that ignores parent padding.
    pub fn absolute_expand(mut self) -> Self {
        self.style.inner = self
            .style
            .inner
            .size_full()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .right_0();

        self
    }
}

/// Calculates size and border offsets based on the border mode.
#[inline(always)]
fn get_size_and_border_offsets(border_mode: BorderMode, border_width: f32) -> (f32, f32) {
    match border_mode {
        BorderMode::Outside => (-border_width, border_width - 2.),
        BorderMode::Inside => (border_width, -(border_width - 1.)),
        BorderMode::Center => (0., 0.),
    }
}

/// Paints squircle paths using pre-extracted style values (no `&self` borrow needed).
fn paint_squircle<const N: usize>(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    size_offset: f32,
    border_offset: f32,
    corner_radii: &CornersRefinement<Pixels>,
    corner_smoothing: f32,
    preserve_smoothing: bool,
    options: [BuildAndPaintOptions; N],
) {
    let size_offset_px = px(size_offset);
    let size = bounds.size - gpui::size(size_offset_px, size_offset_px);

    let bounds = Bounds {
        origin: bounds.origin + point(size_offset_px / 2., size_offset_px / 2.),
        size,
    };
    let Some(path) = SquirclePath::new(
        bounds,
        corner_radii,
        border_offset,
        corner_smoothing,
        preserve_smoothing,
    ) else {
        return;
    };

    let mut opts = options;
    for option in &mut opts {
        path.append_to(&mut option.builder);
    }
    for BuildAndPaintOptions {
        builder,
        background,
        ..
    } in opts
    {
        if let Ok(path) = builder.build() {
            window.paint_path(path, background);
        }
    }
}

impl Element for Squircle {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.interactivity.base_style = Box::new(self.style.inner.clone());
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| {
                window.with_text_style(style.text_style().cloned(), |window| {
                    let child_layout_ids: Vec<LayoutId> = self
                        .children
                        .iter_mut()
                        .map(|child| child.request_layout(window, cx))
                        .collect();
                    window.request_layout(style, child_layout_ids, cx)
                })
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_style, scroll_offset, hitbox, window, cx| {
                window.with_element_offset(scroll_offset, |window| {
                    for child in &mut self.children {
                        child.prepaint(window, cx);
                    }
                });
                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Determine the effective background, taking hover/active state into account
        let hovered = prepaint
            .as_ref()
            .is_some_and(|hitbox| hitbox.is_hovered(window));
        let active = self.interactivity.active.unwrap_or(false);
        let background = if active {
            self.style.active_bg.or(self.style.background)
        } else if hovered {
            self.style.hover_bg.or(self.style.background)
        } else {
            self.style.background
        };

        let border_color = self.style.border_color;
        let border_width = self.style.border_width.unwrap_or_default().to_f64() as f32;
        let border_mode = self.style.border_mode.unwrap_or_default();
        let corner_radii = self.style.corner_radii.clone();
        let corner_smoothing = self.style.corner_smoothing.unwrap_or(px(0.6)).to_f64() as f32;
        let preserve_smoothing = self.style.preserve_smoothing.unwrap_or(true);

        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            prepaint.as_ref(),
            window,
            cx,
            |style, window, cx| {
                if style.display == gpui::Display::None {
                    return;
                }
                match (background, border_color) {
                    (Some(bg), None) => {
                        paint_squircle(
                            window,
                            bounds,
                            0.,
                            0.,
                            &corner_radii,
                            corner_smoothing,
                            preserve_smoothing,
                            [BuildAndPaintOptions::fill(bg)],
                        );
                    }

                    (Some(bg), Some(border_c)) => {
                        let (size_offset, border_offset) =
                            get_size_and_border_offsets(border_mode, border_width);

                        if size_offset == 0. {
                            paint_squircle(
                                window,
                                bounds,
                                0.,
                                border_offset,
                                &corner_radii,
                                corner_smoothing,
                                preserve_smoothing,
                                [
                                    BuildAndPaintOptions::fill(bg),
                                    BuildAndPaintOptions::stroke(border_c, border_width),
                                ],
                            );
                        } else {
                            paint_squircle(
                                window,
                                bounds,
                                0.,
                                0.,
                                &corner_radii,
                                corner_smoothing,
                                preserve_smoothing,
                                [BuildAndPaintOptions::fill(bg)],
                            );

                            paint_squircle(
                                window,
                                bounds,
                                size_offset,
                                border_offset,
                                &corner_radii,
                                corner_smoothing,
                                preserve_smoothing,
                                [BuildAndPaintOptions::stroke(border_c, border_width)],
                            );
                        }
                    }

                    (None, None) => (),

                    (None, Some(border_c)) => {
                        let (size_offset, border_offset) =
                            get_size_and_border_offsets(border_mode, border_width);

                        paint_squircle(
                            window,
                            bounds,
                            size_offset,
                            border_offset,
                            &corner_radii,
                            corner_smoothing,
                            preserve_smoothing,
                            [BuildAndPaintOptions::stroke(border_c, border_width)],
                        );
                    }
                }

                // Paint children on top
                for child in &mut self.children {
                    child.paint(window, cx);
                }
            },
        );
    }
}

impl IntoElement for Squircle {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl InteractiveElement for Squircle {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl StatefulInteractiveElement for Squircle {}

impl ParentElement for Squircle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Creates a new [`Squircle`] element.
///
/// # Example
///
/// ```ignore
/// use crate::ui::squircle::{squircle, SquircleStyled};
/// use gpui::px;
///
/// squircle()
///     .rounded(px(20.))
///     .bg(gpui::blue())
///     .size(px(100.))
/// ```
pub fn squircle() -> Squircle {
    Squircle::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Context, FocusHandle, KeyDownEvent, Modifiers, MouseButton, Render, TestAppContext,
    };
    use std::{cell::Cell, rc::Rc};

    struct Harness {
        focus: FocusHandle,
        activations: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.activations.clone();
            let keys = self.activations.clone();
            squircle()
                .size(px(80.0))
                .rounded(px(20.0))
                .bg(gpui::blue())
                .id("squircle-test")
                .track_focus(&self.focus)
                .tab_index(0)
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                .on_key_down(move |event: &KeyDownEvent, _, _| {
                    if event.keystroke.key == "enter" {
                        keys.set(keys.get() + 1);
                    }
                })
        }
    }

    #[gpui::test]
    fn native_mouse_and_keyboard_handlers_are_registered(cx: &mut TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle());
        let activations = Rc::new(Cell::new(0));
        let (_view, cx) = cx.add_window_view(|_, _| Harness {
            focus: focus.clone(),
            activations: activations.clone(),
        });
        let inside = point(px(40.0), px(40.0));
        cx.simulate_mouse_down(inside, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(inside, MouseButton::Left, Modifiers::none());
        assert_eq!(activations.get(), 1);
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("enter");
        assert_eq!(activations.get(), 2);
    }
}
