//! `terry project` CLI: initialize a local git repo, optional GitHub remote and submodule, and
//! optional GitHub repo creation via a subprocess that runs the same `terry` binary
//! (`github create-repo`).

use crate::steps::{Step, StepManager};
use clap::Subcommand;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use terrance::github::GitHubClient;

/// Subcommands under `terry project`.
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

/// Dispatches a [`ProjectCommands`] variant; on failure prints to stderr and exits the process.
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

/// `argv` entries after the executable for `terry github create-repo` (private repo, no `--add-remote`).
fn github_create_repo_cli_argv(slug: &str) -> Vec<String> {
    vec![
        "github".to_string(),
        "create-repo".to_string(),
        "--name".to_string(),
        slug.to_string(),
    ]
}

/// Full single-string command for [`Step`] (ASCII whitespace–split into argv).
fn github_create_repo_step_command(exe: &str, slug: &str) -> String {
    format!("{} {}", exe, github_create_repo_cli_argv(slug).join(" "))
}

/// Builds the [`Step`] that spawns `exe` with `github create-repo --name` for `slug` (see [`github_create_repo_cli_argv`]).
fn github_create_repo_step(exe: &str, slug: &str) -> Step {
    Step::new(
        "Create GitHub repository",
        &github_create_repo_step_command(exe, slug),
    )
}

/// Runs `project init`: resolves the directory, optionally prompts for a planning submodule URL,
/// then executes [`StepManager`] steps in order—`git init`, optional `remote add origin` (SSH URL
/// from config when `--repo-slug` is set), optional submodule, and optional final `terry github
/// create-repo` subprocess when `--repo-slug` is set.
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

    let (remote_url, create_repo_slug) = if let Some(slug) = repo_slug {
        let trimmed = slug.trim();
        if trimmed.is_empty() {
            return Err("--repo-slug cannot be empty".into());
        }
        let client = GitHubClient::from_config()?;
        let url = client.origin_ssh_url(trimmed);
        (Some(url), Some(trimmed.to_string()))
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

    if let Some(slug) = create_repo_slug.as_ref() {
        let exe = env::current_exe()
            .map_err(|e| format!("failed to resolve terry executable path: {e}"))?
            .to_str()
            .ok_or("terry executable path is not valid UTF-8")?
            .to_string();
        manager = manager.add_step(github_create_repo_step(&exe, slug));
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

/// Project directory: explicit `--path` or the current working directory.
fn resolve_project_path(path: Option<&PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match path {
        Some(p) => Ok(p.clone()),
        None => Ok(env::current_dir()?),
    }
}

/// Prompts on stdout for the planning repository URL (`--with-planning`); reads a line from stdin. Empty input errors.
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

/// Tests for GitHub create-repo argv and step construction helpers.
#[cfg(test)]
mod tests {
    use super::*;

    /// [`github_create_repo_cli_argv`] matches the `github create-repo --name <slug>` tail.
    #[test]
    fn github_create_repo_cli_argv_order_and_flags() {
        assert_eq!(
            github_create_repo_cli_argv("my-app"),
            vec![
                "github".to_string(),
                "create-repo".to_string(),
                "--name".to_string(),
                "my-app".to_string(),
            ]
        );
    }

    /// [`github_create_repo_step_command`] concatenates the executable path and argv tail with spaces.
    #[test]
    fn github_create_repo_step_command_joins_exe_and_argv() {
        assert_eq!(
            github_create_repo_step_command("/usr/local/bin/terry", "slug-1"),
            "/usr/local/bin/terry github create-repo --name slug-1"
        );
    }

    /// [`github_create_repo_step`] uses the step label shown in [`StepManager`] error messages.
    #[test]
    fn github_create_repo_step_label() {
        let step = github_create_repo_step("/tmp/terry", "z");
        assert_eq!(step.name(), "Create GitHub repository");
    }
}
