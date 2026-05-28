use crate::utils::Range;
use serde::{Deserialize, Deserializer, Serialize};

/// A memory-efficient unique identifier for an installation.
///
/// Preserves a legacy deserialisation shim: old project files stored the Id as a
/// PascalCase enum variant string (e.g. `"WavesAtWork"`); newer files store it as an
/// integer.  Both are accepted.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize)]
pub struct Id(pub usize);

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::String(s) => {
                let id = match s.as_str() {
                    "WavesAtWork"                            => Id(0),
                    "RipplesInSpacetime"                     => Id(1),
                    "EnergeticVibrationsAudioVisualiser"     => Id(2),
                    "EnergeticVibrationsProjectionMapping"   => Id(3),
                    "TurbulentEncounters"                    => Id(4),
                    "Cacophony"                              => Id(5),
                    "WrappedInSpectrum"                      => Id(6),
                    "Turret1"                                => Id(7),
                    "Turret2"                                => Id(8),
                    s => Id(s.parse::<usize>().map_err(serde::de::Error::custom)?),
                };
                Ok(id)
            }
            serde_json::Value::Number(n) => {
                let u = n.as_u64()
                    .ok_or_else(|| serde::de::Error::custom("expected u64 for installation Id"))?;
                Ok(Id(u as usize))
            }
            other => Err(serde::de::Error::custom(format!(
                "expected String or Number for installation::Id, got {other:?}"
            ))),
        }
    }
}

/// A single installation within the exhibition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installation {
    #[serde(default = "default::name_string")]
    pub name: String,
    #[serde(default)]
    pub computers: computer::Addresses,
    #[serde(default)]
    pub soundscape: Soundscape,
}

impl Default for Installation {
    fn default() -> Self {
        Installation {
            name: default::name_string(),
            computers: Default::default(),
            soundscape: Default::default(),
        }
    }
}

/// Soundscape constraints for a single installation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Soundscape {
    #[serde(default = "default::simultaneous_sounds")]
    pub simultaneous_sounds: Range<usize>,
}

impl Default for Soundscape {
    fn default() -> Self {
        Soundscape { simultaneous_sounds: default::SIMULTANEOUS_SOUNDS }
    }
}

/// Derive the OSC address string for an installation from its name.
pub fn osc_addr_string(name: &str) -> String {
    format!("/{}", slug::slugify(name))
}

pub mod default {
    use crate::utils::Range;

    pub const SIMULTANEOUS_SOUNDS: Range<usize> = Range { min: 1, max: 8 };

    pub fn name() -> &'static str { "<unnamed>" }
    pub fn name_string() -> String { name().into() }
    pub fn simultaneous_sounds() -> Range<usize> { SIMULTANEOUS_SOUNDS }
}

pub mod computer {
    use fxhash::FxHashMap;
    use serde::{Deserialize, Serialize};
    use std::net;

    #[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Id(pub usize);

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Address {
        pub socket: net::SocketAddrV4,
        pub osc_addr: String,
    }

    pub type Addresses = FxHashMap<Id, Address>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_roundtrip_integer() {
        let id = Id(3);
        let json = serde_json::to_string(&id).unwrap();
        let back: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn id_legacy_string() {
        let back: Id = serde_json::from_str(r#""Cacophony""#).unwrap();
        assert_eq!(back, Id(5));
    }

    #[test]
    fn installation_round_trip() {
        let inst = Installation::default();
        let json = serde_json::to_string(&inst).unwrap();
        let back: Installation = serde_json::from_str(&json).unwrap();
        assert_eq!(inst.name, back.name);
    }
}
