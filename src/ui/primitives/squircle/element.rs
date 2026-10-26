//! Keep Div's layout and interactions, replacing only its painted surface.

use gpui::{
    App, Background, Bounds, Corners, CornersRefinement, Element, ElementId, FillOptions, FillRule,
    GlobalElementId, Hitbox, Hsla, InspectorElementId, InteractiveElement, Interactivity,
    IntoElement, LayoutId, Overflow, ParentElement, PathBuilder, PathStyle, Pixels,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, point, px, size,
};

use super::geometry::SquirclePath;

pub trait SquircleExt:
    Element<PrepaintState = Option<Hitbox>> + InteractiveElement + Styled + Sized
{
    fn squircle(self) -> SquircleElement<Self> {
        SquircleElement(self)
    }
}

impl SquircleExt for gpui::Div {}
impl SquircleExt for gpui::Stateful<gpui::Div> {}

pub struct SquircleElement<E>(E);

impl<E: Styled> Styled for SquircleElement<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl<E: InteractiveElement> InteractiveElement for SquircleElement<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.0.interactivity()
    }
}

impl<E: StatefulInteractiveElement> StatefulInteractiveElement for SquircleElement<E> {}

impl<E: ParentElement> ParentElement for SquircleElement<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.0.extend(elements);
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement + Styled> Element
    for SquircleElement<E>
{
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.0)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.0.source_location()
    }

    fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
        self.0.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.0.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        self.0.a11y_synthetic_children(prepaint, builder);
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.0.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.0.prepaint(id, inspector, bounds, layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = self
            .0
            .interactivity()
            .compute_style(id, hitbox.as_ref(), window, cx);
        if style.display == gpui::Display::None || style.visibility == gpui::Visibility::Hidden {
            return;
        }
        let radii = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        if radii == Corners::default()
            || (bounds.size.width == bounds.size.height
                && radii == Corners::all(bounds.size.width / 2.0))
        {
            self.0
                .paint(id, inspector, bounds, layout, hitbox, window, cx);
            return;
        }
        let opacity = style.opacity.unwrap_or(1.0);
        let shadows = style
            .box_shadow
            .iter()
            .cloned()
            .map(|mut shadow| {
                shadow.color = shadow.color.opacity(opacity);
                shadow
            })
            .collect::<Vec<_>>();
        window.paint_drop_shadows(bounds, radii, &shadows);
        if let Some(background) = style.background.as_ref().and_then(|fill| fill.color()) {
            paint_fill(window, bounds, radii, background.opacity(opacity));
            #[cfg(test)]
            super::test_support::record(window, cx, bounds, radii, background.opacity(opacity));
        }
        for shadow in shadows.iter().filter(|shadow| shadow.inset) {
            if shadow.blur_radius == px(0.0) && shadow.offset == point(px(0.0), px(0.0)) {
                paint_border(
                    window,
                    bounds,
                    radii,
                    gpui::Edges::all(shadow.spread_radius),
                    shadow.color.into(),
                );
            } else {
                window.paint_inset_shadows(bounds, radii, std::slice::from_ref(shadow));
            }
        }

        // shortcut: keep overflow on the base Div; state-dependent overflow needs a GPUI decoration hook.
        let overflow = self.0.style().overflow.clone();
        self.0.style().overflow.x = Some(Overflow::Visible);
        self.0.style().overflow.y = Some(Overflow::Visible);
        // GPUI couples box paint and listeners; clip its box away, retaining the real hitbox and child coordinates.
        let mask = window.content_mask().bounds;
        let margin = shadows
            .iter()
            .map(|shadow| shadow.blur_radius + shadow.spread_radius.abs() + shadow.offset.x.abs())
            .fold(px(1.0), Pixels::max);
        let hidden_bounds = Bounds::new(
            point(
                mask.left() - margin - bounds.size.width - px(1.0),
                bounds.top(),
            ),
            size(px(0.0), px(0.0)),
        );
        window.with_content_mask(style.overflow_mask(bounds, window.rem_size()), |window| {
            self.0
                .paint(id, inspector, hidden_bounds, layout, hitbox, window, cx);
        });
        self.0.style().overflow = overflow;
        if let Some(color) = style.border_color.filter(|color| !color.is_transparent()) {
            paint_border(
                window,
                bounds,
                radii,
                style.border_widths.to_pixels(window.rem_size()),
                color.opacity(opacity).into(),
            );
        }
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement + Styled> IntoElement
    for SquircleElement<E>
{
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

fn corner_mask(bounds: Bounds<Pixels>, radii: Corners<Pixels>) -> Option<gpui::Path<Pixels>> {
    let shape = SquirclePath::new(bounds, &corners(radii), 0.0, 0.6, true)?;
    let mut builder = PathBuilder::fill().with_style(PathStyle::Fill(
        FillOptions::default().with_fill_rule(FillRule::EvenOdd),
    ));
    builder.move_to(bounds.origin);
    builder.line_to(bounds.top_right());
    builder.line_to(bounds.bottom_right());
    builder.line_to(bounds.bottom_left());
    builder.close();
    shape.append_to(&mut builder);
    builder.build().ok()
}

pub(super) fn paint_corner_mask(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radii: Corners<Pixels>,
    backdrop: Hsla,
) {
    // shortcut: GPUI exposes rectangular clips; corner masks require a uniform backdrop.
    if radii != Corners::default()
        && let Some(mask) = corner_mask(bounds, radii)
    {
        window.paint_path(mask, backdrop);
    }
}

fn corners(radii: Corners<Pixels>) -> CornersRefinement<Pixels> {
    CornersRefinement {
        top_left: Some(radii.top_left),
        top_right: Some(radii.top_right),
        bottom_left: Some(radii.bottom_left),
        bottom_right: Some(radii.bottom_right),
    }
}

pub(crate) fn paint_fill(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radii: Corners<Pixels>,
    color: Background,
) {
    if color.is_transparent() {
        return;
    }
    let Some(path) = SquirclePath::new(bounds, &corners(radii), 0.0, 0.6, true) else {
        return;
    };
    let mut builder = PathBuilder::fill();
    path.append_to(&mut builder);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_border(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radii: Corners<Pixels>,
    widths: gpui::Edges<Pixels>,
    color: Background,
) {
    if !widths.any(|width| *width > px(0.0)) || color.is_transparent() {
        return;
    }
    let Some(outer) = SquirclePath::new(bounds, &corners(radii), 0.0, 0.6, true) else {
        return;
    };
    let mut builder = PathBuilder::fill().with_style(PathStyle::Fill(
        FillOptions::default().with_fill_rule(FillRule::EvenOdd),
    ));
    outer.append_to(&mut builder);
    let inner_bounds = Bounds::from_corners(
        bounds.origin + point(widths.left, widths.top),
        bounds.bottom_right() - point(widths.right, widths.bottom),
    );
    let inner_radii = Corners {
        top_left: (radii.top_left - widths.left.max(widths.top)).max(px(0.0)),
        top_right: (radii.top_right - widths.right.max(widths.top)).max(px(0.0)),
        bottom_left: (radii.bottom_left - widths.left.max(widths.bottom)).max(px(0.0)),
        bottom_right: (radii.bottom_right - widths.right.max(widths.bottom)).max(px(0.0)),
    };
    if let Some(inner) = SquirclePath::new(inner_bounds, &corners(inner_radii), 0.0, 0.6, true) {
        inner.append_to(&mut builder);
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, FocusHandle, Modifiers, MouseButton, Render, TestAppContext, div, rgb};

    struct Harness {
        focus: FocusHandle,
    }
    impl Render for Harness {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            super::super::test_support::clear(window, cx);
            div().p(px(10.0)).child(
                div()
                    .id("surface")
                    .track_focus(&self.focus)
                    .tab_index(0)
                    .w(px(80.0))
                    .h(px(40.0))
                    .rounded(px(7.0))
                    .bg(rgb(0xAB1234))
                    .hover(|s| s.bg(rgb(0x1256AB)))
                    .active(|s| s.bg(rgb(0x56AB12)))
                    .focus_visible(|s| s.border_1().border_color(rgb(0x123456)))
                    .overflow_hidden()
                    .p(px(8.0))
                    .child(div().size(px(8.0)).bg(rgb(0x987654)))
                    .squircle(),
            )
        }
    }

    #[gpui::test]
    fn native_states_and_child_coordinates_survive_replacing_the_box(cx: &mut TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle());
        let (_, cx) = cx.add_window_view(|_, _| Harness {
            focus: focus.clone(),
        });
        let check = |cx: &mut gpui::VisualTestContext, color| {
            cx.update(|window, cx| {
                assert!(
                    !window
                        .painted_quads()
                        .iter()
                        .any(|q| q.background.as_solid() == Some(color))
                );
                let surfaces = super::super::test_support::painted_surfaces(window, cx);
                let surface = surfaces
                    .iter()
                    .find(|q| q.background.as_solid() == Some(color))
                    .unwrap();
                let scale = window.scale_factor();
                assert_eq!(surface.bounds.size.width.0, 80.0 * scale);
                assert_eq!(surface.corner_radii.top_left.0, 7.0 * scale);
                let child = surfaces
                    .iter()
                    .find(|q| q.background.as_solid() == Some(rgb(0x987654).into()))
                    .unwrap();
                assert_eq!(child.bounds.origin.x.0, 18.0 * scale);
                assert_eq!(child.bounds.origin.y.0, 18.0 * scale);
            });
        };
        check(cx, rgb(0xAB1234).into());
        cx.simulate_mouse_move(point(px(20.0), px(20.0)), None, Modifiers::none());
        check(cx, rgb(0x1256AB).into());
        cx.simulate_mouse_down(
            point(px(20.0), px(20.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        check(cx, rgb(0x56AB12).into());
        cx.simulate_mouse_up(
            point(px(20.0), px(20.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("tab");
        cx.update(|window, _| assert!(focus.is_focused(window)));
    }

    #[test]
    fn capsules_and_asymmetric_segments_stay_inside_their_bounds() {
        for (width, height, radii) in [
            (120.0, 3.0, Corners::all(px(1.5))),
            (36.0, 20.0, Corners::all(px(10.0))),
            (80.0, 26.0, Corners::all(px(6.0))),
            (
                25.0,
                8.0,
                Corners {
                    top_left: px(4.0),
                    bottom_left: px(4.0),
                    ..Default::default()
                },
            ),
        ] {
            let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)));
            let shape = SquirclePath::new(bounds, &corners(radii), 0.0, 0.6, true).unwrap();
            let mut builder = PathBuilder::fill();
            shape.append_to(&mut builder);
            let path = builder.build().unwrap();
            for vertex in path.vertices {
                assert!(
                    vertex.xy_position.x.as_f32() >= -0.01
                        && vertex.xy_position.x.as_f32() <= width + 0.01
                );
                assert!(
                    vertex.xy_position.y.as_f32() >= -0.01
                        && vertex.xy_position.y.as_f32() <= height + 0.01
                );
            }
        }
    }

    #[test]
    fn image_corner_mask_covers_only_the_four_corners() {
        let bounds = Bounds::new(point(px(17.0), px(29.0)), size(px(96.0), px(80.0)));
        let mask = corner_mask(bounds, Corners::all(px(9.0))).unwrap();
        let covered = |point: gpui::Point<Pixels>| {
            mask.vertices.chunks_exact(3).any(|triangle| {
                let [a, b, c] = std::array::from_fn(|i| triangle[i].xy_position);
                let cross = |u: gpui::Point<Pixels>, v: gpui::Point<Pixels>| {
                    u.x.as_f32() * v.y.as_f32() - u.y.as_f32() * v.x.as_f32()
                };
                let signs = [
                    cross(b - a, point - a),
                    cross(c - b, point - b),
                    cross(a - c, point - c),
                ];
                signs.iter().all(|s| *s >= -0.0001) || signs.iter().all(|s| *s <= 0.0001)
            })
        };
        for offset in [(0.5, 0.5), (95.5, 0.5), (0.5, 79.5), (95.5, 79.5)] {
            assert!(covered(bounds.origin + point(px(offset.0), px(offset.1))));
        }
        for offset in [(48.0, 40.0), (48.0, 0.5), (0.5, 40.0), (5.0, 5.0)] {
            assert!(!covered(bounds.origin + point(px(offset.0), px(offset.1))));
        }
        for vertex in mask.vertices {
            let point = vertex.xy_position;
            assert!(point.x >= bounds.left() - px(0.01) && point.x <= bounds.right() + px(0.01));
            assert!(point.y >= bounds.top() - px(0.01) && point.y <= bounds.bottom() + px(0.01));
        }
        assert!(corner_mask(Bounds::default(), Corners::all(px(9.0))).is_none());
    }

    #[test]
    #[ignore = "manual per-image corner-mask measurement"]
    fn benchmark_image_corner_mask() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(96.0), px(80.0)));
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            std::hint::black_box(corner_mask(bounds, Corners::all(px(9.0))).unwrap());
        }
        eprintln!(
            "image corner mask: {:?} per image",
            start.elapsed() / 10_000
        );
    }

    #[test]
    #[ignore = "manual per-surface geometry measurement"]
    fn benchmark_squircle_geometry() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(160.0), px(32.0)));
        let radii = corners(Corners::all(px(7.0)));
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let shape = SquirclePath::new(bounds, &radii, 0.0, 0.6, true).unwrap();
            let mut builder = PathBuilder::fill();
            shape.append_to(&mut builder);
            std::hint::black_box(builder.build().unwrap());
        }
        eprintln!(
            "squircle geometry: {:?} per surface",
            start.elapsed() / 10_000
        );
    }
}
