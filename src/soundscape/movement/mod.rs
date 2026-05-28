pub mod agent;
pub mod ngon;

use crate::audio::sound::Position;
use crate::geom::Point2;
use crate::metres::Metres;

pub use agent::Agent;
pub use ngon::Ngon;

/// Runtime movement state for an active sound — not persisted.
/// The source config movement (Fixed/Agent/Ngon params) lives in `audio::source::Movement`.
pub enum Movement {
    Fixed(Position),
    Generative(Generative),
}

pub enum Generative {
    Agent(Agent),
    Ngon(Ngon),
}

impl Movement {
    pub fn position(&self) -> Position {
        match self {
            Movement::Fixed(p) => *p,
            Movement::Generative(g) => g.position(),
        }
    }
}

impl Generative {
    pub fn position(&self) -> Position {
        match self {
            Generative::Agent(a) => a.position(),
            Generative::Ngon(n) => n.position(),
        }
    }
}

/// Axis-aligned bounding box for a set of points in exhibition space.
#[derive(Copy, Clone, Debug)]
pub struct BoundingRect {
    pub left: Metres,
    pub right: Metres,
    pub top: Metres,
    pub bottom: Metres,
}

/// Bounding box plus centroid, describing the physical space of one installation.
#[derive(Copy, Clone, Debug)]
pub struct Area {
    pub bounding_rect: BoundingRect,
    pub centroid: Point2<Metres>,
}

impl BoundingRect {
    pub fn width(self) -> Metres { self.right - self.left }
    pub fn height(self) -> Metres { self.top - self.bottom }

    pub fn middle(self) -> Point2<Metres> {
        Point2 {
            x: self.left + self.width() * 0.5,
            y: self.bottom + self.height() * 0.5,
        }
    }

    pub fn from_point(p: Point2<Metres>) -> Self {
        BoundingRect { left: p.x, right: p.x, top: p.y, bottom: p.y }
    }

    pub fn from_points<I: IntoIterator<Item = Point2<Metres>>>(points: I) -> Option<Self> {
        let mut it = points.into_iter();
        it.next().map(|first| {
            it.fold(Self::from_point(first), |rect, p| rect.with_point(p))
        })
    }

    pub fn with_point(self, p: Point2<Metres>) -> Self {
        BoundingRect {
            left: Metres(self.left.0.min(p.x.0)),
            right: Metres(self.right.0.max(p.x.0)),
            bottom: Metres(self.bottom.0.min(p.y.0)),
            top: Metres(self.top.0.max(p.y.0)),
        }
    }
}
