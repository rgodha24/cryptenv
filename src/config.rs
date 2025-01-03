use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write,
    path::PathBuf,
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{Project, Shell};

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

    pub fn unset(&self, shell: Shell) -> String {
        let mut output = String::new();
        let vars: HashSet<_> = std::env::vars().map(|(k, _)| k).collect();

        for key in self
            .projects
            .values()
            .flat_map(|proj| proj.keys())
            .dedup()
            .filter(|key| vars.contains(key.to_owned()))
        {
            let res = match shell {
                Shell::Zsh => writeln!(output, "unset {key}"),
                Shell::Fish => writeln!(output, "set -ge {key};"),
            };

            res.unwrap();
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
