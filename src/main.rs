use bootconf::{host, users};
use clap::{Parser, Subcommand};
use std::path;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Set verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply a host configuration
    Host {
        /// Specify the file path
        #[arg(long, value_name = "PATH")]
        dir: Option<path::PathBuf>,
    },

    /// Apply a users configuration
    Users {
        /// Specify the file path
        #[arg(long, value_name = "PATH")]
        dir: Option<path::PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        match command {
            Commands::Host { dir } => host::apply_host_config(&dir),
            Commands::Users { dir } => users::apply_users_config(&dir),
        }
    }
}
