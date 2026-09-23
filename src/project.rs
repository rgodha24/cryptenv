use std::{
    collections::HashMap,
    fmt::Write,
    path::{Path, PathBuf},
};

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

        Self::project_dir_for(&current_dir, config).or_else(|| {
            // Git worktrees usually live outside the configured dirs, so fall back
            // to the main checkout the worktree belongs to.
            let main_checkout = main_worktree_of(&current_dir)?;
            Self::project_dir_for(&main_checkout, config)
        })
    }

    fn project_dir_for(path: &Path, config: &Config) -> Option<String> {
        for dir in config.dirs() {
            if path.starts_with(&dir) {
                let original_len = dir.components().count();
                let parent = path.components().nth(original_len)?;

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

/// If `path` is inside a linked git worktree, return the root of the main checkout.
///
/// A linked worktree has a `.git` *file* containing `gitdir: <main>/.git/worktrees/<name>`,
/// and that directory has a `commondir` file pointing (usually relatively) at `<main>/.git`.
fn main_worktree_of(path: &Path) -> Option<PathBuf> {
    let dot_git = path
        .ancestors()
        .map(|p| p.join(".git"))
        .find(|p| p.exists())?;

    // A `.git` directory means this is already a main checkout (or a plain repo).
    if !dot_git.is_file() {
        return None;
    }

    let contents = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = PathBuf::from(contents.trim().strip_prefix("gitdir:")?.trim());
    let gitdir = dot_git.parent()?.join(gitdir);

    let commondir = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
    let common = gitdir.join(commondir.trim()).canonicalize().ok()?;

    // Bare repos have no main checkout to follow.
    if common.file_name()? != ".git" {
        return None;
    }

    common.parent().map(Path::to_path_buf)
}
