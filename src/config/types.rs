use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// GitHub configuration
    pub github: GitHubConfig,

    /// Metadata
    pub metadata: ConfigMetadata,

    /// Project init template sources (from 1Password item Project Templates).
    pub templates: TemplatesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatesConfig {
    pub agentic: TemplateSource,
    pub languages: LanguageTemplates,
}

/// Fixed language keys — not a string map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageTemplates {
    pub go: TemplateSource,
    pub rust: TemplateSource,
    pub typescript: TemplateSource,
    pub python: TemplateSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum TemplateLanguage {
    Go,
    Rust,
    TypeScript,
    Python,
}

impl LanguageTemplates {
    pub fn get(&self, lang: TemplateLanguage) -> &TemplateSource {
        match lang {
            TemplateLanguage::Go => &self.go,
            TemplateLanguage::Rust => &self.rust,
            TemplateLanguage::TypeScript => &self.typescript,
            TemplateLanguage::Python => &self.python,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSource {
    /// Release tarball URL; `{ref}` substituted at fetch time using `ref_name`.
    pub url: String,
    /// Pinned tag or release name, e.g. v0.1.0
    pub ref_name: String,
    /// Optional sha256 integrity check (hex or `sha256:` prefix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
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
pub(crate) fn sample_templates_config() -> TemplatesConfig {
    let source = |path: &str| TemplateSource {
        url: format!(
            "https://example.com/{path}/releases/download/{{ref}}/template.tar.gz"
        ),
        ref_name: "v0.1.0".to_string(),
        checksum: None,
    };
    TemplatesConfig {
        agentic: source("terry-template-agentic"),
        languages: LanguageTemplates {
            go: source("terry-template-go"),
            rust: source("terry-template-rust"),
            typescript: source("terry-template-typescript"),
            python: source("terry-template-python"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_deserializes_without_token_write() {
        let json = r#"{"github":{"token":"ghp_x","username":"u"},"metadata":{"synced_at":"","vault_name":""},"templates":{"agentic":{"url":"https://example.com/a/{ref}/t.tar.gz","ref_name":"v0.1.0"},"languages":{"go":{"url":"https://example.com/go/{ref}/t.tar.gz","ref_name":"v0.1.0"},"rust":{"url":"https://example.com/rust/{ref}/t.tar.gz","ref_name":"v0.1.0"},"typescript":{"url":"https://example.com/ts/{ref}/t.tar.gz","ref_name":"v0.1.0"},"python":{"url":"https://example.com/py/{ref}/t.tar.gz","ref_name":"v0.1.0"}}}}"#;
        let c: Config = serde_json::from_str(json).expect("parse legacy");
        assert!(c.github.token_write.is_none());
    }

    #[test]
    fn config_deserialize_fails_without_templates() {
        let json = r#"{"github":{"token":"ghp_x","username":"u"},"metadata":{"synced_at":"","vault_name":""}}"#;
        let err = serde_json::from_str::<Config>(json).unwrap_err();
        assert!(err.to_string().contains("templates"));
    }

    #[test]
    fn language_templates_json_uses_fixed_keys() {
        let templates = sample_templates_config();
        let json = serde_json::to_string(&templates.languages).expect("serialize");
        assert!(json.contains("\"go\""));
        assert!(json.contains("\"rust\""));
        assert!(json.contains("\"typescript\""));
        assert!(json.contains("\"python\""));

        let roundtrip: LanguageTemplates = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(roundtrip.go.ref_name, "v0.1.0");
        assert_eq!(
            roundtrip.get(TemplateLanguage::Rust).url,
            templates.languages.rust.url
        );
    }

    #[test]
    fn templates_config_serde_roundtrip() {
        let templates = sample_templates_config();
        let json = serde_json::to_string(&templates).expect("serialize");
        let parsed: TemplatesConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.agentic.url, templates.agentic.url);
        assert_eq!(
            parsed.languages.get(TemplateLanguage::Python).ref_name,
            "v0.1.0"
        );
    }

    #[test]
    fn template_source_omits_none_checksum() {
        let source = TemplateSource {
            url: "https://example.com/{ref}/t.tar.gz".to_string(),
            ref_name: "v0.1.0".to_string(),
            checksum: None,
        };
        let json = serde_json::to_string(&source).expect("serialize");
        assert!(!json.contains("checksum"));
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
            templates: sample_templates_config(),
        };

        let redacted = config.redacted();
        assert!(redacted.github.token.contains("..."));
        assert_ne!(redacted.github.token, "ghp_1234567890abcdefghij");
        let rw = redacted
            .github
            .token_write
            .as_ref()
            .expect("redacted write token");
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
            templates: sample_templates_config(),
        };
        assert!(config.redacted().github.token_write.is_none());
    }
}
