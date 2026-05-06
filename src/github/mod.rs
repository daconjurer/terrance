pub mod client;

pub use client::{
    CreateRepoOptions, GitHubClient, Repository, normalize_repo_slug, origin_ssh_url,
};
