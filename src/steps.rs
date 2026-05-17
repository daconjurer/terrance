//! Builds and runs shell-style commands as subprocess steps: `{placeholder}` substitution in the
//! command string, per-step environment overrides, and ordered execution via [`StepManager`].
//!
//! **Production vs tests:** CLI commands such as `terry project init` spawn `git commit` without
//! setting author environment variables on the [`Step`]. **Unit tests** that run `git commit` often
//! use [`Step::add_env`] with `GIT_AUTHOR_*` / `GIT_COMMITTER_*` so commits succeed in CI or clean
//! environments without relying on a developer’s global Git config.

use crate::error::{StepError, StepManagerError};
use std::collections::HashMap;
use std::process::Command;

/// Something that can be executed and returns captured output (stdout/stderr) as a [`String`].
pub trait Runnable {
    /// Runs this runnable to completion and returns combined stdout and stderr on success.
    fn run(&self) -> Result<String, StepError>;
}

/// One subprocess step: a command template, optional `{key}` substitutions, and optional env vars.
#[derive(Debug, Clone)]
pub struct Step {
    /// Human-readable label used in errors (not passed to the child process).
    name: String,
    /// Command line template split on ASCII whitespace into argv; `{key}` fragments are replaced using [`Self::add_arg`].
    command: String,
    /// Maps placeholder keys to replacement text for `{key}` in `command`.
    args: HashMap<String, String>,
    /// Extra environment entries for the child; merged with the process environment (see [`Self::add_env`]).
    env: HashMap<String, String>,
    /// When set, [`Runnable::run`] spawns this argv directly (whitespace-splitting the template is skipped).
    argv_override: Option<Vec<String>>,
}

impl Step {
    /// Creates a step with no args and no extra env vars.
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: HashMap::new(),
            env: HashMap::new(),
            argv_override: None,
        }
    }

    /// Creates a step that runs `argv` as the subprocess program and arguments (supports values with spaces).
    ///
    /// Use this when the template splitter would break quoted text (for example `git commit -m` with a
    /// message that contains spaces). Combine with [`Self::add_env`] if a test must override `GIT_AUTHOR_*`
    /// for that child only; production `project init` does not set author env on the commit step.
    ///
    /// The template `command` string is left empty; execution uses `argv_override` only.
    pub fn with_argv(name: &str, argv: Vec<String>) -> Self {
        Self {
            name: name.into(),
            command: String::new(),
            args: HashMap::new(),
            env: HashMap::new(),
            argv_override: Some(argv),
        }
    }

    /// Registers a `{key}` placeholder replacement applied when the step runs.
    pub fn add_arg(mut self, key: &str, value: &str) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    /// Adds or overrides an environment variable for the child process (inherits the rest of the environment).
    ///
    /// Typical in **tests** (for example `GIT_AUTHOR_NAME` on a `git commit` step). Production
    /// `terry project init` does not use this for the initial commit.
    ///
    /// Calling this twice with the same `key` keeps the last value.
    #[allow(dead_code)]
    pub fn add_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Substitutes all `{key}` placeholders in `command` using `args`.
    fn render_command(&self) -> String {
        if let Some(argv) = &self.argv_override {
            return argv.join(" ");
        }
        let mut rendered = self.command.clone();
        for (key, value) in &self.args {
            let placeholder = format!("{{{}}}", key);
            rendered = rendered.replace(&placeholder, value);
        }
        rendered
    }

    /// Returns the step’s display name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Runnable for Step {
    /// Spawns the rendered argv, applies per-step environment overrides, captures output, and maps failures to [`StepError`].
    fn run(&self) -> Result<String, StepError> {
        let parts: Vec<String> = if let Some(argv) = &self.argv_override {
            argv.clone()
        } else {
            let rendered_command = self.render_command();
            rendered_command
                .split_whitespace()
                .map(String::from)
                .collect()
        };

        if parts.is_empty() {
            return Err(StepError::EmptyCommand);
        }

        let mut cmd = Command::new(&parts[0]);
        cmd.args(&parts[1..]);
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let output = cmd.output().map_err(|e| StepError::ExecutionFailed {
            step_name: self.name.clone(),
            source: e,
        })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if !stderr.is_empty() {
                format!("{}{}", stdout, stderr)
            } else {
                stdout
            };
            Ok(combined)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(StepError::StepFailed {
                step_name: self.name.clone(),
                stderr,
            })
        }
    }
}

