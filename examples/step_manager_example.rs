use terrance::{Step, StepManager};

fn main() {
    // Create a step manager and add steps
    let manager = StepManager::new()
        .add_step(Step::new("Initialize Git", "git init {path}").add_arg("path", "."))
        .add_step(
            Step::new("Add remote", "git remote add {name} {url}")
                .add_arg("name", "origin")
                .add_arg("url", "https://github.com/user/repo.git"),
        )
        .add_step(Step::new("Show remotes", "git remote -v"));

    // Execute all steps sequentially
    match manager.execute() {
        Ok(outputs) => {
            println!("✓ All steps completed successfully!");
            for (i, output) in outputs.iter().enumerate() {
                if !output.trim().is_empty() {
                    println!("  Step {}: {}", i + 1, output.trim());
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Execution failed: {}", e);
        }
    }
}
