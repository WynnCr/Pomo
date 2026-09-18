use gpui_kit::*;
use std::f32::consts::TAU;

const TOP: f32 = -TAU / 4.0;

fn polar(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    point(
        center.x + px(radius * angle.cos()),
        center.y + px(radius * angle.sin()),
    )
}

fn stroke_arc(
    window: &mut Window,
    center: Point<Pixels>,
    radius: f32,
    width: f32,
    start: f32,
    sweep: f32,
    color: Hsla,
) {
    if sweep.abs() < 0.001 || radius <= 0.0 || width <= 0.0 || color.a <= 0.001 {
        return;
    }

    let steps = ((sweep.abs() / 0.035).ceil() as usize).clamp(2, 720);
    let mut builder = PathBuilder::stroke(px(width));
    for step in 0..=steps {
        let angle = start + sweep * (step as f32 / steps as f32);
        let p = polar(center, radius, angle);
        if step == 0 {
            builder.move_to(p);
        } else {
            builder.line_to(p);
        }
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn fill_circle(window: &mut Window, center: Point<Pixels>, radius: f32, color: Hsla) {
    if radius <= 0.0 || color.a <= 0.001 {
        return;
    }

    let steps = 48;
    let points: Vec<Point<Pixels>> = (0..steps)
        .map(|i| polar(center, radius, TOP + TAU * (i as f32 / steps as f32)))
        .collect();

    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

#[derive(Clone, Copy)]
pub struct Ring {
    pub progress: f32,
    pub accent: Hsla,
    pub track: Hsla,
    pub thickness: f32,
    pub glow: f32,
    pub burst: f32,
}

impl Ring {
    pub fn render(self) -> impl IntoElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let center = bounds.center();
                let width: f32 = bounds.size.width.into();
                let height: f32 = bounds.size.height.into();
                let extent = width.min(height);
                let radius = (extent - self.thickness) / 2.0 - 1.0;
                if radius <= 1.0 {
                    return;
                }

                let progress = self.progress.clamp(0.0, 1.0);
                let sweep = TAU * progress;

                if self.glow > 0.01 {
                    stroke_arc(
                        window,
                        center,
                        radius,
                        self.thickness * 2.6,
                        TOP,
                        sweep,
                        self.accent.alpha(0.05 + 0.07 * self.glow),
                    );
                    stroke_arc(
                        window,
                        center,
                        radius,
                        self.thickness * 1.7,
                        TOP,
                        sweep,
                        self.accent.alpha(0.06 + 0.1 * self.glow),
                    );
                }

                stroke_arc(window, center, radius, self.thickness, TOP, TAU, self.track);

                stroke_arc(
                    window,
                    center,
                    radius,
                    self.thickness,
                    TOP,
                    sweep,
                    self.accent,
                );

                let cap = self.thickness / 2.0;
                if progress > 0.002 {
                    if progress > 0.01 {
                        fill_circle(window, polar(center, radius, TOP), cap, self.accent);
                    }

                    let head = polar(center, radius, TOP + sweep);
                    fill_circle(window, head, cap, self.accent);
                    fill_circle(
                        window,
                        head,
                        cap * (0.45 + 0.2 * self.glow),
                        self.accent.alpha(0.35 + 0.45 * self.glow),
                    );
                }

                if self.burst > 0.001 {
                    let t = 1.0 - self.burst;
                    stroke_arc(
                        window,
                        center,
                        radius + t * 34.0,
                        self.thickness * (0.9 - 0.6 * t).max(0.1),
                        TOP,
                        TAU,
                        self.accent.alpha(self.burst * 0.55),
                    );
                }
            },
        )
        .size_full()
    }
}

#[derive(Clone, Copy)]
pub struct PlayPause {
    pub t: f32,
    pub color: Hsla,
    pub size: f32,
}

const PLAY_LEFT: [(f32, f32); 4] = [(0.14, 0.02), (0.53, 0.26), (0.53, 0.74), (0.14, 0.98)];
const PLAY_RIGHT: [(f32, f32); 4] = [(0.53, 0.26), (0.93, 0.50), (0.93, 0.50), (0.53, 0.74)];
const PAUSE_LEFT: [(f32, f32); 4] = [(0.12, 0.02), (0.38, 0.02), (0.38, 0.98), (0.12, 0.98)];
const PAUSE_RIGHT: [(f32, f32); 4] = [(0.62, 0.02), (0.88, 0.02), (0.88, 0.98), (0.62, 0.98)];

impl PlayPause {
    pub fn render(self) -> impl IntoElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let t = self.t.clamp(0.0, 1.0);
                let extent = self.size;
                let origin = point(
                    bounds.origin.x + (bounds.size.width - px(extent)) / 2.0,
                    bounds.origin.y + (bounds.size.height - px(extent)) / 2.0,
                );

                let quad = |from: &[(f32, f32); 4], to: &[(f32, f32); 4]| {
                    let points: Vec<Point<Pixels>> = from
                        .iter()
                        .zip(to.iter())
                        .map(|(a, b)| {
                            point(
                                origin.x + px((a.0 + (b.0 - a.0) * t) * extent),
                                origin.y + px((a.1 + (b.1 - a.1) * t) * extent),
                            )
                        })
                        .collect();
                    let mut builder = PathBuilder::fill();
                    builder.add_polygon(&points, true);
                    builder.build().ok()
                };

                for path in [
                    quad(&PLAY_LEFT, &PAUSE_LEFT),
                    quad(&PLAY_RIGHT, &PAUSE_RIGHT),
                ]
                .into_iter()
                .flatten()
                {
                    window.paint_path(path, self.color);
                }
            },
        )
        .size_full()
    }
}