/// Runs an ordered list of [`Step`] values via [`Runnable::run`], stopping on the first failure.
pub struct StepManager {
    /// Steps run from front to back.
    steps: Vec<Step>,
}

impl StepManager {
    /// Creates an empty manager (no steps).
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Appends a step to the end of the execution order.
    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Runs every step in order; returns each step’s captured output, or the first error.
    pub fn execute(&self) -> Result<Vec<String>, StepManagerError> {
        if self.steps.is_empty() {
            return Err(StepManagerError::NoSteps);
        }

        let mut results = Vec::new();

        for (index, step) in self.steps.iter().enumerate() {
            match step.run() {
                Ok(output) => {
                    results.push(output);
                }
                Err(e) => {
                    return Err(StepManagerError::StepExecutionFailed {
                        step_name: step.name().to_string(),
                        position: index,
                        source: e,
                    });
                }
            }
        }

        Ok(results)
    }
}

impl Default for StepManager {
    /// Same as [`StepManager::new`].
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for [`Step`] and [`StepManager`].

    use super::*;

    mod step {
        //! Tests for [`Step`].

        use super::*;

        /// Builds a trivial step and checks initial fields.
        #[test]
        fn test_step_creation() {
            let step = Step::new("test", "echo hello");
            assert_eq!(step.name(), "test");
            assert_eq!(step.command, "echo hello");
            assert!(step.args.is_empty());
            assert!(step.env.is_empty());
            assert!(step.argv_override.is_none());
        }

        /// [`Step::with_argv`] keeps multi-word arguments intact when the process runs.
        /// Uses [`Step::add_env`] for `GIT_AUTHOR_*` / `GIT_COMMITTER_*` so `git commit` does not depend on global Git config.
        #[test]
        fn test_with_argv_commit_message_with_spaces() {
            use std::fs;

            let temp_dir = std::env::temp_dir().join("terry_test_with_argv_commit");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).expect("mkdir temp");
            let path = temp_dir.to_str().expect("path utf-8");

            let init = Step::new("git init", "git init {path}").add_arg("path", path);
            init.run().expect("git init");

            fs::write(temp_dir.join("a.txt"), "x").expect("write file");
            Step::new("git add", "git -C {path} add a.txt")
                .add_arg("path", path)
                .run()
                .expect("git add");

            let step = Step::with_argv(
                "commit",
                vec![
                    "git".into(),
                    "-C".into(),
                    path.into(),
                    "commit".into(),
                    "-m".into(),
                    "initial commit".into(),
                ],
            )
            .add_env("GIT_AUTHOR_NAME", "test")
            .add_env("GIT_AUTHOR_EMAIL", "test@example.com")
            .add_env("GIT_COMMITTER_NAME", "test")
            .add_env("GIT_COMMITTER_EMAIL", "test@example.com");

            step.run().expect("git commit");

            let msg = std::process::Command::new("git")
                .args(["-C", path, "log", "-1", "--format=%s"])
                .output()
                .expect("git log");
            assert_eq!(
                String::from_utf8_lossy(&msg.stdout).trim(),
                "initial commit"
            );

            fs::remove_dir_all(&temp_dir).expect("cleanup");
        }

        /// [`Step::render_command`] joins argv override with spaces for display.
        #[test]
        fn test_render_command_argv_override() {
            let step = Step::with_argv(
                "x",
                vec![
                    "git".into(),
                    "commit".into(),
                    "-m".into(),
                    "hello world".into(),
                ],
            );
            assert_eq!(step.render_command(), "git commit -m hello world");
        }

        /// Ensures [`Step::add_arg`] stores the placeholder map entry.
        #[test]
        fn test_step_add_arg() {
            let step = Step::new("test", "echo {msg}").add_arg("msg", "hello world");

            assert_eq!(
                step.args.get("msg").expect("msg arg should exist"),
                "hello world"
            );
        }

        /// Ensures [`Step::add_env`] stores the env map entry.
        #[test]
        fn test_step_add_env() {
            let step = Step::new("test", "true").add_env("_TERRY_STEP_ENV_CHECK", "hello");

            assert_eq!(
                step.env
                    .get("_TERRY_STEP_ENV_CHECK")
                    .expect("env key should exist"),
                "hello"
            );
        }

