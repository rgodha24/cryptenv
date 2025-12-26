use std::{collections::HashMap, fmt::Write};

use serde::{Deserialize, Serialize};

use crate::{config::Config, store::Store, Shell};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    vars: HashMap<String, String>,
}

impl Project {
    pub fn to_shell(&self, store: &Store, shell: Shell) -> String {
        let mut output = String::new();

        for (key, value) in &self.vars {
            let variable = store.get_decrypted_or_exit(value);

            let res = match shell {
                Shell::Zsh => writeln!(output, "export {}={}", key, variable.value()),
                Shell::Fish => writeln!(output, "set -gx {} {};", key, variable.value()),
            };
            res.expect("writing to string succeeded");
        }

        output
    }

    /// Get the project in the current directory
    pub fn get_from_cwd() -> Option<Self> {
        let config = Config::read();

        let Some(project_dir) = Self::get_project_dir(&config) else {
            return None;
        };

        Self::from_project_config(&project_dir, &config)
    }

    /// Get the current project or the project with the given name
    pub fn get_current_or_named(name: Option<&str>) -> Option<Self> {
        match name {
            Some(name) => Self::get_by_name(name),
            None => Self::get_from_cwd(),
        }
    }

    pub fn get_project_dir(config: &Config) -> Option<String> {
        let current_dir = std::env::current_dir().unwrap();
        let dirs = config.dirs();

        for dir in dirs.into_iter() {
            if current_dir.starts_with(&dir) {
                let original_len = dir.components().collect::<Vec<_>>().len();
                let parent = current_dir.components().nth(original_len)?;

                return Some(parent.as_os_str().to_str().unwrap().to_string());
            }
        }

        None
    }

    pub fn get_by_name(name: &str) -> Option<Self> {
        let config = Config::read();
        Self::from_project_config(name, &config)
    }

    fn from_project_config(name: &str, config: &Config) -> Option<Self> {
        let project_config = config.get_project_config(name)?;

        let mut project = Project::default();

        // Add vars from the project config
        for (key, value) in &project_config.vars {
            project.vars.insert(key.clone(), value.clone());
        }

        // Add vars from profiles
        for profile_name in &project_config.profiles {
            if let Some(profile) = config.get_profile(profile_name) {
                for (key, value) in profile {
                    // Project-specific vars take precedence over profile vars
                    if !project.vars.contains_key(key) {
                        project.vars.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        Some(project)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.vars.keys().map(String::as_str)
    }

    pub fn variables(&self) -> impl Iterator<Item = &str> {
        self.vars.values().map(String::as_str)
    }

    pub fn into_inner(self) -> HashMap<String, String> {
        self.vars
    }
}
