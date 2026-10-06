//! Corner geometry adapted from figma_squircle 0.1.0 (MIT; see LICENSE).
//!
//! Build GPUI paths directly instead of formatting and reparsing SVG per paint.

use gpui::{Bounds, CornersRefinement, PathBuilder, Pixels, Point, point, px};

#[derive(Clone, Copy, Default)]
struct Corner {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    p: f32,
    radius: f32,
    arc: f32,
}

impl Corner {
    fn new(radius: f32, mut smoothing: f32, preserve: bool, budget: f32) -> Self {
        if radius == 0.0 {
            return Self::default();
        }

        // Figma's smoothing construction: cubic, circular arc, cubic.
        let mut p = (1.0 + smoothing) * radius;
        if !preserve {
            smoothing = smoothing.min(budget / radius - 1.0);
            p = p.min(budget);
        }

        let arc_measure = 90.0 * (1.0 - smoothing);
        let arc = (arc_measure / 2.0).to_radians().sin() * radius * std::f32::consts::SQRT_2;
        let alpha = (90.0 - arc_measure) / 2.0;
        let p3_to_p4 = radius * (alpha / 2.0).to_radians().tan();
        let beta = (45.0 * smoothing).to_radians();
        let c = p3_to_p4 * beta.cos();
        let d = c * beta.tan();
        let mut b = (p - arc - c - d) / 3.0;
        let mut a = 2.0 * b;

        if preserve && p > budget {
            let max_distance = budget - d - arc - c;
            let min_a = max_distance / 6.0;
            b = b.min(max_distance - min_a);
            a = max_distance - b;
            p = p.min(budget);
        }

        Self {
            a,
            b,
            c,
            d,
            p,
            radius,
            arc,
        }
    }
}

pub(super) struct SquirclePath {
    bounds: Bounds<Pixels>,
    // Clockwise, starting at the top left.
    corners: [Corner; 4],
}

impl SquirclePath {
    pub(super) fn new(
        bounds: Bounds<Pixels>,
        radii: &CornersRefinement<Pixels>,
        radius_offset: f32,
        smoothing: f32,
        preserve: bool,
    ) -> Option<Self> {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let radii = [
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        ]
        .map(|radius| (f32::from(radius.unwrap_or_default()) + radius_offset).max(0.0));
        let (radii, budgets) = normalize(width, height, radii);
        let smoothing = smoothing.clamp(0.0, 1.0);
        Some(Self {
            bounds,
            corners: std::array::from_fn(|i| {
                Corner::new(radii[i], smoothing, preserve, budgets[i])
            }),
        })
    }

    pub(super) fn append_to(&self, builder: &mut PathBuilder) {
        let width = f32::from(self.bounds.size.width);
        let height = f32::from(self.bounds.size.height);
        let [tl, tr, br, bl] = self.corners;
        let starts = [
            (width - tr.p, 0.0),
            (width, height - br.p),
            (bl.p, height),
            (0.0, tl.p),
        ];
        for (i, (corner, (x, y))) in [tr, br, bl, tl].into_iter().zip(starts).enumerate() {
            let start = self.bounds.origin + point(px(x), px(y));
            if i == 0 {
                builder.move_to(start);
            } else {
                builder.line_to(start);
            }
            if corner.radius == 0.0 {
                continue;
            }

            // Rotate the same top-right construction for the other three corners.
            let at = |x: f32, y: f32| -> Point<Pixels> {
                let (x, y) = match i {
                    0 => (x, y),
                    1 => (-y, x),
                    2 => (-x, -y),
                    _ => (y, -x),
                };
                start + point(px(x), px(y))
            };
            let Corner {
                a, b, c, d, arc, ..
            } = corner;
            let first_x = a + b + c;
            builder.cubic_bezier_to(at(first_x, d), at(a, 0.0), at(a + b, 0.0));
            if arc > 0.0 {
                builder.arc_to(
                    point(px(corner.radius), px(corner.radius)),
                    px(0.0),
                    false,
                    true,
                    at(first_x + arc, d + arc),
                );
            }
            builder.cubic_bezier_to(
                at(first_x + arc + d, d + arc + a + b + c),
                at(first_x + arc + d, d + arc + c),
                at(first_x + arc + d, d + arc + b + c),
            );
        }
        builder.close();
    }
}

