use std::{collections::HashMap, fmt::Write, process};

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
            let variable = store.get(value).map(|v| v.decrypt()).unwrap_or_else(|| {
                eprintln!("cryptenv: variable {} not found", value);
                process::exit(1);
            });

            let res = match shell {
                Shell::Zsh => writeln!(output, "export {}={}", key, variable.value()),
                Shell::Fish => writeln!(output, "set -gx {} {};", key, variable.value()),
            };
            res.expect("writing to string succeeded");
        }

        output
    }

    /// get the project in the current directory
    pub fn get_from_cwd() -> Option<Self> {
        let mut config = Config::read();

        let Some(project_dir) = Self::get_project_dir(&config) else {
            return Default::default();
        };

        // it's fine to remove the "project" from the config because config is dropped at the end
        // of this function anyways
        config.projects_mut().remove(&project_dir)
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

        config.projects().get(name).cloned()
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
