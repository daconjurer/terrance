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
    /// Read-only PAT (fine-grained: **Metadata** + **Contents**, both read-only). Default for least-privilege; used for `gh repo view` / `terry github exists`.
    pub token: String,
    /// PAT with permission to create repos (fine-grained: **Metadata** read-only + **Administration** read/write). Used for `gh repo create` / `terry github create-repo`. Omitted in older encrypted configs until re-sync.
    #[serde(default)]
    pub token_write: Option<String>,
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
        config.github.token_write = config
            .github
            .token_write
            .as_ref()
            .map(|t| Self::redact_token(t));
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
    fn config_deserializes_without_token_write() {
        let json = r#"{"github":{"token":"ghp_x","username":"u"},"metadata":{"synced_at":"","vault_name":""}}"#;
        let c: Config = serde_json::from_str(json).expect("parse legacy");
        assert!(c.github.token_write.is_none());
    }

    #[test]
    fn test_redacted_github_token() {
        let config = Config {
            github: GitHubConfig {
                token: "ghp_1234567890abcdefghij".to_string(),
                token_write: Some("ghp_writetoken_abcdefghij".to_string()),
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
        let rw = redacted.github.token_write.as_ref().expect("redacted write token");
        assert!(rw.contains("..."));
        assert_ne!(rw, "ghp_writetoken_abcdefghij");
    }

    #[test]
    fn redacted_without_token_write_leaves_none() {
        let config = Config {
            github: GitHubConfig {
                token: "ghp_abc".to_string(),
                token_write: None,
                username: "u".to_string(),
            },
            metadata: ConfigMetadata {
                synced_at: "".to_string(),
                vault_name: "".to_string(),
            },
        };
        assert!(config.redacted().github.token_write.is_none());
    }
}
