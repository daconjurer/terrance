use clap::Subcommand;
use std::io::{self, Write};
use terrance::config::{
    Config, ConfigManager, ConfigMetadata, GitHubConfig, ITEM_TERRY_GITHUB,
    ITEM_TERRY_PROJECT_TEMPLATES, OnePasswordClient, OpError, OpItem, SECTION_TEMPLATE_AGENTIC,
    SECTION_TEMPLATE_GO, SECTION_TEMPLATE_PYTHON, SECTION_TEMPLATE_RUST,
    SECTION_TEMPLATE_TYPESCRIPT, TemplateSource, TemplatesConfig,
};

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Sync GitHub credentials and project templates from 1Password (`op`). Requires `--vault`; reads item **`Github`** with fields **`username`**, **`token`** (fine-grained PAT: **Metadata** + **Contents** read-only), and **`token_write`** (fine-grained PAT: **Metadata** read-only, **Administration** read/write), and item **`Project Templates`** with sections **`agentic`**, **`go`**, **`rust`**, **`typescript`**, and **`python`** (each with fields **`url`** and **`ref_name`**, optional **`checksum`**).
    Sync {
        /// 1Password vault name (required; passed to every `op` invocation)
        #[arg(short, long)]
        vault: String,

        /// Force re-sync even if config exists
        #[arg(short, long)]
        force: bool,
    },

    /// Show current configuration (redacted)
    Show {
        /// Show sensitive values (requires confirmation)
        #[arg(long)]
        reveal: bool,
    },

    /// Edit configuration
    Edit,

    /// Clear local encrypted configuration
    Clear {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Get configuration directory path
    Path,
}

