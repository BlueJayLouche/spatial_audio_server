use crate::geom::Point2;
use crate::metres::Metres;
use serde::{Deserialize, Serialize};

pub type Point = Point2<Metres>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera {
    #[serde(default = "default_position")]
    pub position: Point,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
    #[serde(default = "default_floorplan_pixels_per_metre")]
    pub floorplan_pixels_per_metre: f64,
}

impl Camera {
    pub fn metres_to_scalar(&self, Metres(metres): Metres) -> f64 {
        self.zoom * metres * self.floorplan_pixels_per_metre
    }

    pub fn scalar_to_metres(&self, scalar: f64) -> Metres {
        Metres((scalar / self.zoom) / self.floorplan_pixels_per_metre)
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            position: default_position(),
            zoom: default_zoom(),
            floorplan_pixels_per_metre: default_floorplan_pixels_per_metre(),
        }
    }
}

fn default_position() -> Point { Point2::new(Metres(0.0), Metres(0.0)) }
fn default_zoom() -> f64 { 0.0 }
fn default_floorplan_pixels_per_metre() -> f64 { 94.0 }