/// Distribute each side between adjacent corners, largest radii first.
/// Fixed arrays replace the upstream HashMap and phf adjacency map.
fn normalize(width: f32, height: f32, mut radii: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    if radii.iter().all(|radius| *radius == radii[0]) {
        let budget = width.min(height) / 2.0;
        return ([radii[0].min(budget); 4], [budget; 4]);
    }
    let mut order = [0, 1, 3, 2]; // Preserve the upstream tie order.
    order.sort_by(|a, b| radii[*b].total_cmp(&radii[*a]));
    let mut budgets = [-1.0; 4];
    let adjacents = [
        [(1, width), (3, height)],
        [(0, width), (2, height)],
        [(3, width), (1, height)],
        [(2, width), (0, height)],
    ];
    for i in order {
        let radius = radii[i];
        let budget = adjacents[i]
            .map(|(other, side)| {
                if radius == 0.0 && radii[other] == 0.0 {
                    0.0
                } else if budgets[other] >= 0.0 {
                    side - budgets[other]
                } else {
                    radius / (radius + radii[other]) * side
                }
            })
            .into_iter()
            .fold(f32::INFINITY, f32::min)
            .max(0.0);
        budgets[i] = budget;
        radii[i] = radius.min(budget);
    }
    (radii, budgets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn smoothing_matches_upstream_corner_geometry() {
        let round = Corner::new(20.0, 0.0, false, 100.0);
        assert!((round.arc - 20.0).abs() < 0.0001);
        assert!((round.a + round.b + round.c + round.d).abs() < 0.0001);

        // Reference values from figma_squircle 0.1.0 for a 20px, fully smooth corner.
        let smooth = Corner::new(20.0, 1.0, true, 100.0);
        for (actual, expected) in [
            (smooth.p, 40.0),
            (smooth.arc, 0.0),
            (smooth.a, 18.856182),
            (smooth.b, 9.428091),
            (smooth.c, 5.8578644),
            (smooth.d, 5.8578644),
        ] {
            assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
        }
        assert_eq!(Corner::new(20.0, 1.0, true, 20.0).p, 20.0);
    }

    #[test]
    fn direct_paths_tessellate_within_bounds_for_all_corners() {
        let bounds = Bounds {
            origin: point(px(13.0), px(27.0)),
            size: size(px(120.0), px(90.0)),
        };
        for radii in [
            [0.0; 4],
            [20.0; 4],
            [500.0; 4],
            [0.0, 75.0, 10.0, 150.0],
            [-5.0, 15.0, 35.0, 0.0],
        ] {
            let radii = CornersRefinement {
                top_left: Some(px(radii[0])),
                top_right: Some(px(radii[1])),
                bottom_right: Some(px(radii[2])),
                bottom_left: Some(px(radii[3])),
            };
            for smoothing in [0.0, 0.6, 1.0] {
                for preserve in [false, true] {
                    let shape =
                        SquirclePath::new(bounds, &radii, 0.0, smoothing, preserve).unwrap();
                    let mut builder = PathBuilder::fill();
                    shape.append_to(&mut builder);
                    let path = builder.build().unwrap();
                    assert!(!path.vertices.is_empty());
                    for vertex in &path.vertices {
                        let x = f32::from(vertex.xy_position.x);
                        let y = f32::from(vertex.xy_position.y);
                        assert!(x.is_finite() && y.is_finite());
                        assert!((12.99..=133.01).contains(&x), "x={x}");
                        assert!((26.99..=117.01).contains(&y), "y={y}");
                    }
                }
            }
        }
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                SquirclePath::new(
                    Bounds {
                        size: size(px(width), px(90.0)),
                        ..bounds
                    },
                    &CornersRefinement::default(),
                    0.0,
                    1.0,
                    true
                )
                .is_none()
            );
        }
    }
}
