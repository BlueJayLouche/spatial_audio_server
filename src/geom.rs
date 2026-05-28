use serde::{Deserialize, Serialize};
use std::ops;

/// A 2D point.  Serializes as `{"x": ..., "y": ...}` to match the original nannou Point2 format
/// so existing project JSON files load without changes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point2<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point2<T> {
    pub fn new(x: T, y: T) -> Self {
        Point2 { x, y }
    }
}

impl<T: Copy> From<[T; 2]> for Point2<T> {
    fn from(arr: [T; 2]) -> Self {
        Point2 { x: arr[0], y: arr[1] }
    }
}

impl<T: Copy + ops::Add<Output = T>> ops::Add for Point2<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Point2 { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

impl<T: Copy + ops::Sub<Output = T>> ops::Sub for Point2<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Point2 { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}
