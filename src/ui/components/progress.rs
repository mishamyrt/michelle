use crate::ui::squircle::SquircleExt;
use gpui::{
    App, Div, Hsla, PathBuilder, Pixels, RenderOnce, StyleRefinement, Window, canvas, div, point,
    prelude::*, px, relative,
};

use super::control::with_defaults;
use crate::ui::UiColors;

fn normalized_fraction(fraction: Option<f64>) -> Option<f32> {
    fraction
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0) as f32)
}

#[derive(IntoElement)]
pub struct ProgressBar {
    base: Div,
    fraction: Option<f64>,
    fill: Option<Hsla>,
    track: Option<Hsla>,
    minimum: f64,
    segments: Option<Vec<(f64, Hsla)>>,
}

impl ProgressBar {
    pub fn new(fraction: Option<f64>) -> Self {
        Self {
            base: div(),
            fraction,
            fill: None,
            track: None,
            minimum: 0.0,
            segments: None,
        }
    }
    pub fn fill_color(mut self, fill: Hsla) -> Self {
        self.fill = Some(fill);
        self
    }
    pub fn track_color(mut self, track: Hsla) -> Self {
        self.track = Some(track);
        self
    }
    pub fn minimum_fill(mut self, minimum: f64) -> Self {
        self.minimum = minimum;
        self
    }
    /// Prepared fractions and colors; the owner computes their amounts and total.
    pub fn segments(mut self, segments: impl IntoIterator<Item = (f64, Hsla)>) -> Self {
        self.segments = Some(segments.into_iter().collect());
        self
    }
}

impl Styled for ProgressBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let segmented = self.segments.is_some();
        let mut segments = self
            .segments
            .unwrap_or_default()
            .into_iter()
            .map(|(fraction, color)| {
                normalized_fraction(Some(fraction)).map(|fraction| (fraction, color))
            })
            .collect::<Option<Vec<_>>>();
        if let Some(segments) = &mut segments {
            segments.retain(|(fraction, _)| *fraction > 0.0);
        }
        let minimum = normalized_fraction(Some(self.minimum)).unwrap_or(0.0);
        let fraction = normalized_fraction(self.fraction)
            .filter(|_| segments.is_some())
            .map(|fraction| fraction.max(minimum));
        with_defaults(self.base, |base| {
            base.h(px(3.0))
                .w_full()
                .flex_none()
                .rounded_full()
                .bg(self.track.unwrap_or(colors.overlay_strong))
        })
        .when_some(fraction, |base, fraction| {
            let segments = segments.unwrap_or_default();
            let count = segments.len();
            let fill =
                div()
                    .h_full()
                    .w(relative(fraction))
                    .rounded_full()
                    .when(!segmented, |fill| {
                        fill.bg(self.fill.unwrap_or(colors.accent))
                    })
                    .when(segmented, |fill| fill.flex().overflow_hidden())
                    .children(segments.into_iter().enumerate().map(
                        |(index, (fraction, color))| {
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .bg(color)
                                .when(index == 0, |segment| segment.rounded_l_full())
                                .when(index + 1 == count, |segment| segment.rounded_r_full())
                                .squircle()
                        },
                    ));
            base.child(fill.squircle())
        })
        .squircle()
    }
}

#[derive(IntoElement)]
pub struct ProgressRing {
    style: StyleRefinement,
    fraction: Option<f64>,
    fill: Option<Hsla>,
    track: Option<Hsla>,
    minimum: f64,
    stroke: Pixels,
}

impl ProgressRing {
    pub fn new(fraction: Option<f64>) -> Self {
        Self {
            style: StyleRefinement::default(),
            fraction,
            fill: None,
            track: None,
            minimum: 0.0,
            stroke: px(2.5),
        }
    }
    pub fn fill_color(mut self, fill: Hsla) -> Self {
        self.fill = Some(fill);
        self
    }
    pub fn track_color(mut self, track: Hsla) -> Self {
        self.track = Some(track);
        self
    }
    pub fn minimum_fill(mut self, minimum: f64) -> Self {
        self.minimum = minimum;
        self
    }
    pub fn stroke_width(mut self, width: Pixels) -> Self {
        self.stroke = width;
        self
    }
}

impl Styled for ProgressRing {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ProgressRing {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let fill = self.fill.unwrap_or(colors.accent);
        let track = self.track.unwrap_or(colors.border_strong);
        let minimum = normalized_fraction(Some(self.minimum)).unwrap_or(0.0);
        let fraction = normalized_fraction(self.fraction).map(|fraction| fraction.max(minimum));
        let mut ring = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let diameter = bounds.size.width.min(bounds.size.height);
                let stroke = self.stroke;
                if !diameter.as_f32().is_finite()
                    || diameter <= px(0.0)
                    || !stroke.as_f32().is_finite()
                    || stroke <= px(0.0)
                    || stroke >= diameter
                {
                    return;
                }
                let center = bounds.center();
                let radius = (diameter - stroke) / 2.0;
                // A full circle needs two arcs; a single zero-length arc is rejected.
                let full_circle = |builder: &mut PathBuilder| {
                    builder.move_to(point(center.x + radius, center.y));
                    builder.arc_to(
                        point(radius, radius),
                        px(0.0),
                        false,
                        true,
                        point(center.x - radius, center.y),
                    );
                    builder.arc_to(
                        point(radius, radius),
                        px(0.0),
                        false,
                        true,
                        point(center.x + radius, center.y),
                    );
                    builder.close();
                };
                let mut track_builder = PathBuilder::stroke(stroke);
                full_circle(&mut track_builder);
                if let Ok(path) = track_builder.build() {
                    window.paint_path(path, track);
                }
                let Some(fraction) = fraction.filter(|fraction| *fraction > 0.0) else {
                    return;
                };
                let mut arc = PathBuilder::stroke(stroke);
                if fraction >= 0.999 {
                    full_circle(&mut arc);
                } else {
                    let angle = -std::f32::consts::FRAC_PI_2 + fraction * std::f32::consts::TAU;
                    arc.move_to(point(center.x, center.y - radius));
                    arc.arc_to(
                        point(radius, radius),
                        px(0.0),
                        fraction > 0.5,
                        true,
                        point(
                            center.x + radius * angle.cos(),
                            center.y + radius * angle.sin(),
                        ),
                    );
                }
                if let Ok(path) = arc.build() {
                    window.paint_path(path, fill);
                }
            },
        )
        .size(px(13.0))
        .flex_none();
        ring.style().refine(&self.style);
        ring
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractions_distinguish_unknown_from_zero_and_normalize_only_finite_values() {
        for value in [
            None,
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
        ] {
            assert_eq!(normalized_fraction(value), None);
        }
        for (value, expected) in [
            (-1.0, 0.0),
            (0.0, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
            (f64::MAX, 1.0),
        ] {
            assert_eq!(normalized_fraction(Some(value)), Some(expected));
        }
    }
}
