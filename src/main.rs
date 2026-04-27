use bootconf::{host, users};
use clap::{Parser, Subcommand};
use log::LevelFilter;
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
        file: path::PathBuf,
    },

    /// Apply a users configuration
    Users {
        /// Specify the file path
        #[arg(long, value_name = "PATH")]
        file: path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    env_logger::Builder::new().filter_level(level).init();

    let command = if let Some(command) = cli.command {
        command
    } else {
        return;
    };

    match command {
        Commands::Host { file } => {
            if let Err(e) = host::apply_host_config(&file, None) {
                eprintln!("Error applying host config: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Users { file } => {
            if let Err(e) = users::apply_users_config(&file) {
                eprintln!("Error applying users config: {}", e);
                std::process::exit(1);
            }
        }
    }
}
