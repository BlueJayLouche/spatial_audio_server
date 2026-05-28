use crate::geom::Point2;
use crate::installation;
use crate::metres::Metres;
use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};

/// Speaker Ids use `u64` to match the original format (JSON compatibility).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Id(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Speaker {
    pub point: Point2<Metres>,
    pub channel: usize,
    #[serde(default)]
    pub installations: FxHashSet<installation::Id>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_round_trip() {
        let s = Speaker {
            point: Point2::new(Metres(1.0), Metres(2.5)),
            channel: 3,
            installations: Default::default(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Speaker = serde_json::from_str(&json).unwrap();
        assert_eq!(s.channel, back.channel);
        assert_eq!(s.point.x, back.point.x);
    }
}
