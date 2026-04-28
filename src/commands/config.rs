use clap::Subcommand;
use std::fs;
use std::io::{self, Write};
use terrance::config::{Config, ConfigManager};

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Sync configuration from 1Password vault
    Sync {
        /// 1Password vault name
        #[arg(short, long)]
        vault: Option<String>,

        /// Force re-sync even if config exists
        #[arg(short, long)]
        force: bool,
    },

    /// Show current configuration (redacted)
    Show {
        /// Show sensitive values (requires confirmation)
        #[arg(long)]
        reveal: bool,
    },

    /// Edit configuration
    Edit,

    /// Clear local encrypted configuration
    Clear {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Get configuration directory path
    Path,
}

pub fn handle_command(command: &ConfigCommands) {
    let result = match command {
        ConfigCommands::Sync { vault, force } => handle_sync(vault.as_deref(), *force),
        ConfigCommands::Show { reveal } => handle_show(*reveal),
        ConfigCommands::Edit => handle_edit(),
        ConfigCommands::Clear { yes } => handle_clear(*yes),
        ConfigCommands::Path => handle_path(),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn handle_sync(
    _vault: Option<&str>,
    _force: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Not yet implemented");
    Ok(())
}

fn handle_show(reveal: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    println!("Configuration directory: {}", manager.config_dir_path().display());
    println!("Configuration file:      {}", manager.get_config_path().display());
    println!("Config file exists:      {}", manager.config_exists());

    if !manager.config_exists() {
        return Ok(());
    }

    let path = manager.get_config_path().clone();
    let raw = fs::read_to_string(&path)?;

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        println!("Config file is empty.");
        return Ok(());
    }

    let parsed: Config = serde_json::from_str(trimmed)?;

    if reveal {
        print!("Reveal sensitive values? Type 'yes' to continue: ");
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let line = line.trim();
        if line != "yes" {
            println!("Aborted.");
            return Ok(());
        }

        let json = serde_json::to_string_pretty(&parsed)?;
        println!("{}", json);
    } else {
        let redacted = parsed.redacted();
        let json = serde_json::to_string_pretty(&redacted)?;
        println!("{}", json);
    }

    Ok(())
}

fn handle_edit() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Not yet implemented");
    Ok(())
}

fn handle_clear(skip_confirm: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    if !manager.config_exists() {
        println!("No configuration file at {}.", manager.get_config_path().display());
        return Ok(());
    }

    if !skip_confirm {
        print!(
            "Remove {}? Type 'yes' to continue: ",
            manager.get_config_path().display()
        );
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if line.trim() != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    manager.remove_config_file()?;
    println!("Configuration cleared.");
    Ok(())
}

fn handle_path() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;
    println!("{}", manager.config_dir_path().display());
    Ok(())
}
