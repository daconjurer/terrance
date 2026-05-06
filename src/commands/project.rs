use crate::steps::{Step, StepManager};
use clap::Subcommand;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use terrance::github::{CreateRepoOptions, GitHubClient};

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

        /// GitHub repository slug under your synced GitHub username (`terry config sync`).
        /// Derives `origin` as SSH (`git@github.com:user/repo.git`), then creates the private repo
        /// on GitHub via `gh` after local `git init` (requires synced `token_write`). Omit for a
        /// local repository with no `origin`.
        #[arg(long)]
        repo_slug: Option<String>,

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
            repo_slug,
            with_planning,
        } => {
            if let Err(e) = handle_init(name, path.as_ref(), repo_slug.as_ref(), *with_planning) {
                eprintln!("Error initializing project: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn handle_init(
    name: &str,
    path: Option<&PathBuf>,
    repo_slug: Option<&String>,
    with_planning: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = resolve_project_path(path)?;

    println!(
        "Initializing project '{}' at {}",
        name,
        project_path.display()
    );

    let (remote_url, github_create) = if let Some(slug) = repo_slug {
        let trimmed = slug.trim();
        if trimmed.is_empty() {
            return Err("--repo-slug cannot be empty".into());
        }
        let client = GitHubClient::from_config()?;
        let url = client.origin_ssh_url(trimmed);
        (Some(url), Some((client, trimmed.to_string())))
    } else {
        (None, None)
    };
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
            if let Some((client, slug)) = github_create {
                println!("Creating GitHub repository '{}' via gh...", slug);
                client.create_repository(&CreateRepoOptions {
                    name: slug,
                    description: None,
                    add_remote: false,
                })?;
                println!("  Remote repository created on GitHub (private)");
            }

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
