use crate::audio::sound::Position;
use crate::geom::Point2;
use crate::metres::Metres;
use std::time::Duration;

use super::BoundingRect;

/// N-gon path-tracing movement: the sound traces edges of a regular N-gon inscribed in the
/// installation's bounding rectangle.
#[derive(Debug)]
pub struct Ngon {
    pub vertices: usize,
    /// Travel between every `nth` vertex (e.g. 2 = star polygon).
    pub nth: usize,
    /// Normalised radius of the N-gon — (1.0, 1.0) fills the installation bounding rect.
    pub normalised_dimensions: [f64; 2],
    pub radians_offset: f64,
    /// Travel speed in metres per second.
    pub speed: f64,

    // mutable state
    line_start: usize,
    line_end: usize,
    lerp: f64,
    sound_position: Position,
}

impl Ngon {
    pub fn new(
        vertices: usize,
        nth: usize,
        normalised_dimensions: [f64; 2],
        radians_offset: f64,
        speed: f64,
        bounding_rect: &BoundingRect,
    ) -> Self {
        assert!(vertices >= 3, "Ngon requires at least 3 vertices");
        let nth = nth.max(1);
        let (middle, half) = middle_and_half(bounding_rect, normalised_dimensions);
        let start_point = vertex(vertices, middle, half, radians_offset, 0);
        let sound_position = Position {
            point: Point2::new(Metres(start_point[0]), Metres(start_point[1])),
            radians: 0.0,
        };
        Ngon {
            vertices,
            nth,
            normalised_dimensions,
            radians_offset,
            speed,
            line_start: 0,
            line_end: nth % vertices,
            lerp: 0.0,
            sound_position,
        }
    }

    pub fn position(&self) -> Position {
        self.sound_position
    }

    /// Advance the N-gon tracer by `delta_time` within the given bounding rectangle.
    pub fn update(&mut self, delta_time: &Duration, bounding_rect: &BoundingRect) {
        let (middle, half) = middle_and_half(bounding_rect, self.normalised_dimensions);
        let v = |i| vertex(self.vertices, middle, half, self.radians_offset, i);

        let mut travel_left = self.speed * delta_time.as_secs_f64();

        loop {
            let start_pt = v(self.line_start);
            let end_pt = v(self.line_end);
            let current = lerp2(start_pt, end_pt, self.lerp);
            let dist_to_end = dist(current, end_pt);

            if travel_left == 0.0 || dist_to_end < 1e-10 {
                self.sound_position.point =
                    Point2::new(Metres(current[0]), Metres(current[1]));
                return;
            }

            if travel_left < dist_to_end {
                let seg_len = dist(start_pt, end_pt);
                let new_lerp = if seg_len > 1e-10 {
                    (seg_len - (dist_to_end - travel_left)) / seg_len
                } else {
                    self.lerp
                };
                let new_pt = lerp2(start_pt, end_pt, new_lerp);
                self.sound_position.point =
                    Point2::new(Metres(new_pt[0]), Metres(new_pt[1]));
                self.lerp = new_lerp;
                return;
            }

            travel_left -= dist_to_end;
            self.lerp = 0.0;
            self.line_start = self.line_end;
            self.line_end = (self.line_end + self.nth) % self.vertices;
        }
    }
}

fn vertex(
    total: usize,
    middle: [f64; 2],
    half: [f64; 2],
    radians_offset: f64,
    index: usize,
) -> [f64; 2] {
    let step = index as f64 / total as f64;
    let angle = step * 2.0 * std::f64::consts::PI + radians_offset;
    [
        middle[0] + half[0] * angle.cos(),
        middle[1] + half[1] * angle.sin(),
    ]
}

fn middle_and_half(rect: &BoundingRect, norm_dims: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    let m = rect.middle();
    let middle = [m.x.0, m.y.0];
    let half = [
        rect.width().0 * norm_dims[0] * 0.5,
        rect.height().0 * norm_dims[1] * 0.5,
    ];
    (middle, half)
}

fn lerp2(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}
