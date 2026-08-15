use clap::Subcommand;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;

#[derive(Subcommand)]
pub enum AgenticCommands {
    /// Sync agent config: copy source to target, then symlink ai -> target
    Sync {
        /// Path to the version-controlled source file
        #[arg(long)]
        source: String,

        /// Centralized local config location (the canonical copy)
        #[arg(long)]
        target: String,

        /// Where the AI agent expects the file (will be a symlink)
        #[arg(long)]
        ai: String,

        /// Agent name for messaging (claude, opencode, copilot, etc.)
        #[arg(long)]
        agent: Option<String>,

        /// Create target parent directory if it doesn't exist
        #[arg(short = 'p', long)]
        mkdir: bool,

        /// Preview operations without executing them
        #[arg(long)]
        dry_run: bool,

        /// Overwrite existing non-symlink files at --ai location
        #[arg(long)]
        force: bool,
    },
}

pub fn handle_command(command: &AgenticCommands) {
    let result = match command {
        AgenticCommands::Sync {
            source,
            target,
            ai,
            agent,
            mkdir,
            dry_run,
            force,
        } => handle_sync(
            source,
            target,
            ai,
            agent.as_deref(),
            *mkdir,
            *dry_run,
            *force,
        ),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn handle_sync(
    source: &str,
    target: &str,
    ai: &str,
    agent: Option<&str>,
    mkdir: bool,
    dry_run: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let target_path = Path::new(target);
    let ai_path = Path::new(ai);

    // Validate source exists
    if !source_path.exists() {
        return Err(format!("Source file does not exist: {}", source_path.display()).into());
    }

    // Ensure target parent directory exists
    if let Some(parent) = target_path.parent()
        && !parent.exists()
    {
        if mkdir {
            if dry_run {
                println!("[dry-run] Would create directory: {}", parent.display());
            } else {
                fs::create_dir_all(parent)?;
                println!("Created directory: {}", parent.display());
            }
        } else {
            return Err(format!(
                "Target parent directory does not exist: {}. Use -p to create it.",
                parent.display()
            )
            .into());
        }
    }

    // Step 1: Copy source -> target
    if dry_run {
        println!(
            "[dry-run] Would copy {} -> {}",
            source_path.display(),
            target_path.display()
        );
    } else {
        fs::copy(source_path, target_path)?;
        println!(
            "Copied {} -> {}",
            source_path.display(),
            target_path.display()
        );
    }

    // Step 2: Remove existing symlink/file at ai_path
    if ai_path.exists() || ai_path.symlink_metadata().is_ok() {
        let metadata = ai_path.symlink_metadata()?;
        let is_symlink = metadata.file_type().is_symlink();

        if is_symlink {
            if dry_run {
                println!(
                    "[dry-run] Would remove existing symlink: {}",
                    ai_path.display()
                );
            } else {
                fs::remove_file(ai_path)?;
                println!("Removed existing symlink: {}", ai_path.display());
            }
        } else if force {
            if dry_run {
                println!(
                    "[dry-run] Would remove existing file: {}",
                    ai_path.display()
                );
            } else {
                fs::remove_file(ai_path)?;
                println!("Removed existing file: {}", ai_path.display());
            }
        } else {
            return Err(format!(
                "File exists at {} and is not a symlink. Use --force to overwrite.",
                ai_path.display()
            )
            .into());
        }
    }

    // Step 3: Create symlink ai -> target
    if dry_run {
        println!(
            "[dry-run] Would create symlink {} -> {}",
            ai_path.display(),
            target_path.display()
        );
        println!("[dry-run] Sync complete (no changes made)");
    } else {
        unix_fs::symlink(target_path, ai_path)?;
        println!(
            "Created symlink {} -> {}",
            ai_path.display(),
            target_path.display()
        );

        if let Some(agent_name) = agent {
            println!("{} config is now synced", capitalize(agent_name));
        } else {
            println!("Sync complete");
        }
    }

    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
