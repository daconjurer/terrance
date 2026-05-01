use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// GitHub configuration
    pub github: GitHubConfig,

    /// Metadata
    pub metadata: ConfigMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub synced_at: String,
    pub vault_name: String,
}

impl Config {
    pub fn redacted(&self) -> Self {
        let mut config = self.clone();
        config.github.token = Self::redact_token(&config.github.token);
        config
    }

    fn redact_token(token: &str) -> String {
        if token.len() > 8 {
            format!("{}...{}", &token[..4], &token[token.len() - 4..])
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
            github: GitHubConfig {
                token: "ghp_1234567890abcdefghij".to_string(),
                username: "user".to_string(),
            },
            metadata: ConfigMetadata {
                synced_at: "t".to_string(),
                vault_name: "test vault".to_string(),
            },
        };

        let redacted = config.redacted();
        assert!(redacted.github.token.contains("..."));
        assert_ne!(redacted.github.token, "ghp_1234567890abcdefghij");
    }
}
