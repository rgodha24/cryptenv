mod config;
mod project;
mod store;

use clap::{Parser, Subcommand, ValueEnum};
use std::process::{self};

pub use config::Config;
pub use project::Project;
pub use store::Store;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Checks every project and makes sure that the env variables they're referencing are all in
    /// the cryptenv store
    Check,
    /// set up cryptenv for your shell
    Init { shell: Shell },

    /// edit the env variables in the cryptenv store
    Env {
        #[command(subcommand)]
        subcommand: EnvSubcommand,
    },
    /// manage projects in the cryptenv store
    Project {
        #[command(subcommand)]
        subcommand: ProjectSubcommand,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Shell {
    Zsh,
    Fish,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ProjectSubcommand {
    /// loads the env for the current directory. you probably shouldn't run this directly
    ///
    /// returns the shell script to set the environment variables for the current project
    Load { shell: Shell },
    /// gets the name of the project in CWD
    /// exits with status code 1 if we're not in a project
    Name,
    /// lists all the names of the environment variables in the current project
    /// you can either pass in the project, or use the project in CWD
    List { project: String },
    /// returns the environment variables of the current project in the
    /// `KEY=VALUE` format used by .env files
    Export { project: String },
}

#[derive(Subcommand, Clone, Debug)]
pub enum EnvSubcommand {
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
    /// lists all the environment variables in the store
    List {
        /// whether to show the value as well
        #[arg(short, long, default_value_t = false)]
        decrypt: bool,
    },
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
            let store = Store::read();
            let mut found_error = false;

            for (name, project) in config.projects().iter() {
                for variable in project.variables() {
                    if store.get(variable).is_none() {
                        found_error = true;

                        println!(
                            "cryptenv: variable {} defined in project {} not found in store",
                            variable, name
                        );
                    }
                }
            }

            if found_error {
                process::exit(1);
            } else {
                println!("the config is correct!");
            }
        }
        Commands::Init { shell } => {
            println!("{}", shell.init());
        }
        Commands::Env { subcommand } => match subcommand {
            EnvSubcommand::Add {
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
            EnvSubcommand::Get { name } => {
                let store = Store::read();
                let name = name.to_uppercase();

                let variable = store.get(&name).map(|v| v.decrypt()).unwrap_or_else(|| {
                    eprintln!("cryptenv: variable {} not found", name);
                    process::exit(1);
                });

                println!("{}", variable.value());
            }
            EnvSubcommand::List { decrypt } => {
                let store = Store::read();

                for (name, variable) in store.iter() {
                    if decrypt {
                        println!("{}={}", name, variable.decrypt().value());
                    } else {
                        println!("{}", name);
                    }
                }
            }
        },

        Commands::Project { subcommand } => match subcommand {
            ProjectSubcommand::Load { shell } => {
                let config = Config::read();
                let store = Store::read();
                let project = Project::get_from_cwd().unwrap_or_default();

                println!("{}", config.unset(shell));
                println!("{}", project.to_shell(&store, shell));
            }
            ProjectSubcommand::Name => {
                let dir = Project::get_project_dir(&Config::read());

                match dir {
                    Some(d) => {
                        println!("{d}");
                    }
                    _ => {
                        process::exit(1);
                    }
                }
            }
            ProjectSubcommand::List { project } => {
                let p = Project::get_by_name(&project);
                match p {
                    Some(project) => {
                        for v in project.variables() {
                            println!("{}", v);
                        }
                    }
                    None => {
                        eprintln!("project {project} was not find");
                    }
                }
            }
            ProjectSubcommand::Export { project } => {
                let p = Project::get_by_name(&project);
                let store = Store::read();

                match p {
                    Some(project) => {
                        for (k, v) in project.into_inner() {
                            println!("{}={}", k, store.get(&v).unwrap().decrypt().value());
                        }
                    }
                    None => {
                        eprintln!("project {project} was not find");
                    }
                }
            }
        },
    }
}
