use crate::project;
use serde::{Deserialize, Serialize};
use slug::slugify;

/// Top-level application config, persisted to `assets/config.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub project_default: project::Config,
    #[serde(default = "default_project_slug")]
    pub selected_project_slug: String,
    #[serde(default)]
    pub cpu_saving_mode: bool,
    #[serde(default)]
    pub target_input_device_name: String,
    #[serde(default)]
    pub target_output_device_name: String,
}

fn default_project_slug() -> String {
    slugify(project::DEFAULT_PROJECT_NAME)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            project_default: Default::default(),
            selected_project_slug: default_project_slug(),
            cpu_saving_mode: false,
            target_input_device_name: String::new(),
            target_output_device_name: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let c = Config::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c.selected_project_slug, back.selected_project_slug);
    }
}
