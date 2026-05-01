use crate::error::{StepError, StepManagerError};
use std::collections::HashMap;
use std::process::Command;

pub trait Runnable {
    fn run(&self) -> Result<String, StepError>;
}

#[derive(Debug, Clone)]
pub struct Step {
    name: String,
    command: String,
    args: HashMap<String, String>,
}

impl Step {
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: HashMap::new(),
        }
    }

    pub fn add_arg(mut self, key: &str, value: &str) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    fn render_command(&self) -> String {
        let mut rendered = self.command.clone();
        for (key, value) in &self.args {
            let placeholder = format!("{{{}}}", key);
            rendered = rendered.replace(&placeholder, value);
        }
        rendered
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Runnable for Step {
    fn run(&self) -> Result<String, StepError> {
        let rendered_command = self.render_command();
        let parts: Vec<&str> = rendered_command.split_whitespace().collect();

        if parts.is_empty() {
            return Err(StepError::EmptyCommand);
        }

        let output = Command::new(parts[0])
            .args(&parts[1..])
            .output()
            .map_err(|e| StepError::ExecutionFailed {
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

pub struct StepManager {
    steps: Vec<Step>,
}

impl StepManager {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

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
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod step {
        use super::*;

        #[test]
        fn test_step_creation() {
            let step = Step::new("test", "echo hello");
            assert_eq!(step.name(), "test");
            assert_eq!(step.command, "echo hello");
            assert!(step.args.is_empty());
        }

        #[test]
        fn test_step_add_arg() {
            let step = Step::new("test", "echo {msg}").add_arg("msg", "hello world");

            assert_eq!(
                step.args.get("msg").expect("msg arg should exist"),
                "hello world"
            );
        }

        #[test]
        fn test_render_command_no_args() {
            let step = Step::new("test", "git init");
            assert_eq!(step.render_command(), "git init");
        }

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

        #[test]
        fn test_render_command_multiple_same_placeholder() {
            let step = Step::new("test", "echo {word} {word}").add_arg("word", "hello");

            assert_eq!(step.render_command(), "echo hello hello");
        }

        #[test]
        fn test_run_simple_command() {
            let step = Step::new("echo test", "echo hello");
            let result = step.run();

            assert!(result.is_ok());
            let output = result.expect("command should succeed");
            assert_eq!(output.trim(), "hello");
        }

        #[test]
        fn test_run_command_with_args() {
            let step = Step::new("echo with args", "echo {message}").add_arg("message", "test123");

            let result = step.run();
            assert!(result.is_ok());
            let output = result.expect("command should succeed");
            assert_eq!(output.trim(), "test123");
        }

        #[test]
        fn test_run_failing_command() {
            let step = Step::new("failing command", "false");
            let result = step.run();

            assert!(result.is_err());
            let error = result.expect_err("command should fail");
            assert!(error.to_string().contains("failing command"));
        }

        #[test]
        fn test_run_nonexistent_command() {
            let step = Step::new("nonexistent", "this_command_does_not_exist_12345");
            let result = step.run();

            assert!(result.is_err());
        }

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
        use super::*;

        #[test]
        fn test_step_manager_creation() {
            let manager = StepManager::new();
            assert_eq!(manager.steps.len(), 0);
        }

        #[test]
        fn test_step_manager_add_step() {
            let manager = StepManager::new()
                .add_step(Step::new("test", "echo hello"))
                .add_step(Step::new("test2", "echo world"));

            assert_eq!(manager.steps.len(), 2);
        }

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

        #[test]
        fn test_execute_single_step() {
            let manager = StepManager::new().add_step(Step::new("echo test", "echo hello"));

            let result = manager.execute();
            assert!(result.is_ok());

            let outputs = result.expect("execution should succeed");
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].trim(), "hello");
        }

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
