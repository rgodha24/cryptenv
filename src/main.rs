mod config;
mod project;
mod store;

use clap::{Parser, Subcommand, ValueEnum};
use std::process;

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
    /// loads the env for the current directory. you probably shouldn't run this directly
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
    /// lists all the environment variables in the store
    List {
        /// whether to show the value as well
        #[arg(short, long, default_value_t = false)]
        decrypt: bool,
    },
    /// Checks every project and makes sure that the env variables they're referencing are all in
    /// the envcrypt store
    Check,
    /// set up envcrypt for your shell
    Init { shell: Shell },
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
                .filter_map(|project| config.projects().get(&project))
                .next()
                .cloned()
                .unwrap_or_else(Project::get_from_cwd);

            println!("{}", project.to_bash(&store));
        }
        Commands::Init { shell } => {
            println!("{}", shell.init());
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
    }
}