pub fn handle_command(command: &ConfigCommands) {
    let result = match command {
        ConfigCommands::Sync { vault, force } => handle_sync(vault, *force),
        ConfigCommands::Show { reveal } => handle_show(*reveal),
        ConfigCommands::Edit => handle_edit(),
        ConfigCommands::Clear { yes } => handle_clear(*yes),
        ConfigCommands::Path => handle_path(),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn handle_sync(vault: &str, force: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;
    let updating_existing = manager.config_exists();

    if updating_existing && !force {
        println!(
            "Configuration already exists at {}. Use --force to re-sync.",
            manager.get_config_path().display()
        );
        return Ok(());
    }

    println!("Syncing configuration from 1Password…");

    let op_client = OnePasswordClient::new(vault);

    op_client.verify_setup().map_err(sync_op_message)?;

    println!("  Fetching GitHub configuration…");
    let github_config = fetch_github_config(&op_client)?;

    println!("  Fetching project templates…");
    let templates_config = fetch_templates_config(&op_client).map_err(templates_sync_message)?;

    let config = Config {
        github: github_config,
        metadata: ConfigMetadata {
            synced_at: chrono::Utc::now().to_rfc3339(),
            vault_name: vault.to_string(),
        },
        templates: templates_config,
    };

    manager.save_config(&config)?;

    println!();
    println!("✓ Configuration synced and encrypted successfully!");
    println!("  GitHub: {}", config.github.username);
    println!("  Templates: agentic + 4 language sources");
    if updating_existing {
        println!();
        println!(
            "  Note: Existing configs require re-sync after adding the \"{ITEM_TERRY_PROJECT_TEMPLATES}\" item to your vault."
        );
    }

    Ok(())
}

fn sync_op_message(err: OpError) -> Box<dyn std::error::Error + Send + Sync> {
    let mut msg = err.to_string();

    match &err {
        OpError::NotInstalled | OpError::NotSignedIn | OpError::NotSignedInWithDetail(_) => {
            msg.push_str("\n\nInstall the CLI (`just install-1password-cli`) and enable Integrate with 1Password CLI in the 1Password app (Settings → Developer). Terry runs `op signin --force` automatically when you are not signed in.");
            msg.push_str("\nVault items:");
            msg.push_str("\n  • \"");
            msg.push_str(ITEM_TERRY_GITHUB);
            msg.push_str("\" — fields username, token (read-only PAT), and token_write (repo-creation PAT); use concealed type for both tokens.");
            msg.push_str("\n  • \"");
            msg.push_str(ITEM_TERRY_PROJECT_TEMPLATES);
            msg.push_str("\" — Secure Note with sections agentic, go, rust, typescript, python; each section needs fields url and ref_name (optional checksum).");
        }
        OpError::SignInFailed(_) => {
            msg.push_str("\n\nOpen and unlock the 1Password app, confirm Integrate with 1Password CLI is enabled under Settings → Developer, then try again.");
        }
        _ => {}
    }

    msg.into()
}

fn fetch_github_config(client: &OnePasswordClient) -> Result<GitHubConfig, OpError> {
    let username = client.get_field(ITEM_TERRY_GITHUB, "username")?;
    let token = client.get_field(ITEM_TERRY_GITHUB, "token")?;
    let token_write = client.get_field(ITEM_TERRY_GITHUB, "token_write")?;

    Ok(GitHubConfig {
        token,
        token_write: Some(token_write),
        username,
    })
}

fn fetch_templates_config(client: &OnePasswordClient) -> Result<TemplatesConfig, OpError> {
    let item = client.get_item(ITEM_TERRY_PROJECT_TEMPLATES)?;
    Ok(TemplatesConfig {
        agentic: template_source_from_section(&item, SECTION_TEMPLATE_AGENTIC)?,
        languages: terrance::config::LanguageTemplates {
            go: template_source_from_section(&item, SECTION_TEMPLATE_GO)?,
            rust: template_source_from_section(&item, SECTION_TEMPLATE_RUST)?,
            typescript: template_source_from_section(&item, SECTION_TEMPLATE_TYPESCRIPT)?,
            python: template_source_from_section(&item, SECTION_TEMPLATE_PYTHON)?,
        },
    })
}

fn template_source_from_section(item: &OpItem, section: &str) -> Result<TemplateSource, OpError> {
    Ok(TemplateSource {
        url: item.require_section_field(section, "url")?,
        ref_name: item.require_section_field(section, "ref_name")?,
        checksum: item.get_section_field_value(section, "checksum"),
    })
}

fn templates_sync_message(err: OpError) -> Box<dyn std::error::Error + Send + Sync> {
    let mut msg = err.to_string();
    match &err {
        OpError::ItemNotFound(name, _) if name == ITEM_TERRY_PROJECT_TEMPLATES => {
            msg.push_str("\n\nCreate a Secure Note item \"");
            msg.push_str(ITEM_TERRY_PROJECT_TEMPLATES);
            msg.push_str("\" with sections agentic, go, rust, typescript, and python. Each section needs fields url and ref_name.");
        }
        OpError::SectionFieldNotFound {
            item,
            section,
            field,
        } if item == ITEM_TERRY_PROJECT_TEMPLATES => {
            msg.push_str("\n\nAdd field `");
            msg.push_str(field);
            msg.push_str("` to section `");
            msg.push_str(section);
            msg.push_str("` in the \"");
            msg.push_str(ITEM_TERRY_PROJECT_TEMPLATES);
            msg.push_str("\" item.");
        }
        _ => {}
    }
    msg.into()
}

fn handle_show(reveal: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    println!(
        "Configuration directory: {}",
        manager.config_dir_path().display()
    );
    println!(
        "Configuration file:      {}",
        manager.get_config_path().display()
    );
    println!("Config file exists:      {}", manager.config_exists());

    if !manager.config_exists() {
        return Ok(());
    }

    let config = manager.load_config()?;

    if reveal {
        print!("Reveal sensitive values? Type 'yes' to continue: ");
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let line = line.trim();
        if line != "yes" {
            println!("Aborted.");
            return Ok(());
        }

        let json = serde_json::to_string_pretty(&config)?;
        println!("{}", json);
    } else {
        let redacted = config.redacted();
        let json = serde_json::to_string_pretty(&redacted)?;
        println!("{}", json);
    }

    Ok(())
}

fn handle_edit() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Not yet implemented");
    Ok(())
}

fn handle_clear(skip_confirm: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;

    if !manager.config_exists() {
        println!(
            "No configuration file at {}.",
            manager.get_config_path().display()
        );
        return Ok(());
    }

    if !skip_confirm {
        print!(
            "Remove {}? Type 'yes' to continue: ",
            manager.get_config_path().display()
        );
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if line.trim() != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    manager.remove_config_file()?;
    println!("Configuration cleared.");
    Ok(())
}

fn handle_path() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = ConfigManager::new()?;
    println!("{}", manager.config_dir_path().display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrance::config::one_password::{
        ITEM_TERRY_PROJECT_TEMPLATES, SECTION_TEMPLATE_AGENTIC, SECTION_TEMPLATE_RUST,
    };

    fn project_templates_op_item() -> OpItem {
        let section_fields = |section: &str, url_suffix: &str| {
            vec![
                serde_json::json!({
                    "label": "url",
                    "value": format!("https://github.com/my-org/terry-template-{url_suffix}/releases/download/{{ref}}/template.tar.gz"),
                    "type": "STRING",
                    "section": { "label": section }
                }),
                serde_json::json!({
                    "label": "ref_name",
                    "value": "v0.1.0",
                    "type": "STRING",
                    "section": { "label": section }
                }),
            ]
        };

        let mut fields: Vec<serde_json::Value> = Vec::new();
        for (section, suffix) in [
            (SECTION_TEMPLATE_AGENTIC, "agentic"),
            ("go", "go"),
            (SECTION_TEMPLATE_RUST, "rust"),
            ("typescript", "typescript"),
            ("python", "python"),
        ] {
            fields.extend(section_fields(section, suffix));
        }

        OpItem::from_json_value(serde_json::json!({
            "id": "abc",
            "title": ITEM_TERRY_PROJECT_TEMPLATES,
            "fields": fields
        }))
        .expect("item")
    }

    #[test]
    fn fetch_templates_config_builds_from_multi_section_item() {
        let item = project_templates_op_item();
        let config = TemplatesConfig {
            agentic: template_source_from_section(&item, SECTION_TEMPLATE_AGENTIC).unwrap(),
            languages: terrance::config::LanguageTemplates {
                go: template_source_from_section(&item, "go").unwrap(),
                rust: template_source_from_section(&item, SECTION_TEMPLATE_RUST).unwrap(),
                typescript: template_source_from_section(&item, "typescript").unwrap(),
                python: template_source_from_section(&item, "python").unwrap(),
            },
        };
        assert!(config.agentic.url.contains("terry-template-agentic"));
        assert!(config.languages.rust.url.contains("terry-template-rust"));
        assert_eq!(config.languages.rust.ref_name, "v0.1.0");
    }

    #[test]
    fn template_source_from_section_fails_when_required_field_missing() {
        let item = project_templates_op_item();
        let err = template_source_from_section(&item, "missing-section").unwrap_err();
        assert!(matches!(err, OpError::SectionFieldNotFound { .. }));
        let msg = err.to_string();
        assert!(msg.contains("missing-section"));
    }

    #[test]
    fn templates_sync_message_mentions_project_templates_item() {
        let err = OpError::ItemNotFound(
            ITEM_TERRY_PROJECT_TEMPLATES.to_string(),
            "missing".to_string(),
        );
        let msg = templates_sync_message(err).to_string();
        assert!(msg.contains(ITEM_TERRY_PROJECT_TEMPLATES));
        assert!(msg.contains("agentic"));
    }

    #[test]
    fn templates_sync_message_mentions_section_and_field() {
        let err = OpError::SectionFieldNotFound {
            item: ITEM_TERRY_PROJECT_TEMPLATES.to_string(),
            section: "rust".to_string(),
            field: "ref_name".to_string(),
        };
        let msg = templates_sync_message(err).to_string();
        assert!(msg.contains("rust"));
        assert!(msg.contains("ref_name"));
    }
}
