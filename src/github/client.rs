use crate::config::ConfigManager;
use std::path::Path;
use std::process::Command;

fn box_sync_err(err: Box<dyn std::error::Error + Send + Sync>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(err.to_string()))
}

const WRITE_TOKEN_HELP: &str = "Add a concealed field labeled `token_write` to your 1Password item (fine-grained PAT: Metadata read-only, Administration read/write), then run `terry config sync --vault <vault> --force`.";

/// Trim whitespace and leading/trailing slashes from a GitHub username or repo slug.
pub fn normalize_repo_slug(value: &str) -> String {
    value.trim().trim_matches('/').to_string()
}

/// Canonical SSH clone URL for `github.com`, using **username + repo slug only** (no HTTPS).
///
/// Format: `git@github.com:<username>/<repo>.git`
pub fn origin_ssh_url(github_username: &str, repo_slug: &str) -> String {
    let owner = normalize_repo_slug(github_username);
    let repo = normalize_repo_slug(repo_slug);
    format!("git@github.com:{owner}/{repo}.git")
}

/// GitHub automation via **`gh`**, using two fine-grained PATs when synced from 1Password (principle of least privilege).
///
/// - **`token`** — read-only (**Metadata** + **Contents**). Used for [`GitHubClient::repo_exists`] (`gh repo view`).
/// - **`token_write`** — (**Metadata** read-only + **Administration** read/write). Used for [`GitHubClient::create_repository`] (`gh repo create`).
pub struct GitHubClient {
    /// Read-only PAT for repository reads (`gh repo view`, etc.).
    token: String,
    /// PAT allowed to create repositories. `None` if the encrypted config predates `token_write` or sync omitted it.
    token_write: Option<String>,
    username: String,
}

#[derive(Debug, Clone)]
pub struct CreateRepoOptions {
    pub name: String,
    pub description: Option<String>,
    pub add_remote: bool,
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    /// SSH clone URL (`git@github.com:user/repo.git`).
    pub url: String,
}

impl GitHubClient {
    /// Loads GitHub username and both PATs from Terry config (`terry config sync` from **1Password**).
    pub fn from_config() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = ConfigManager::new().map_err(box_sync_err)?;
        let config = manager.load_config().map_err(box_sync_err)?;
        Ok(Self {
            token: config.github.token,
            token_write: config.github.token_write,
            username: config.github.username,
        })
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// [`origin_ssh_url`] using the synced GitHub username from config.
    pub fn origin_ssh_url(&self, repo_slug: &str) -> String {
        origin_ssh_url(&self.username, repo_slug)
    }

    /// Create the repo via **`gh`** using the **write** PAT (`GH_TOKEN` must allow `repo create`). When **`add_remote`** is true,
    /// prefers **`origin`** as **SSH** (`git@github.com:…`), rewriting whatever URL `gh` added by default.
    pub fn create_repository(
        &self,
        options: &CreateRepoOptions,
    ) -> Result<Repository, Box<dyn std::error::Error>> {
        let write_token = self
            .token_write
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| -> Box<dyn std::error::Error> {
                format!("GitHub write token missing. {WRITE_TOKEN_HELP}").into()
            })?;

        let slug = normalize_repo_slug(&options.name);
        if slug.is_empty() {
            return Err("repository name cannot be empty".into());
        }

        let qualified = format!("{}/{}", normalize_repo_slug(&self.username), slug);

        let mut cmd = Command::new("gh");
        cmd.arg("repo").arg("create").arg(&qualified);
        cmd.arg("--private");

        if let Some(desc) = &options.description {
            cmd.arg("--description").arg(desc);
        }

        if options.add_remote {
            cmd.arg("--source").arg(".").arg("--remote").arg("origin");
        }

        cmd.env("GH_TOKEN", write_token);

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to create repository: {}", stderr).into());
        }

        let ssh_url = self.origin_ssh_url(&slug);

        if options.add_remote {
            let cwd = std::env::current_dir()?;
            ensure_origin_ssh_remote(&cwd, &ssh_url)?;
        }

        Ok(Repository {
            name: slug,
            url: ssh_url,
        })
    }

    /// Returns whether `gh repo view owner/repo` succeeds, using the **read-only** PAT (Metadata + Contents read).
    pub fn repo_exists(&self, name: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let slug = normalize_repo_slug(name);
        if slug.is_empty() {
            return Ok(false);
        }

        let qualified = format!("{}/{}", normalize_repo_slug(&self.username), slug);

        let output = Command::new("gh")
            .arg("repo")
            .arg("view")
            .arg(&qualified)
            .env("GH_TOKEN", &self.token)
            .output()?;

        Ok(output.status.success())
    }
}

fn ensure_origin_ssh_remote(
    repo_root: &Path,
    ssh_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let set = Command::new("git")
        .current_dir(repo_root)
        .args(["remote", "set-url", "origin", ssh_url])
        .output()?;

    if set.status.success() {
        return Ok(());
    }

    let add = Command::new("git")
        .current_dir(repo_root)
        .args(["remote", "add", "origin", ssh_url])
        .output()?;

    if add.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&add.stderr);
    Err(format!("could not point remote origin at {}: {}", ssh_url, stderr).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_repo_slug_trims_whitespace_and_outer_slashes() {
        assert_eq!(normalize_repo_slug("  foo-bar  "), "foo-bar");
        assert_eq!(normalize_repo_slug("/baz/"), "baz");
    }

    #[test]
    fn origin_ssh_url_formats_github_clone_url() {
        assert_eq!(
            origin_ssh_url("octocat", "hello-world"),
            "git@github.com:octocat/hello-world.git"
        );
    }
}
