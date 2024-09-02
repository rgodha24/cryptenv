use std::{borrow::Cow, collections::HashMap, fmt::Write, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    projects: HashMap<String, Project>,
    dirs: Vec<String>,
}

impl Config {
    pub fn read() -> Self {
        let config_path = shellexpand::tilde("~/.config/cryptenv.toml");

        let config = std::fs::read_to_string(&*config_path).expect("Could not read config file");

        toml::from_str(&config).expect("Could not parse config file")
    }

    pub fn dirs(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .map(shellexpand::tilde)
            .map(Cow::into_owned)
            .map(PathBuf::from)
            .collect()
    }

    pub fn unset_all_bash(&self) -> String {
        let mut output = String::new();

        for project in self.projects.values() {
            for key in project.keys() {
                writeln!(output, "unset {}", key).unwrap();
            }
        }

        output
    }

    pub fn projects(&self) -> &HashMap<String, Project> {
        &self.projects
    }

    pub fn projects_mut(&mut self) -> &mut HashMap<String, Project> {
        &mut self.projects
    }
}
