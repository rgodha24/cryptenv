mod store;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap, fmt::Write, path::PathBuf, process};
use store::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    projects: HashMap<String, Project>,
    dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Project {
    vars: HashMap<String, String>,
}

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// loads the env for the current directory
    ///
    /// returns the zsh script to set the environment variables for the current project
    Load {
        #[arg(short, long)]
        project: Option<String>,
    },

    /// add an environment variable to the store
    Add {
        /// the name of the environment variable. automatically uppercased
        name: String,
        /// the value of the environment variable.
        /// stored in a JSON file with encrypted VALUES ONLY at dirs::data_dir()/cryptenv/store.json
        value: String,

        #[arg(short, long, default_value_t = false)]
        /// overwrite the value if it already exists
        /// WARNING: this will irrevcably delete the old value
        /// default: false
        overwrite: bool,
    },

    /// read an environment variable from the store
    Get {
        /// the name of the environment variable. automatically uppercased
        name: String,
    },

    Init {
        shell: Shell,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Shell {
    Zsh,
}

impl Shell {
    fn init(&self) -> &'static str {
        match self {
            Shell::Zsh => include_str!("../shells/init.zsh"),
        }
    }
}

impl Config {
    fn read() -> Self {
        let config_path = shellexpand::tilde("~/.config/cryptenv.toml");

        let config = std::fs::read_to_string(&*config_path).expect("Could not read config file");

        toml::from_str(&config).expect("Could not parse config file")
    }

    fn dirs(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .map(shellexpand::tilde)
            .map(Cow::into_owned)
            .map(PathBuf::from)
            .collect()
    }

    fn unset_all_bash(&self) -> String {
        let mut output = String::new();

        for project in self.projects.values() {
            for key in project.vars.keys() {
                writeln!(output, "unset {}", key).unwrap();
            }
        }

        output
    }
}

impl Project {
    fn to_bash(&self, store: &Store) -> String {
        let mut output = String::new();

        for (key, value) in &self.vars {
            let variable = store.get(&value).map(|v| v.decrypt()).unwrap_or_else(|| {
                eprintln!("cryptenv: variable {} not found", value);
                process::exit(1);
            });

            writeln!(output, "export {}={}", key, variable.value()).unwrap();
        }

        output
    }

    /// get the project in the current directory
    /// returns Project::default() if no project is found
    fn get_from_dir() -> Self {
        let mut config = Config::read();

        let Some(project_dir) = Self::get_project_dir(&config) else {
            eprintln!("cryptenv: current dir is not a project directory");
            return Default::default();
        };

        // it's fine to remove the "project" from the config because config is dropped at the end
        // of this function anyways
        match config.projects.remove(&project_dir) {
            Some(project) => project,
            None => {
                eprintln!("cryptenv: current project is not in the config file");
                Default::default()
            }
        }
    }

    fn get_project_dir(config: &Config) -> Option<String> {
        let current_dir = std::env::current_dir().unwrap();
        let dirs = config.dirs();

        for dir in dirs.into_iter() {
            if current_dir.starts_with(&dir) {
                let original_len = dir.components().collect::<Vec<_>>().len();
                let parent = current_dir.components().skip(original_len).next()?;

                return Some(parent.as_os_str().to_str().unwrap().to_string());
            }
        }

        None
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Add {
            name,
            value,
            overwrite,
        } => {
            let mut store = Store::read();
            let name = name.to_uppercase();

            let is_used = store.get(&name).is_some();
            match (is_used, overwrite) {
                (false, _) => {
                    store.add(name, &value);
                }
                (true, true) => {
                    eprintln!("Overwriting value for {}", name);
                    store.add(name, &value);
                }
                (true, false) => {
                    eprintln!(
                        "Value for {} already exists. Use --overwrite to replace it",
                        name
                    );
                }
            }

            store.save_to_disk();
        }
        Commands::Get { name } => {
            let store = Store::read();
            let name = name.to_uppercase();

            let variable = store.get(&name).map(|v| v.decrypt()).unwrap_or_else(|| {
                eprintln!("cryptenv: variable {} not found", name);
                process::exit(1);
            });

            println!("{}", variable.value());
        }
        Commands::Load { project } => {
            let config = Config::read();
            let store = Store::read();

            println!("{}", config.unset_all_bash());

            let project = project
                .into_iter()
                .filter_map(|project| config.projects.get(&project))
                .cloned()
                .next()
                .unwrap_or_else(Project::get_from_dir);

            println!("{}", project.to_bash(&store));
        }
        Commands::Init { shell } => {
            println!("{}", shell.init());
        }
    }
}
