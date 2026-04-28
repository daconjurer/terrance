mod commands;
mod error;
mod steps;

use clap::{Parser, Subcommand};
use commands::{config, project};

#[derive(Parser)]
#[command(name = "terry")]
#[command(author, version, about = "Development environment configuration manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage project configurations
    Project {
        #[command(subcommand)]
        command: project::ProjectCommands,
    },

    /// Manage Terry configuration
    Config {
        #[command(subcommand)]
        command: config::ConfigCommands,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Project { command } => {
            project::handle_command(command);
        }
        Commands::Config { command } => {
            config::handle_command(command);
        }
    }
}
