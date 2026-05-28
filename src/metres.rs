use serde::{Deserialize, Serialize};
use std::ops;

/// A distance in metres.  Serialises as a plain f64 to match the original format.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Metres(pub f64);

impl Metres {
    pub fn min(self, other: Self) -> Self { if other < self { other } else { self } }
    pub fn max(self, other: Self) -> Self { if other > self { other } else { self } }
}

impl ops::Add for Metres {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Metres(self.0 + rhs.0) }
}

impl ops::Sub for Metres {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Metres(self.0 - rhs.0) }
}

impl ops::Mul for Metres {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Metres(self.0 * rhs.0) }
}

impl ops::Mul<f64> for Metres {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self { Metres(self.0 * rhs) }
}

impl ops::Div for Metres {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { Metres(self.0 / rhs.0) }
}

impl ops::Div<f64> for Metres {
    type Output = Self;
    fn div(self, rhs: f64) -> Self { Metres(self.0 / rhs) }
}

impl ops::Neg for Metres {
    type Output = Self;
    fn neg(self) -> Self { Metres(-self.0) }
}

impl ops::AddAssign for Metres {
    fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
}

impl ops::SubAssign for Metres {
    fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
}

impl ops::MulAssign for Metres {
    fn mul_assign(&mut self, rhs: Self) { self.0 *= rhs.0; }
}

impl ops::DivAssign for Metres {
    fn div_assign(&mut self, rhs: Self) { self.0 /= rhs.0; }
}

impl From<f64> for Metres {
    fn from(v: f64) -> Self { Metres(v) }
}

impl From<Metres> for f64 {
    fn from(m: Metres) -> f64 { m.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metres_round_trip() {
        let m = Metres(3.14);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "3.14");
        let back: Metres = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
