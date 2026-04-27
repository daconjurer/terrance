use thiserror::Error;

#[derive(Error, Debug)]
pub enum StepError {
    #[error("Command is empty")]
    EmptyCommand,

    #[error("Failed to execute '{step_name}': {source}")]
    ExecutionFailed {
        step_name: String,
        source: std::io::Error,
    },

    #[error("Step '{step_name}' failed: {stderr}")]
    StepFailed { step_name: String, stderr: String },
}

#[derive(Error, Debug)]
pub enum StepManagerError {
    #[error("Step '{step_name}' failed at position {position}: {source}")]
    StepExecutionFailed {
        step_name: String,
        position: usize,
        source: StepError,
    },

    #[error("No steps to execute")]
    NoSteps,
}