        /// Runs [`Step`] with one extra env var visible to `printenv`.
        #[test]
        fn test_run_with_single_env_var() {
            let step = Step::new("printenv one var", "printenv _TERRY_STEP_SINGLE")
                .add_env("_TERRY_STEP_SINGLE", "from_step");

            let output = step.run().expect("command should succeed");
            assert_eq!(output.trim(), "from_step");
        }

        /// Confirms two [`Step::add_env`] entries are both visible in separate runs.
        #[test]
        fn test_run_with_multiple_env_vars() {
            let step = Step::new("printenv A", "printenv _TERRY_STEP_A")
                .add_env("_TERRY_STEP_A", "one")
                .add_env("_TERRY_STEP_B", "two");

            assert_eq!(step.run().expect("command should succeed").trim(), "one");

            let step_b = Step::new("printenv B", "printenv _TERRY_STEP_B")
                .add_env("_TERRY_STEP_A", "one")
                .add_env("_TERRY_STEP_B", "two");

            assert_eq!(step_b.run().expect("command should succeed").trim(), "two");
        }

        /// Duplicate [`Step::add_env`] keys behave like a single override to the last value.
        #[test]
        fn test_add_env_last_wins_on_duplicate_key() {
            let step = Step::new("env override", "printenv _TERRY_STEP_DUP")
                .add_env("_TERRY_STEP_DUP", "first")
                .add_env("_TERRY_STEP_DUP", "second");

            let output = step.run().expect("command should succeed");
            assert_eq!(output.trim(), "second");
        }

        /// [`Step::render_command`] leaves the template unchanged when there are no args.
        #[test]
        fn test_render_command_no_args() {
            let step = Step::new("test", "git init");
            assert_eq!(step.render_command(), "git init");
        }

        /// Substitutes multiple distinct placeholders in one template.
        #[test]
        fn test_render_command_with_args() {
            let step = Step::new("test", "git remote add {remote_name} {remote_url}")
                .add_arg("remote_name", "origin")
                .add_arg("remote_url", "https://github.com/user/repo.git");

            assert_eq!(
                step.render_command(),
                "git remote add origin https://github.com/user/repo.git"
            );
        }

        /// Replaces every occurrence of the same `{key}` in the template.
        #[test]
        fn test_render_command_multiple_same_placeholder() {
            let step = Step::new("test", "echo {word} {word}").add_arg("word", "hello");

            assert_eq!(step.render_command(), "echo hello hello");
        }

        /// Runs a fixed echo command with no placeholders.
        #[test]
        fn test_run_simple_command() {
            let step = Step::new("echo test", "echo hello");
            let result = step.run();

            assert!(result.is_ok());
            let output = result.expect("command should succeed");
            assert_eq!(output.trim(), "hello");
        }

        /// Runs echo with a substituted argument.
        #[test]
        fn test_run_command_with_args() {
            let step = Step::new("echo with args", "echo {message}").add_arg("message", "test123");

            let result = step.run();
            assert!(result.is_ok());
            let output = result.expect("command should succeed");
            assert_eq!(output.trim(), "test123");
        }

        /// Maps a non-zero exit status to [`StepError::StepFailed`].
        #[test]
        fn test_run_failing_command() {
            let step = Step::new("failing command", "false");
            let result = step.run();

            assert!(result.is_err());
            let error = result.expect_err("command should fail");
            assert!(error.to_string().contains("failing command"));
        }

        /// Missing executables surface as execution failures.
        #[test]
        fn test_run_nonexistent_command() {
            let step = Step::new("nonexistent", "this_command_does_not_exist_12345");
            let result = step.run();

            assert!(result.is_err());
        }

        /// End-to-end `git init` against a temp directory.
        #[test]
        fn test_git_init_step() {
            use std::env;
            use std::fs;

            let temp_dir = env::temp_dir().join("terry_test_git_init");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).expect("should create temp directory");

            let temp_path = temp_dir.to_str().expect("temp path should be valid UTF-8");
            let step = Step::new("Initialize Git repository", "git init {path}")
                .add_arg("path", temp_path);

            let result = step.run();
            assert!(result.is_ok(), "git init should succeed: {:?}", result);

