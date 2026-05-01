//! 1Password CLI (`op`) integration for loading Terry configuration.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::process::{Command, Stdio};

pub const ITEM_TERRY_GITHUB: &str = "GitHub";

#[derive(Debug)]
pub struct OnePasswordClient {
    vault: String,
}

impl OnePasswordClient {
    pub fn new(vault: &str) -> Self {
        Self {
            vault: vault.to_string(),
        }
    }

    /// Check that `op` is installed and an account session is available.
    ///
    /// If `op whoami` fails (typical when no session exists), runs `op signin --force`
    /// so the 1Password desktop app can authenticate the CLI ([app integration](https://developer.1password.com/docs/cli/app-integration/)),
    /// then verifies the session again.
    pub fn verify_setup(&self) -> Result<(), OpError> {
        let output = Command::new("op")
            .arg("--version")
            .output()
            .map_err(op_spawn_error)?;

        if !output.status.success() {
            return Err(OpError::NotInstalled);
        }

        if whoami_check().is_ok() {
            return Ok(());
        }

        implicit_op_signin()?;
        whoami_check()
    }

    /// Full item as JSON (`op item get … --format json`).
    pub fn get_item(&self, item_name: &str) -> Result<OpItem, OpError> {
        let mut cmd = Command::new("op");
        cmd.arg("item").arg("get").arg(item_name);
        cmd.arg("--format").arg("json");
        self.apply_vault(&mut cmd);

        let output = cmd.output().map_err(op_spawn_error)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OpError::ItemNotFound(
                item_name.to_string(),
                stderr.trim().to_string(),
            ));
        }

        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| OpError::ParseError(e.to_string()))?;
        OpItem::from_json_value(json)
    }

    /// Single field by label (`op item get … --fields label=… --reveal`).
    pub fn get_field(&self, item_name: &str, field_label: &str) -> Result<String, OpError> {
        let mut cmd = Command::new("op");
        cmd.arg("item").arg("get").arg(item_name);
        cmd.arg("--fields").arg(format!("label={field_label}"));
        cmd.arg("--reveal");
        cmd.arg("--format").arg("json");
        self.apply_vault(&mut cmd);

        let output = cmd.output().map_err(op_spawn_error)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OpError::FieldNotFound(
                field_label.to_string(),
                stderr.trim().to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Err(OpError::FieldNotFound(
                field_label.to_string(),
                "(empty response)".to_string(),
            ));
        }

        let json: Value =
            serde_json::from_str(trimmed).map_err(|e| OpError::ParseError(e.to_string()))?;

        parse_field_value(&json, field_label).ok_or_else(|| {
            OpError::FieldNotFound(
                field_label.to_string(),
                "field not present in JSON response".to_string(),
            )
        })
    }

    fn apply_vault(&self, cmd: &mut Command) {
        cmd.arg("--vault").arg(self.vault.as_str());
    }
}

