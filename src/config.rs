use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write,
    path::PathBuf,
    process,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::Shell;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    dirs: Vec<String>,
    profile: HashMap<String, HashMap<String, String>>,
    project: HashMap<String, ProjectValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectValue {
    Config(ProjectConfig),
    Profiles(Vec<String>),
}

impl Config {
    pub fn read() -> Self {
        let config_path = shellexpand::tilde("~/.config/cryptenv.toml");

        let config = std::fs::read_to_string(&*config_path).expect("Could not read config file");

        match toml::from_str(&config) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Could not parse config file: {}", e);
                process::exit(1);
            }
        }
    }

    pub fn dirs(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .map(shellexpand::tilde)
            .map(Cow::into_owned)
            .map(PathBuf::from)
            .collect()
    }

    pub fn unset(&self, shell: Shell) -> String {
        let mut output = String::new();
        let vars: HashSet<_> = std::env::vars().map(|(k, _)| k).collect();

        // Get all unique keys from project configs
        let mut all_keys = HashSet::new();

        // Process all project configurations
        for (_, project_value) in self
            .project
            .iter()
            .filter(|(k, _)| k.starts_with("project."))
        {
            match project_value {
                ProjectValue::Config(project_config) => {
                    // Add keys from project vars
                    for key in project_config.vars.keys() {
                        all_keys.insert(key.to_owned());
                    }

                    // Add keys from profiles referenced by projects
                    for profile_name in &project_config.profiles {
                        if let Some(profile) = self.profile.get(profile_name) {
                            for key in profile.keys() {
                                all_keys.insert(key.to_owned());
                            }
                        }
                    }
                }
                ProjectValue::Profiles(profiles) => {
                    // Add keys from profiles referenced directly
                    for profile_name in profiles {
                        if let Some(profile) = self.profile.get(profile_name) {
                            for key in profile.keys() {
                                all_keys.insert(key.to_owned());
                            }
                        }
                    }
                }
            }
        }

        // Only unset variables that are currently set in the environment
        for key in all_keys.iter().dedup().filter(|key| vars.contains(*key)) {
            let res = match shell {
                Shell::Zsh => writeln!(output, "unset {key}"),
                Shell::Fish => writeln!(output, "set -ge {key};"),
            };

            res.unwrap();
        }

        output
    }

    pub fn get_profile(&self, name: &str) -> Option<&HashMap<String, String>> {
        self.profile.get(name)
    }

    pub fn get_project_config(&self, name: &str) -> Option<ProjectConfig> {
        match self.project.get(name) {
            Some(ProjectValue::Config(config)) => Some(config.clone()),
            Some(ProjectValue::Profiles(profiles)) => {
                // Convert array of profiles to a ProjectConfig
                let mut config = ProjectConfig::default();
                config.profiles = profiles.clone();
                Some(config)
            }
            None => None,
        }
    }

    pub fn get_profiles(&self) -> &HashMap<String, HashMap<String, String>> {
        &self.profile
    }

    pub fn get_project_configs(&self) -> HashMap<String, ProjectConfig> {
        let mut result = HashMap::new();

        for (key, value) in self
            .project
            .iter()
            .filter(|(k, _)| k.starts_with("project."))
        {
            let project_name = key.strip_prefix("project.").unwrap();

            match value {
                ProjectValue::Config(config) => {
                    result.insert(project_name.to_string(), config.clone());
                }
                ProjectValue::Profiles(profiles) => {
                    let mut config = ProjectConfig::default();
                    config.profiles = profiles.clone();
                    result.insert(project_name.to_string(), config);
                }
            }
        }

        result
    }
}
