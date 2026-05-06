use clap::Subcommand;
use terrance::github::{CreateRepoOptions, GitHubClient};

#[derive(Subcommand)]
pub enum GitHubCommands {
    /// Create a GitHub repository via `gh` using the **write** PAT from config (`token_write`). Optional `--add-remote` sets `origin` to SSH.
    CreateRepo {
        /// Repository name (slug under your configured GitHub user).
        #[arg(short, long)]
        name: String,

        #[arg(short, long)]
        description: Option<String>,

        /// Run `gh repo create` with `--source . --remote origin`, then rewrite `origin` to SSH.
        #[arg(long)]
        add_remote: bool,
    },

    /// Check whether a repository exists under your GitHub user (`gh repo view` with the **read-only** PAT: Metadata + Contents read).
    Exists { name: String },
}

pub fn handle_command(command: &GitHubCommands) {
    let result = match command {
        GitHubCommands::CreateRepo {
            name,
            description,
            add_remote,
        } => handle_create_repo(name, description.as_deref(), *add_remote),
        GitHubCommands::Exists { name } => handle_exists(name),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn handle_create_repo(
    name: &str,
    description: Option<&str>,
    add_remote: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating GitHub repository '{}'...", name);

    let client = GitHubClient::from_config()?;
    let opts = CreateRepoOptions {
        name: name.to_string(),
        description: description.map(String::from),
        add_remote,
    };

    let repo = client.create_repository(&opts)?;

    println!("\n✓ Repository created successfully!");
    println!("  Name: {}", repo.name);
    println!("  SSH URL: {}", repo.url);
    println!("  Visibility: Private",);

    Ok(())
}

fn handle_exists(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::from_config()?;

    if client.repo_exists(name)? {
        println!("✓ Repository '{}' exists", name);
    } else {
        println!("✗ Repository '{}' does not exist", name);
    }

    Ok(())
}
