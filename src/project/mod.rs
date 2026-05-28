pub mod config;
pub use config::Config;

use crate::audio::source;
use crate::camera::Camera;
use crate::installation::{self, Installation};
use crate::master::Master;
use crate::soundscape::group;
use crate::{audio, soundscape};
use fxhash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_PROJECT_NAME: &str = "Default Project";

const PROJECTS_DIRECTORY_STEM: &str = "projects";
const STATE_FILE_STEM: &str = "state";
const CONFIG_FILE_STEM: &str = "config";
const STATE_EXTENSION: &str = "json";
const CONFIG_EXTENSION: &str = "json";
const AUDIO_DIRECTORY_STEM: &str = "audio";

// ── Type aliases matching the original project::State ────────────────────────

pub type Installations = FxHashMap<installation::Id, Installation>;
pub type SoundscapeGroups = FxHashMap<group::Id, SoundscapeGroup>;
pub type Speakers = FxHashMap<audio::speaker::Id, Speaker>;
pub type SourcesMap = FxHashMap<source::Id, Source>;
pub type SoloedSources = FxHashSet<source::Id>;

// ── Wrapper types (name + audio data) ────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundscapeGroup {
    pub name: String,
    pub soundscape: soundscape::group::Group,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub audio: audio::speaker::Speaker,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub audio: audio::source::Source,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sources {
    #[serde(default)]
    pub map: SourcesMap,
    #[serde(default)]
    pub soloed: SoloedSources,
}

// ── State ─────────────────────────────────────────────────────────────────────

/// All persisted state for a single project (saved to `<project>/state.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default = "default_project_name")]
    pub name: String,
    #[serde(default)]
    pub master: Master,
    #[serde(default = "default_installations")]
    pub installations: Installations,
    #[serde(default)]
    pub soundscape_groups: SoundscapeGroups,
    #[serde(default)]
    pub speakers: Speakers,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub camera: Camera,
}

fn default_project_name() -> String { DEFAULT_PROJECT_NAME.into() }

/// Produce the default set of Beyond Perception installations (the original museum deployment).
fn default_installations() -> Installations {
    const NAMES: &[&str] = &[
        "Waves At Work",
        "Ripples In Spacetime",
        "Energetic Vibrations - Audio Visualiser",
        "Energetic Vibrations - Projection Mapping",
        "Turbulent Encounters",
        "Cacophony",
        "Wrapped In Spectrum",
        "Turret 1",
        "Turret 2",
    ];
    NAMES
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            (
                installation::Id(i),
                Installation { name: name.into(), ..Default::default() },
            )
        })
        .collect()
}

// ── Project ───────────────────────────────────────────────────────────────────

pub struct Project {
    pub config: Config,
    pub state: State,
}

impl Project {
    fn state_path(assets: &Path, slug: &str) -> PathBuf {
        assets
            .join(PROJECTS_DIRECTORY_STEM)
            .join(slug)
            .join(STATE_FILE_STEM)
            .with_extension(STATE_EXTENSION)
    }

    fn config_path(assets: &Path, slug: &str) -> PathBuf {
        assets
            .join(PROJECTS_DIRECTORY_STEM)
            .join(slug)
            .join(CONFIG_FILE_STEM)
            .with_extension(CONFIG_EXTENSION)
    }

    pub fn audio_path(assets: &Path, slug: &str) -> PathBuf {
        assets
            .join(PROJECTS_DIRECTORY_STEM)
            .join(slug)
            .join(AUDIO_DIRECTORY_STEM)
    }

    pub fn load(assets: &Path, slug: &str) -> Self {
        let state: State =
            crate::utils::load_from_json_or_default(&Self::state_path(assets, slug));
        let config: Config =
            crate::utils::load_from_json_or_default(&Self::config_path(assets, slug));
        Project { config, state }
    }

    pub fn save(&self, assets: &Path, slug: &str) -> anyhow::Result<()> {
        crate::utils::save_to_json(&Self::state_path(assets, slug), &self.state)?;
        crate::utils::save_to_json(&Self::config_path(assets, slug), &self.config)?;
        Ok(())
    }

    /// Save project `state` and `config` directly, without constructing a `Project`.
    pub fn save_parts(
        assets: &Path,
        slug: &str,
        state: &State,
        config: &Config,
    ) -> anyhow::Result<()> {
        crate::utils::save_to_json(&Self::state_path(assets, slug), state)?;
        crate::utils::save_to_json(&Self::config_path(assets, slug), config)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip() {
        let state = State {
            name: "Test".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(state.name, back.name);
    }

    #[test]
    fn default_installations_count() {
        let insts = default_installations();
        assert_eq!(insts.len(), 9);
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            name: default_project_name(),
            master: Default::default(),
            installations: default_installations(),
            soundscape_groups: Default::default(),
            speakers: Default::default(),
            sources: Default::default(),
            camera: Default::default(),
        }
    }
}
