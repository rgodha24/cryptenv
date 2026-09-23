mod config;
mod project;
mod store;

use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

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
    ///
    /// If no value is given, it is read from stdin (piped input, or a hidden
    /// prompt when run interactively) so it never ends up in shell history.
    Add {
        /// The name of the environment variable (automatically uppercased)
        name: String,
        /// The value of the environment variable (will be encrypted). Use `-` or omit to read from stdin
        #[arg(conflicts_with = "from_file")]
        value: Option<String>,
        /// Read the value from a file
        #[arg(short, long, value_name = "PATH")]
        from_file: Option<PathBuf>,
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
    /// Run a command with the environment variables from a profile
    Run {
        /// The name of the profile to use
        profile: String,
        /// The command to run
        command: Vec<String>,
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
            from_file,
            overwrite,
        } => {
            let mut store = Store::read();
            let name = name.to_uppercase();

            // check before reading the value so we don't prompt for a secret we'd throw away
            let is_used = store.get(&name).is_some();
            if is_used && !overwrite {
                eprintln!(
                    "Value for {} already exists. Use --overwrite to replace it",
                    name
                );
                process::exit(1);
            }

            let value = match (value, from_file) {
                (Some(v), _) if v != "-" => v,
                (_, Some(path)) => read_value_from_file(&path),
                _ => read_value_from_stdin(&name),
            };

            if value.is_empty() {
                eprintln!("Refusing to add an empty value for {}", name);
                process::exit(1);
            }

            if is_used {
                eprintln!("Overwriting value for {}", name);
            }
            store.add(name.clone(), &value);
            store.save_to_disk();

            if !is_used {
                println!("Added {} to store", name);
            }
        }
        Commands::Get { name } => {
            let store = Store::read();
            let name = name.to_uppercase();

            let variable = store.get_decrypted_or_exit(&name);

            println!("{}", variable.value());
        }
        Commands::List { decrypt } => {
            let store = Store::read();

            for (name, _) in store.iter() {
                if decrypt {
                    let variable = store.get_decrypted_or_exit(name);
                    println!("{}={}", name, variable.value());
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
                        let variable = store.get_decrypted_or_exit(&v);
                        println!("{}={}", k, variable.value());
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
        Commands::Run { profile, command } => {
            if command.is_empty() {
                eprintln!("No command specified");
                process::exit(1);
            }

            let config = Config::read();
            let store = Store::read();

            let profile_vars = match config.get_profile(&profile) {
                Some(profile) => profile,
                None => {
                    eprintln!("Profile '{}' not found", profile);
                    process::exit(1);
                }
            };

            if profile_vars.is_empty() {
                eprintln!("Profile '{}' has no variables", profile);
                process::exit(1);
            }

            let mut cmd = Command::new(&command[0]);

            // Add the remaining arguments
            if command.len() > 1 {
                cmd.args(&command[1..]);
            }

            // Add environment variables from the profile
            for (key, value) in profile_vars {
                let variable = store.get_decrypted_or_exit(value);

                cmd.env(key, variable.value());
            }

            // Execute the command
            match cmd.status() {
                Ok(status) => {
                    if !status.success() {
                        process::exit(status.code().unwrap_or(1));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to execute command: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}

/// Strips a single trailing newline, since files and piped input almost always end with one.
fn strip_trailing_newline(mut s: String) -> String {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

fn read_value_from_file(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => strip_trailing_newline(s),
        Err(e) => {
            eprintln!("Failed to read {}: {}", path.display(), e);
            process::exit(1);
        }
    }
}

fn read_value_from_stdin(name: &str) -> String {
    if io::stdin().is_terminal() {
        match rpassword::prompt_password(format!("Value for {}: ", name)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to read value: {}", e);
                process::exit(1);
            }
        }
    } else {
        let mut s = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut s) {
            eprintln!("Failed to read value from stdin: {}", e);
            process::exit(1);
        }
        strip_trailing_newline(s)
    }
}
