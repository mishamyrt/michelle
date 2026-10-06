use std::f32::consts::TAU;
use std::time::Duration;

use gpui::{AnyElement, Hsla, Path, Pixels, Point, canvas, point, prelude::*};

use super::motion::{Pulse, pulse};
use crate::ui::sp;

const SPOKES: usize = 8;
const PERIOD: Duration = Duration::from_millis(900);

/// A system-style progress indicator: fixed rounded spokes with a clockwise
/// fading trail. The shared pulse clock also honors reduce motion.
pub fn spinner(size: f32, color: Hsla) -> AnyElement {
    spinner_pulse(size, color).into_any_element()
}

/// Half-rate progress indicator for panes that stay busy for a whole turn.
pub fn spinner_slow(size: f32, color: Hsla) -> AnyElement {
    spinner_pulse(size, color).every(2).into_any_element()
}

fn spinner_pulse(size: f32, color: Hsla) -> Pulse {
    pulse(PERIOD, move |phase| {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let diameter = bounds.size.width.min(bounds.size.height);
                for index in 0..SPOKES {
                    window.paint_path(
                        spoke_path(bounds.center(), diameter, index),
                        color.opacity(spoke_opacity(index, phase)),
                    );
                }
            },
        )
        .size(sp(size))
        .flex_none()
        .into_any_element()
    })
}

fn spoke_opacity(index: usize, phase: f32) -> f32 {
    let head = (phase * SPOKES as f32) as usize % SPOKES;
    let age = (head + SPOKES - index) % SPOKES;
    1.0 - 0.88 * age as f32 / (SPOKES - 1) as f32
}

fn spoke_path(center: Point<Pixels>, diameter: Pixels, index: usize) -> Path<Pixels> {
    let (sin, cos) = (index as f32 * TAU / SPOKES as f32).sin_cos();
    let at = |x: f32, y: f32| {
        center
            + point(
                diameter * (x * cos - y * sin),
                diameter * (x * sin + y * cos),
            )
    };
    let radius = 0.06;
    let inner = -0.26;
    let outer = -0.435;
    // Direct quadratic paths keep each spinner to one layout element and
    // avoid tessellating strokes or preparing symbol assets on every frame.
    let mut path = Path::new(at(-radius, outer));
    path.line_to(at(-radius, inner));
    path.curve_to(at(0.0, inner + radius), at(-radius, inner + radius));
    path.curve_to(at(radius, inner), at(radius, inner + radius));
    path.line_to(at(radius, outer));
    path.curve_to(at(0.0, outer - radius), at(radius, outer - radius));
    path.curve_to(at(-radius, outer), at(-radius, outer - radius));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn spokes_fit_the_indicator_and_the_trail_advances_clockwise() {
        for size in [12.0, 14.0, 18.0, 36.0] {
            let center = point(px(130.0), px(240.0));
            for index in 0..SPOKES {
                let path = spoke_path(center, px(size), index);
                assert!(path.bounds.left() >= center.x - px(size / 2.0));
                assert!(path.bounds.right() <= center.x + px(size / 2.0));
                assert!(path.bounds.top() >= center.y - px(size / 2.0));
                assert!(path.bounds.bottom() <= center.y + px(size / 2.0));
                let phase = (index as f32 + 0.5) / SPOKES as f32;
                assert_eq!(spoke_opacity(index, phase), 1.0);
                let behind = (index + SPOKES - 1) % SPOKES;
                let ahead = (index + 1) % SPOKES;
                assert!(spoke_opacity(behind, phase) > spoke_opacity(ahead, phase));
                assert_eq!(spoke_opacity(index, 0.0), spoke_opacity(index, 1.0));
            }
        }
    }

    #[test]
    #[ignore = "manual per-frame spinner geometry measurement"]
    fn benchmark_spinner_geometry() {
        let started = std::time::Instant::now();
        for _ in 0..10_000 {
            for index in 0..SPOKES {
                std::hint::black_box(spoke_path(Point::default(), px(14.0), index));
            }
        }
        eprintln!(
            "spinner geometry: {:?} per indicator",
            started.elapsed() / 10_000
        );
    }
}