            let git_dir = temp_dir.join(".git");
            assert!(git_dir.exists(), ".git directory should be created");

            fs::remove_dir_all(&temp_dir).expect("should clean up temp directory");
        }
    }

    mod step_manager {
        //! Tests for [`StepManager`].

        use super::*;

        /// New manager starts with no steps.
        #[test]
        fn test_step_manager_creation() {
            let manager = StepManager::new();
            assert_eq!(manager.steps.len(), 0);
        }

        /// [`StepManager::add_step`] grows the internal list.
        #[test]
        fn test_step_manager_add_step() {
            let manager = StepManager::new()
                .add_step(Step::new("test", "echo hello"))
                .add_step(Step::new("test2", "echo world"));

            assert_eq!(manager.steps.len(), 2);
        }

        /// [`StepManager::execute`] rejects an empty step list.
        #[test]
        fn test_execute_empty_steps() {
            let manager = StepManager::new();
            let result = manager.execute();

            assert!(result.is_err());
            match result.expect_err("should fail with no steps") {
                StepManagerError::NoSteps => {}
                _ => panic!("Expected NoSteps error"),
            }
        }

        /// One successful step yields one output string.
        #[test]
        fn test_execute_single_step() {
            let manager = StepManager::new().add_step(Step::new("echo test", "echo hello"));

            let result = manager.execute();
            assert!(result.is_ok());

            let outputs = result.expect("execution should succeed");
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].trim(), "hello");
        }

        /// Steps run in insertion order with independent outputs.
        #[test]
        fn test_execute_multiple_steps() {
            let manager = StepManager::new()
                .add_step(Step::new("echo 1", "echo first"))
                .add_step(Step::new("echo 2", "echo second"))
                .add_step(Step::new("echo 3", "echo third"));

            let result = manager.execute();
            assert!(result.is_ok());

            let outputs = result.expect("execution should succeed");
            assert_eq!(outputs.len(), 3);
            assert_eq!(outputs[0].trim(), "first");
            assert_eq!(outputs[1].trim(), "second");
            assert_eq!(outputs[2].trim(), "third");
        }

        /// A failing step stops execution and reports its index.
        #[test]
        fn test_execute_early_abort_on_failure() {
            let manager = StepManager::new()
                .add_step(Step::new("echo 1", "echo first"))
                .add_step(Step::new("failing step", "false"))
                .add_step(Step::new("echo 3", "echo third"));

            let result = manager.execute();
            assert!(result.is_err());

            let error = result.expect_err("should fail at second step");
            match error {
                StepManagerError::StepExecutionFailed {
                    step_name,
                    position,
                    ..
                } => {
                    assert_eq!(step_name, "failing step");
                    assert_eq!(position, 1);
                }
                _ => panic!("Expected StepExecutionFailed error"),
            }
        }

        /// Placeholder substitution works across multiple managed steps.
        #[test]
        fn test_execute_with_args() {
            let manager = StepManager::new()
                .add_step(Step::new("echo with arg", "echo {msg}").add_arg("msg", "hello"))
                .add_step(Step::new("echo with arg 2", "echo {msg}").add_arg("msg", "world"));

            let result = manager.execute();
            assert!(result.is_ok());

            let outputs = result.expect("execution should succeed");
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].trim(), "hello");
            assert_eq!(outputs[1].trim(), "world");
        }

        /// Later steps observe filesystem effects from earlier steps.
        #[test]
        fn test_execute_sequential_dependency() {
            use std::env;
            use std::fs;

            let temp_dir = env::temp_dir().join("terry_test_sequential");
            let _ = fs::remove_dir_all(&temp_dir);
            fs::create_dir_all(&temp_dir).expect("should create temp directory");

            let test_file = temp_dir.join("test.txt");
            let test_file_path = test_file
                .to_str()
                .expect("test file path should be valid UTF-8");

            let manager = StepManager::new()
                .add_step(Step::new("create file", "touch {file}").add_arg("file", test_file_path))
                .add_step(
                    Step::new("write to file", "sh -c echo hello > {file}")
                        .add_arg("file", test_file_path),
                );

            let result = manager.execute();
            assert!(result.is_ok(), "sequential steps should succeed");

            assert!(test_file.exists(), "file should be created");

            fs::remove_dir_all(&temp_dir).expect("should clean up temp directory");
        }
    }
}
