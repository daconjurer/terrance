use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Git configuration
    pub git: GitConfig,

    /// GitHub configuration
    pub github: Option<GitHubConfig>,

    /// Metadata
    pub metadata: ConfigMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub user_name: String,
    pub user_email: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub synced_at: String,
    pub vault_name: Option<String>,
}

impl Config {
    pub fn redacted(&self) -> Self {
        let mut config = self.clone();
        if let Some(github) = &mut config.github {
            github.token = Self::redact_token(&github.token);
        }
        config
    }

    fn redact_token(token: &str) -> String {
        if token.len() > 8 {
            format!(
                "{}...{}",
                &token[..4],
                &token[token.len() - 4..]
            )
        } else {
            "***".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacted_github_token() {
        let config = Config {
            git: GitConfig {
                user_name: "n".to_string(),
                user_email: "e@example.com".to_string(),
                default_branch: None,
            },
            github: Some(GitHubConfig {
                token: "ghp_1234567890abcdefghij".to_string(),
                username: "user".to_string(),
            }),
            metadata: ConfigMetadata {
                synced_at: "t".to_string(),
                vault_name: None,
            },
        };

        let redacted = config.redacted();
        let token = redacted.github.expect("github").token;
        assert!(token.contains("..."));
        assert_ne!(token, "ghp_1234567890abcdefghij");
    }
}
