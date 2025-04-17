mod config;
mod project;
mod store;

use clap::{Parser, Subcommand, ValueEnum};
use std::process::{self};

pub use config::{Config, ProjectConfig};
pub use project::Project;
pub use store::Store;

#[derive(Parser)]
#[command(version, about = "A simple encrypted environment variable manager")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if all project variables are defined in the store
    Check,
    /// Set up cryptenv for your shell
    Init {
        /// The shell to initialize
        shell: Shell,
    },
    /// Add an environment variable to the store
    Add {
        /// The name of the environment variable (automatically uppercased)
        name: String,
        /// The value of the environment variable (will be encrypted)
        value: String,
        /// Overwrite the value if it already exists
        #[arg(short, long, default_value_t = false)]
        overwrite: bool,
    },
    /// Get an environment variable from the store
    Get {
        /// The name of the environment variable (automatically uppercased)
        name: String,
    },
    /// List environment variables in the store
    List {
        /// Show decrypted values
        #[arg(short, long, default_value_t = false)]
        decrypt: bool,
    },
    /// Load environment for the current directory
    Load {
        /// The shell to generate script for
        shell: Shell,
    },
    /// Get the name of the current project
    Project,
    /// List variables in a project
    Variables {
        /// The project name (defaults to current directory if not specified)
        project: Option<String>,
    },
    /// Export the environment variables of a project in KEY=VALUE format
    Export {
        /// The project name (defaults to current directory if not specified)
        project: Option<String>,
    },
    /// List all available profiles
    Profiles,
    /// Show variables in a specific profile
    ProfileVars {
        /// The name of the profile to show variables for
        name: String,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Shell {
    Zsh,
    Fish,
}

impl Shell {
    fn init(&self) -> &'static str {
        match self {
            Shell::Zsh => include_str!("../shells/init.zsh"),
            Shell::Fish => include_str!("../shells/init.fish"),
        }
    }
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Check => {
            let config = Config::read();
            #[cfg(debug_assertions)]
            println!("Config: {:#?}", config);
            let store = Store::read();
            let mut found_error = false;

            // Check variables from project configs
            for (name, project_config) in config.get_project_configs() {
                // Check direct vars
                for var_name in project_config.vars.values() {
                    if store.get(var_name).is_none() {
                        found_error = true;
                        println!(
                            "cryptenv: variable {} defined in project {} not found in store",
                            var_name, name
                        );
                    }
                }

                // Check vars from referenced profiles
                for profile_name in &project_config.profiles {
                    if let Some(profile) = config.get_profile(profile_name) {
                        for var_name in profile.values() {
                            if store.get(var_name).is_none() {
                                found_error = true;
                                println!(
                                    "cryptenv: variable {} defined in profile {} (referenced by project {}) not found in store",
                                    var_name, profile_name, name
                                );
                            }
                        }
                    } else {
                        println!(
                            "cryptenv: warning - profile {} referenced by project {} not found",
                            profile_name, name
                        );
                    }
                }
            }

            if found_error {
                process::exit(1);
            } else {
                println!("All variables found in store!");
            }
        }
        Commands::Init { shell } => {
            println!("{}", shell.init());
        }
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
                    store.add(name.clone(), &value);
                    println!("Added {} to store", name);
                }
                (true, true) => {
                    eprintln!("Overwriting value for {}", name);
                    store.add(name.clone(), &value);
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
        Commands::List { decrypt } => {
            let store = Store::read();

            for (name, variable) in store.iter() {
                if decrypt {
                    println!("{}={}", name, variable.decrypt().value());
                } else {
                    println!("{}", name);
                }
            }
        }
        Commands::Load { shell } => {
            let config = Config::read();
            let store = Store::read();
            let project = Project::get_from_cwd().unwrap_or_default();

            println!("{}", config.unset(shell));
            println!("{}", project.to_shell(&store, shell));
        }
        Commands::Project => {
            let dir = Project::get_project_dir(&Config::read());

            match dir {
                Some(d) => {
                    println!("{d}");
                }
                _ => {
                    eprintln!("Not in a project directory");
                    process::exit(1);
                }
            }
        }
        Commands::Variables { project } => {
            let p = Project::get_current_or_named(project.as_deref());

            match p {
                Some(project) => {
                    for v in project.variables() {
                        println!("{}", v);
                    }
                }
                None => {
                    match project {
                        None => eprintln!("Not in a project directory"),
                        Some(project) => eprintln!("Project {} not found", project),
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Export { project } => {
            let p = Project::get_current_or_named(project.as_deref());
            let store = Store::read();

            match p {
                Some(project) => {
                    for (k, v) in project.into_inner() {
                        println!("{}={}", k, store.get(&v).unwrap().decrypt().value());
                    }
                }
                None => {
                    match project {
                        None => eprintln!("Not in a project directory"),
                        Some(project) => eprintln!("Project {} not found", project),
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Profiles => {
            let config = Config::read();

            if config.get_profiles().is_empty() {
                println!("No profiles defined");
                return;
            }

            println!("Available profiles:");
            for profile_name in config.get_profiles().keys() {
                println!("  {}", profile_name);
            }
        }
        Commands::ProfileVars { name } => {
            let config = Config::read();

            match config.get_profile(&name) {
                Some(profile) => {
                    if profile.is_empty() {
                        println!("Profile '{}' has no variables", name);
                        return;
                    }

                    println!("Variables in profile '{}':", name);
                    for (key, value) in profile {
                        println!("  {}={}", key, value);
                    }
                }
                None => {
                    eprintln!("Profile '{}' not found", name);
                    process::exit(1);
                }
            }
        }
    }
}
