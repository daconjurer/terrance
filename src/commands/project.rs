use crate::steps::{Step, StepManager};
use clap::Subcommand;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Initialize a new project configuration
    Init {
        /// Name of the project
        #[arg(short, long)]
        name: String,

        /// Path to the project directory
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Git remote URL (will prompt if not provided)
        #[arg(short, long)]
        remote: Option<String>,

        /// Include a planning directory as a git submodule
        #[arg(long)]
        with_planning: bool,
    },
}

pub fn handle_command(command: &ProjectCommands) {
    match command {
        ProjectCommands::Init {
            name,
            path,
            remote,
            with_planning,
        } => {
            if let Err(e) = handle_init(name, path.as_ref(), remote.as_ref(), *with_planning) {
                eprintln!("Error initializing project: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn handle_init(
    name: &str,
    path: Option<&PathBuf>,
    remote: Option<&String>,
    with_planning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = resolve_project_path(path)?;

    println!(
        "Initializing project '{}' at {}",
        name,
        project_path.display()
    );

    let remote_url = get_or_prompt_remote(remote)?;
    let has_remote = remote_url.is_some();

    let planning_submodule_url = if with_planning {
        Some(get_planning_submodule_url()?)
    } else {
        None
    };

    let mut manager = StepManager::new().add_step(
        Step::new("Initialize Git repository", "git init {path}")
            .add_arg("path", project_path.to_str().ok_or("Invalid path")?),
    );

    if let Some(url) = remote_url {
        manager = manager.add_step(
            Step::new("Add remote origin", "git -C {path} remote add origin {url}")
                .add_arg("path", project_path.to_str().ok_or("Invalid path")?)
                .add_arg("url", &url),
        );
    }

    if let Some(submodule_url) = planning_submodule_url {
        manager = manager.add_step(
            Step::new(
                "Add planning repo as git submodule",
                "git -C {path} submodule add {submodule_url} planning",
            )
            .add_arg("path", project_path.to_str().ok_or("Invalid path")?)
            .add_arg("submodule_url", &submodule_url),
        );
    }

    match manager.execute() {
        Ok(_) => {
            println!("\n✓ Project '{}' initialized successfully!", name);
            if has_remote {
                println!("  Git repository created with remote origin");
            } else {
                println!("  Git repository created (no remote)");
            }
            if with_planning {
                println!("  Planning directory added as git submodule");
            }
            Ok(())
        }
        Err(e) => Err(Box::new(e)),
    }
}

fn resolve_project_path(path: Option<&PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match path {
        Some(p) => Ok(p.clone()),
        None => Ok(env::current_dir()?),
    }
}

fn get_or_prompt_remote(
    remote: Option<&String>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(url) = remote {
        return Ok(Some(url.clone()));
    }

    print!("Git remote URL (press Enter to skip): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn get_planning_submodule_url() -> Result<String, Box<dyn std::error::Error>> {
    print!("Planning repository URL: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        Err("Planning repository URL is required when using --with-planning".into())
    } else {
        Ok(trimmed.to_string())
    }
}
