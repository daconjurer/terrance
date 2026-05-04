use crate::steps::{Step, StepManager};
use clap::Subcommand;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use terrance::github::GitHubClient;

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

        /// Git remote URL (will prompt if not provided). Cannot be combined with `--github-repo`.
        #[arg(short, long)]
        remote: Option<String>,

        /// GitHub repository slug under your synced GitHub username (`terry config sync`). Sets `origin` to SSH (`git@github.com:user/repo.git`). Cannot be combined with `--remote`.
        #[arg(long)]
        github_repo: Option<String>,

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
            github_repo,
            with_planning,
        } => {
            if let Err(e) = handle_init(
                name,
                path.as_ref(),
                remote.as_ref(),
                github_repo.as_ref(),
                *with_planning,
            ) {
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
    github_repo: Option<&String>,
    with_planning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = resolve_project_path(path)?;

    println!(
        "Initializing project '{}' at {}",
        name,
        project_path.display()
    );

    let remote_url = resolve_remote(remote, github_repo)?;
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

    if let Some(url) = remote_url.as_ref() {
        manager = manager.add_step(
            Step::new("Add remote origin", "git -C {path} remote add origin {url}")
                .add_arg("path", project_path.to_str().ok_or("Invalid path")?)
                .add_arg("url", url),
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

fn resolve_remote(
    remote: Option<&String>,
    github_repo: Option<&String>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match (remote, github_repo) {
        (Some(_), Some(_)) => Err(
            "Cannot use both --remote and --github-repo; choose an explicit URL or a GitHub slug."
                .into(),
        ),
        (_, Some(slug)) => {
            let trimmed = slug.trim();
            if trimmed.is_empty() {
                return Err("--github-repo cannot be empty".into());
            }
            let client = GitHubClient::from_config()?;
            Ok(Some(client.origin_ssh_url(trimmed)))
        }
        (Some(url), None) => Ok(Some(url.clone())),
        (None, None) => get_or_prompt_remote(None),
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
