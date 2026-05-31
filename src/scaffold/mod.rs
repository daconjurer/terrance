//! Project init scaffolding (agentic + language templates). Phase 1 wires orchestration;
//! fetch/merge is implemented in later phases.

use crate::config::{Config, TemplateLanguage, TemplatesConfig};
use std::path::PathBuf;
use thiserror::Error;

pub const SCAFFOLDING_COMMIT_MESSAGE: &str = "chore: add project scaffolding";

#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    pub skip_agentic: bool,
    pub language: Option<TemplateLanguage>,
    pub project_name: String,
    pub project_path: PathBuf,
    pub templates: TemplatesConfig,
}

/// Whether init needs synced template config (agentic default or explicit `--language`).
pub fn needs_templates(skip_agentic: bool, language: Option<TemplateLanguage>) -> bool {
    !skip_agentic || language.is_some()
}

pub fn require_templates(config: &Config) -> Result<&TemplatesConfig, ScaffoldError> {
    Ok(&config.templates)
}

/// Phase 1 placeholder: validates options and resolves template sources; no files written yet.
    pub fn apply_scaffolding(options: &ScaffoldOptions) -> Result<bool, ScaffoldError> {
    if let Some(lang) = options.language {
        let source = options.templates.languages.get(lang);
        let _ = &source.url;
        let _ = &source.ref_name;
    }

    if !options.skip_agentic {
        let _ = &options.templates.agentic.url;
        let _ = &options.templates.agentic.ref_name;
    }

        Ok(false)
    }

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error(
        "Project templates are not configured. Run `terry config sync --vault <name>` after adding the \"Project Templates\" item to your vault."
    )]
    TemplatesMissing,

    #[error("{0}")]
    Other(String),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ScaffoldError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ScaffoldError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::sample_templates_config;

    #[test]
    fn needs_templates_when_agentic_default() {
        assert!(needs_templates(false, None));
    }

    #[test]
    fn needs_templates_when_language_set() {
        assert!(needs_templates(true, Some(TemplateLanguage::Rust)));
    }

    #[test]
    fn needs_templates_false_when_skip_agentic_and_no_language() {
        assert!(!needs_templates(true, None));
    }

    #[test]
    fn apply_scaffolding_stub_succeeds_with_sample_config() {
        let templates = sample_templates_config();
        let options = ScaffoldOptions {
            skip_agentic: false,
            language: Some(TemplateLanguage::Go),
            project_name: "demo".to_string(),
            project_path: PathBuf::from("/tmp/demo"),
            templates,
        };
        assert!(!apply_scaffolding(&options).expect("stub"));
    }
}