fn whoami_check() -> Result<(), OpError> {
    let output = Command::new("op")
        .arg("whoami")
        .output()
        .map_err(|e| OpError::CommandFailed(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    if looks_like_not_signed_in(&combined) {
        return Err(OpError::NotSignedIn);
    }
    Err(OpError::NotSignedInWithDetail(combined.trim().to_string()))
}

/// `op signin` is idempotent; `--force` avoids the “run eval $(op signin)” warning when stdout
/// is not a shell. Inherited stdio lets the desktop app / system prompt for biometrics.
fn implicit_op_signin() -> Result<(), OpError> {
    let mut cmd = Command::new("op");
    cmd.arg("signin");
    cmd.arg("--force");
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd.status().map_err(op_spawn_error)?;
    if !status.success() {
        return Err(OpError::SignInFailed(
            "'op signin --force' exited unsuccessfully.".into(),
        ));
    }
    Ok(())
}

fn looks_like_not_signed_in(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("no account")
        || lower.contains("not signed")
        || lower.contains("sign in")
        || lower.contains("signin")
        || lower.contains("authenticate")
        || lower.contains("eval $(")
        || lower.contains("meant to be executed")
}

fn op_spawn_error(err: io::Error) -> OpError {
    if err.kind() == io::ErrorKind::NotFound {
        OpError::NotInstalled
    } else {
        OpError::CommandFailed(err.to_string())
    }
}

/// Extract a field value from `op item get … --format json` output (full or partial item).
fn parse_field_value(root: &Value, label: &str) -> Option<String> {
    if let Some(fields) = root.get("fields").and_then(|f| f.as_array()) {
        for field in fields {
            let Some(field_label) = field.get("label").and_then(|l| l.as_str()) else {
                continue;
            };
            if field_label != label {
                continue;
            }
            let value = field.get("value")?;
            return json_value_as_string(value);
        }
        return None;
    }

    // Some shapes expose a single `value` at the root (defensive).
    root.get("value").and_then(json_value_as_string)
}

fn json_value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Value::Null => None,
        _ => Some(value.to_string().trim_matches('"').to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpItem {
    pub id: String,
    pub title: String,
    pub fields: Vec<OpField>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpField {
    #[serde(default)]
    pub id: String,
    pub label: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub field_type: Option<String>,
}

impl OpItem {
    pub fn from_json_value(json: Value) -> Result<Self, OpError> {
        serde_json::from_value(json).map_err(|e| OpError::ParseError(e.to_string()))
    }

    pub fn get_field_value(&self, label: &str) -> Option<String> {
        self.fields.iter().find_map(|f| {
            let matches = f.label.as_deref() == Some(label);
            if !matches {
                return None;
            }
            f.value
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("1Password CLI not installed. Run: just install-1password-cli")]
    NotInstalled,

    #[error(
        "Not signed in to 1Password. Enable the 1Password app under Settings → Developer → Integrate with 1Password CLI, keep the app running, then retry."
    )]
    NotSignedIn,

    #[error("1Password session error: {0}")]
    NotSignedInWithDetail(String),

    #[error("{0}")]
    SignInFailed(String),

    #[error("1Password command failed: {0}")]
    CommandFailed(String),

    #[error("1Password item '{0}' not found: {1}")]
    ItemNotFound(String, String),

    #[error("1Password field '{0}' not found: {1}")]
    FieldNotFound(String, String),

    #[error("Failed to parse 1Password output: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_op_item_get_field_value() {
        let item = OpItem {
            id: "abc".into(),
            title: "Test".into(),
            fields: vec![OpField {
                id: "1".into(),
                label: Some("username".into()),
                value: Some("testuser".into()),
                field_type: Some("STRING".into()),
            }],
        };
        assert_eq!(
            item.get_field_value("username"),
            Some("testuser".to_string())
        );
    }

    #[test]
    fn test_op_item_from_json() {
        let json = json!({
            "id": "abc123",
            "title": "Test Item",
            "fields": [
                {
                    "id": "field1",
                    "label": "username",
                    "value": "testuser",
                    "type": "STRING"
                }
            ]
        });
        let item = OpItem::from_json_value(json).unwrap();
        assert_eq!(
            item.get_field_value("username"),
            Some("testuser".to_string())
        );
    }

    #[test]
    fn test_parse_field_value_from_op_shape() {
        let json = json!({
            "title": "Terry GitHub",
            "fields": [
                {"label": "username", "value": "octocat", "type": "STRING"}
            ]
        });
        assert_eq!(
            parse_field_value(&json, "username").as_deref(),
            Some("octocat")
        );
    }

    #[test]
    fn test_op_error_display_mentions_cli() {
        let err = OpError::NotInstalled;
        assert!(err.to_string().contains("install-1password-cli"));
    }

    #[test]
    #[ignore = "requires 1Password CLI sign-in; run: cargo test -p terrance verify_setup -- --ignored"]
    fn verify_setup_integration() {
        OnePasswordClient::new("test vault")
            .verify_setup()
            .expect("op should be installed and signed in");
    }
}
