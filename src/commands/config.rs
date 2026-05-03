use clap::Subcommand;
use std::io::{self, Write};
use terrance::config::{
    Config, ConfigManager, ConfigMetadata, GitHubConfig, ITEM_TERRY_GITHUB, OnePasswordClient,
    OpError,
};

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Sync GitHub credentials and sync metadata from 1Password (`op`). Requires `--vault`; reads item **`Terry GitHub`** (fields: `username`, `token`).
    Sync {
        /// 1Password vault name (required; passed to every `op` invocation)
        #[arg(short, long)]
        vault: String,

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
        ConfigCommands::Sync { vault, force } => handle_sync(vault, *force),
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

fn handle_sync(vault: &str, force: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    if manager.config_exists() && !force {
        println!(
            "Configuration already exists at {}. Use --force to re-sync.",
            manager.get_config_path().display()
        );
        return Ok(());
    }

    println!("Syncing configuration from 1Password…");

    let op_client = OnePasswordClient::new(vault);

    op_client.verify_setup().map_err(sync_op_message)?;

    println!("  Fetching GitHub configuration…");
    let github_config = fetch_github_config(&op_client)?;

    let config = Config {
        github: github_config,
        metadata: ConfigMetadata {
            synced_at: chrono::Utc::now().to_rfc3339(),
            vault_name: vault.to_string(),
        },
    };

    manager.save_config(&config)?;

    println!();
    println!("✓ Configuration synced and encrypted successfully!");
    println!("  GitHub: {}", config.github.username);

    Ok(())
}

fn sync_op_message(err: OpError) -> Box<dyn std::error::Error + Send + Sync> {
    let mut msg = err.to_string();

    match &err {
        OpError::NotInstalled | OpError::NotSignedIn | OpError::NotSignedInWithDetail(_) => {
            msg.push_str("\n\nInstall the CLI (`just install-1password-cli`) and enable Integrate with 1Password CLI in the 1Password app (Settings → Developer). Terry runs `op signin --force` automatically when you are not signed in.");
            msg.push_str("\nVault items: create \"");
            msg.push_str(ITEM_TERRY_GITHUB);
            msg.push_str("\" with username and token (concealed).");
        }
        OpError::SignInFailed(_) => {
            msg.push_str("\n\nOpen and unlock the 1Password app, confirm Integrate with 1Password CLI is enabled under Settings → Developer, then try again.");
        }
        _ => {}
    }

    msg.into()
}

fn fetch_github_config(client: &OnePasswordClient) -> Result<GitHubConfig, OpError> {
    let username = client.get_field(ITEM_TERRY_GITHUB, "username")?;
    let token = client.get_field(ITEM_TERRY_GITHUB, "token")?;

    Ok(GitHubConfig { token, username })
}

fn handle_show(reveal: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    println!(
        "Configuration directory: {}",
        manager.config_dir_path().display()
    );
    println!(
        "Configuration file:      {}",
        manager.get_config_path().display()
    );
    println!("Config file exists:      {}", manager.config_exists());

    if !manager.config_exists() {
        return Ok(());
    }

    let config = manager.load_config()?;

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

        let json = serde_json::to_string_pretty(&config)?;
        println!("{}", json);
    } else {
        let redacted = config.redacted();
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
        println!(
            "No configuration file at {}.",
            manager.get_config_path().display()
        );
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
