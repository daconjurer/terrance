//! `terry project` CLI: initialize a local git repo, optional GitHub remote and submodule, and
//! optional GitHub repo creation via a subprocess that runs the same `terry` binary
//! (`github create-repo`).
//!
//! **Git author for the initial commit:** In production, `terry` does **not** set `GIT_AUTHOR_*`,
//! `GIT_COMMITTER_*`, or `git config user.*`. The first commit uses whatever identity Git already
//! resolves (global/system/repo config). **Unit tests** inject author env vars only in the test
//! process (see `GitAuthorEnvForTest` in this module’s `#[cfg(test)]` submodule) or on individual
//! [`crate::steps::Step`]s so `cargo test` does not require a configured Git user.

use crate::steps::{Step, StepManager};
use clap::Subcommand;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use terrance::config::{ConfigManager, TemplateLanguage};
use terrance::github::GitHubClient;
use terrance::scaffold::{
    SCAFFOLDING_COMMIT_MESSAGE, ScaffoldOptions, apply_scaffolding, needs_templates,
    require_templates,
};

/// Subcommands under `terry project`.
#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Initialize a new project configuration
    Init {
        /// Name of the project
        #[arg(short, long)]
        name: String,

        /// Path to the project directory
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// GitHub repository slug under your synced GitHub username (`terry config sync`).
        /// Derives `origin` as SSH (`git@github.com:user/repo.git`), then creates the private repo
        /// on GitHub via `gh` after local `git init` (requires synced `token_write`). Omit for a
        /// local repository with no `origin`.
        #[arg(long)]
        repo_slug: Option<String>,

        /// Include a planning directory as a git submodule
        #[arg(long)]
        with_planning: bool,

        /// Skip Cursor/agent scaffolding (AGENTS.md, .cursor/, sandbox.json).
        #[arg(long)]
        skip_agentic: bool,

        /// Primary language template: go | rust | typescript | python
        #[arg(long, value_enum)]
        language: Option<TemplateLanguage>,
    },
}

/// Dispatches a [`ProjectCommands`] variant; on failure prints to stderr and exits the process.
pub fn handle_command(command: &ProjectCommands) {
    match command {
        ProjectCommands::Init {
            name,
            path,
            repo_slug,
            with_planning,
            skip_agentic,
            language,
        } => {
            if let Err(e) = handle_init(
                name,
                path.as_ref(),
                repo_slug.as_ref(),
                *with_planning,
                *skip_agentic,
                *language,
            ) {
                eprintln!("Error initializing project: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// `argv` entries after the executable for `terry github create-repo` (private repo, no `--add-remote`).
fn github_create_repo_cli_argv(slug: &str) -> Vec<String> {
    vec![
        "github".to_string(),
        "create-repo".to_string(),
        "--name".to_string(),
        slug.to_string(),
    ]
}

/// Full single-string command for [`Step`] (ASCII whitespace–split into argv).
fn github_create_repo_step_command(exe: &str, slug: &str) -> String {
    format!("{} {}", exe, github_create_repo_cli_argv(slug).join(" "))
}

/// Builds the [`Step`] that spawns `exe` with `github create-repo --name` for `slug` (see [`github_create_repo_cli_argv`]).
fn github_create_repo_step(exe: &str, slug: &str) -> Step {
    Step::new(
        "Create GitHub repository",
        &github_create_repo_step_command(exe, slug),
    )
}

/// Subject line for the first commit (empty `README.md` on `main` so the branch is a non-empty ref).
///
/// Author identity comes from the user’s normal Git configuration (or `GIT_*` env vars **they**
/// set in their shell). `terry` does not set author env vars for this command.
const INITIAL_COMMIT_MESSAGE: &str = "initial commit";

/// Runs `project init`: resolves the directory, optionally prompts for a planning submodule URL,
/// then executes steps in order—`mkdir`, `git init`, `main`, empty `README.md`, initial commit,
/// optional language/agentic scaffolding (Phase 1 stubs), optional scaffolding commit,
/// optional `remote add origin`, optional submodule, optional `terry github create-repo`, then push.
fn handle_init(
    name: &str,
    path: Option<&PathBuf>,
    repo_slug: Option<&String>,
    with_planning: bool,
    skip_agentic: bool,
    language: Option<TemplateLanguage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = resolve_project_path(path)?;
    let path_str = project_path.to_str().ok_or("Invalid path")?;

    println!(
        "Initializing project '{}' at {}",
        name,
        project_path.display()
    );

    let run_scaffolding = needs_templates(skip_agentic, language);
    let templates = if run_scaffolding {
        let manager = ConfigManager::new().map_err(|e| e as Box<dyn std::error::Error>)?;
        let config = manager
            .load_config()
            .map_err(|e| e as Box<dyn std::error::Error>)?;
        Some(require_templates(&config)?.clone())
    } else {
        None
    };

    let (remote_url, create_repo_slug) = if let Some(slug) = repo_slug {
        let trimmed = slug.trim();
        if trimmed.is_empty() {
            return Err("--repo-slug cannot be empty".into());
        }
        let client = GitHubClient::from_config()?;
        let url = client.origin_ssh_url(trimmed);
        (Some(url), Some(trimmed.to_string()))
    } else {
        (None, None)
    };
    let has_remote = remote_url.is_some();

    let planning_submodule_url = if with_planning {
        Some(get_planning_submodule_url()?)
    } else {
        None
    };

    let mut manager = StepManager::new().add_step(
        Step::new("Create project directory", "mkdir -p {path}").add_arg("path", path_str),
    );

    let readme_path = project_path.join("README.md");
    let readme_str = readme_path
        .to_str()
        .ok_or("README.md path is not valid UTF-8")?;

    manager = manager.add_step(
        Step::new("Initialize Git repository", "git init {path}").add_arg("path", path_str),
    );

    manager = manager
        .add_step(
            Step::new("Set default branch to main", "git -C {path} branch -M main")
                .add_arg("path", path_str),
        )
        .add_step(Step::new("Create README.md", "touch {readme}").add_arg("readme", readme_str))
        .add_step(
            Step::new("Stage README.md", "git -C {path} add README.md").add_arg("path", path_str),
        )
        // Production: no `GIT_AUTHOR_*` / per-step author env — uses Git’s resolved user identity.
        .add_step(Step::with_argv(
            "Create initial commit",
            vec![
                "git".to_string(),
                "-C".to_string(),
                path_str.to_string(),
                "commit".to_string(),
                "-m".to_string(),
                INITIAL_COMMIT_MESSAGE.to_string(),
            ],
        ));

    match manager.execute() {
        Ok(_) => {}
        Err(e) => return Err(Box::new(e)),
    }

    if run_scaffolding {
        let templates = templates.expect("templates loaded when scaffolding runs");
        let scaffolding_changed = apply_scaffolding(&ScaffoldOptions {
            skip_agentic,
            language,
            project_name: name.to_string(),
            project_path: project_path.clone(),
            templates,
        })?;

        if scaffolding_changed {
            let scaffold_manager = StepManager::new()
                .add_step(
                    Step::new("Stage scaffolding files", "git -C {path} add -A")
                        .add_arg("path", path_str),
                )
                .add_step(Step::with_argv(
                    "Create scaffolding commit",
                    vec![
                        "git".to_string(),
                        "-C".to_string(),
                        path_str.to_string(),
                        "commit".to_string(),
                        "-m".to_string(),
                        SCAFFOLDING_COMMIT_MESSAGE.to_string(),
                    ],
                ));

            match scaffold_manager.execute() {
                Ok(_) => {}
                Err(e) => return Err(Box::new(e)),
            }
        }
    }

    let mut remote_manager = StepManager::new();
    let mut has_remote_phase_steps = false;

    if let Some(url) = remote_url.as_ref() {
        remote_manager = remote_manager.add_step(
            Step::new("Add remote origin", "git -C {path} remote add origin {url}")
                .add_arg("path", path_str)
                .add_arg("url", url),
        );
        has_remote_phase_steps = true;
    }

    if let Some(submodule_url) = planning_submodule_url {
        remote_manager = remote_manager.add_step(
            Step::new(
                "Add planning repo as git submodule",
                "git -C {path} submodule add {submodule_url} planning",
            )
            .add_arg("path", path_str)
            .add_arg("submodule_url", &submodule_url),
        );
        has_remote_phase_steps = true;
    }

    if let Some(slug) = create_repo_slug.as_ref() {
        let exe = env::current_exe()
            .map_err(|e| format!("failed to resolve terry executable path: {e}"))?
            .to_str()
            .ok_or("terry executable path is not valid UTF-8")?
            .to_string();
        remote_manager = remote_manager.add_step(github_create_repo_step(&exe, slug));
        has_remote_phase_steps = true;
    }

    if has_remote {
        remote_manager = remote_manager.add_step(
            Step::new(
                "Push main branch to origin",
                "git -C {path} push -u origin main",
            )
            .add_arg("path", path_str),
        );
        has_remote_phase_steps = true;
    }

    if has_remote_phase_steps {
        match remote_manager.execute() {
            Ok(_) => {}
            Err(e) => return Err(Box::new(e)),
        }
    }

    println!("\n✓ Project '{}' initialized successfully!", name);
    if has_remote {
        println!("  Git repository created with remote origin (initial commit pushed to main)");
    } else {
        println!("  Git repository created on main with initial commit (no remote)");
    }
    if with_planning {
        println!("  Planning directory added as git submodule");
    }
    Ok(())
}

/// Project directory: explicit `--path` or the current working directory.
fn resolve_project_path(path: Option<&PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match path {
        Some(p) => Ok(p.clone()),
        None => Ok(env::current_dir()?),
    }
}

/// Prompts on stdout for the planning repository URL (`--with-planning`); reads a line from stdin. Empty input errors.
fn get_planning_submodule_url() -> Result<String, Box<dyn std::error::Error>> {
    print!("Planning repository URL: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        Err("Planning repository URL is required when using --with-planning".into())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Unit and integration tests for project path resolution, init validation, GitHub helper argv, and local `git init`.
///
/// See module docs: tests set author env vars here; [`super::handle_init`] does not.
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    /// Serializes tests that call [`std::env::set_var`] / [`std::env::remove_var`] for Git author overrides.
    ///
    /// The `terry` CLI never uses this; it exists only so `handle_init` integration tests can run
    /// `git commit` without assuming [`Git`](https://git-scm.com/) user config on the machine.
    static GIT_AUTHOR_ENV_LOCK: Mutex<()> = Mutex::new(());

    /// **Test-only:** saves `GIT_AUTHOR_*` / `GIT_COMMITTER_*`, sets fixed values for child `git` processes, restores on drop.
    ///
    /// Production [`handle_init`](super::handle_init) does **not** set these variables; users rely on normal Git identity.
    struct GitAuthorEnvForTest {
        author_name: Option<OsString>,
        author_email: Option<OsString>,
        committer_name: Option<OsString>,
        committer_email: Option<OsString>,
    }

    impl GitAuthorEnvForTest {
        fn set_test_authority() -> Self {
            let author_name = env::var_os("GIT_AUTHOR_NAME");
            let author_email = env::var_os("GIT_AUTHOR_EMAIL");
            let committer_name = env::var_os("GIT_COMMITTER_NAME");
            let committer_email = env::var_os("GIT_COMMITTER_EMAIL");
            // SAFETY: `GIT_AUTHOR_ENV_LOCK` is held by the caller so no concurrent `set_var` in tests using this guard.
            unsafe {
                env::set_var("GIT_AUTHOR_NAME", "terry-test");
                env::set_var("GIT_AUTHOR_EMAIL", "terry-test@example.com");
                env::set_var("GIT_COMMITTER_NAME", "terry-test");
                env::set_var("GIT_COMMITTER_EMAIL", "terry-test@example.com");
            }
            Self {
                author_name,
                author_email,
                committer_name,
                committer_email,
            }
        }
    }

    impl Drop for GitAuthorEnvForTest {
        fn drop(&mut self) {
            restore_os_env("GIT_AUTHOR_NAME", &self.author_name);
            restore_os_env("GIT_AUTHOR_EMAIL", &self.author_email);
            restore_os_env("GIT_COMMITTER_NAME", &self.committer_name);
            restore_os_env("GIT_COMMITTER_EMAIL", &self.committer_email);
        }
    }

    fn restore_os_env(key: &str, prev: &Option<OsString>) {
        // SAFETY: called only from `GitAuthorEnvForTest::drop` while `GIT_AUTHOR_ENV_LOCK` is held.
        unsafe {
            match prev {
                Some(v) => env::set_var(key, v),
                None => env::remove_var(key),
            }
        }
    }

    fn unique_temp_project_path() -> PathBuf {
        let n = TEST_DIR_SEQ.fetch_add(1, Ordering::SeqCst);
        env::temp_dir().join(format!(
            "terrance_project_test_{}_{}",
            std::process::id(),
            n
        ))
    }

    /// [`resolve_project_path`] returns a clone of `--path` when provided.
    #[test]
    fn resolve_project_path_returns_explicit_path() {
        let p = PathBuf::from("/tmp/example/project-name");
        assert_eq!(resolve_project_path(Some(&p)).unwrap(), p);
    }

    /// [`resolve_project_path`] uses the process current directory when `--path` is omitted.
    #[test]
    fn resolve_project_path_none_is_current_dir() {
        let cwd = env::current_dir().unwrap();
        assert_eq!(resolve_project_path(None).unwrap(), cwd);
    }

    /// [`handle_init`] rejects a `--repo-slug` that is empty after trimming (before loading GitHub config).
    #[test]
    fn handle_init_rejects_blank_repo_slug() {
        for slug in ["", " ", "  ", "\t\n"] {
            let s = slug.to_string();
            let err = handle_init("proj", None, Some(&s), false, true, None).unwrap_err();
            assert!(
                err.to_string().contains("--repo-slug"),
                "expected repo-slug error for slug={slug:?}, got {err}"
            );
        }
    }

    /// [`handle_init`] runs `mkdir -p`, `git init`, empty `README.md`, and an initial commit on `main` when no remote or planning options are set.
    ///
    /// **Test-only:** sets and restores process `GIT_AUTHOR_*` / `GIT_COMMITTER_*` around `handle_init`
    /// so the subprocess commit succeeds without the developer’s Git user config. The CLI does not do this.
    #[test]
    fn handle_init_creates_directory_and_git_repository() {
        use std::process::Command;

        let _env_lock = GIT_AUTHOR_ENV_LOCK
            .lock()
            .expect("GIT_AUTHOR env lock poisoned");
        let _git_author_env = GitAuthorEnvForTest::set_test_authority();

        let dir = unique_temp_project_path();
        let _ = fs::remove_dir_all(&dir);

        handle_init("functional-test", Some(&dir), None, false, true, None)
            .expect("handle_init should mkdir, git init, and create initial commit");

        assert!(dir.is_dir(), "project directory should exist");
        assert!(
            dir.join(".git").exists(),
            ".git should exist after git init"
        );

        let readme = dir.join("README.md");
        assert!(readme.is_file(), "README.md should exist");
        assert_eq!(fs::read_to_string(&readme).unwrap(), "");

        let dir_s = dir.to_str().expect("temp path utf-8");
        let head = Command::new("git")
            .args(["-C", dir_s, "rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .expect("git rev-parse");
        assert!(head.status.success(), "git rev-parse should succeed");
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "main");

        let log = Command::new("git")
            .args(["-C", dir_s, "log", "-1", "--format=%s"])
            .output()
            .expect("git log");
        assert!(log.status.success(), "git log should succeed");
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim(),
            INITIAL_COMMIT_MESSAGE
        );

        fs::remove_dir_all(&dir).expect("remove temp project dir");
    }

    /// Default init (agentic scaffolding) requires synced config.
    #[test]
    fn handle_init_without_config_errors_when_agentic_default() {
        let dir = unique_temp_project_path();
        let _ = fs::remove_dir_all(&dir);

        let err = handle_init("needs-config", Some(&dir), None, false, false, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("config sync") || msg.contains("templates"),
            "expected config sync or templates hint, got {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// [`handle_init`] with `--skip-agentic` and no language does not require config.
    #[test]
    fn handle_init_skip_agentic_without_language_skips_config() {
        use std::process::Command;

        let _env_lock = GIT_AUTHOR_ENV_LOCK
            .lock()
            .expect("GIT_AUTHOR env lock poisoned");
        let _git_author_env = GitAuthorEnvForTest::set_test_authority();

        let dir = unique_temp_project_path();
        let _ = fs::remove_dir_all(&dir);

        handle_init("local-only", Some(&dir), None, false, true, None)
            .expect("skip-agentic init should succeed without config");

        assert!(dir.join(".git").exists());

        let dir_s = dir.to_str().expect("temp path utf-8");
        let log = Command::new("git")
            .args(["-C", dir_s, "log", "--oneline"])
            .output()
            .expect("git log");
        assert!(log.status.success());
        let log_text = String::from_utf8_lossy(&log.stdout);
        assert!(
            !log_text.contains(SCAFFOLDING_COMMIT_MESSAGE),
            "stub scaffolding should not create a second commit"
        );

        fs::remove_dir_all(&dir).expect("remove temp project dir");
    }

    /// [`github_create_repo_cli_argv`] matches the `github create-repo --name <slug>` tail.
    #[test]
    fn github_create_repo_cli_argv_order_and_flags() {
        assert_eq!(
            github_create_repo_cli_argv("my-app"),
            vec![
                "github".to_string(),
                "create-repo".to_string(),
                "--name".to_string(),
                "my-app".to_string(),
            ]
        );
    }

    /// [`github_create_repo_step_command`] concatenates the executable path and argv tail with spaces.
    #[test]
    fn github_create_repo_step_command_joins_exe_and_argv() {
        assert_eq!(
            github_create_repo_step_command("/usr/local/bin/terry", "slug-1"),
            "/usr/local/bin/terry github create-repo --name slug-1"
        );
    }

    /// [`github_create_repo_step`] uses the step label shown in [`StepManager`] error messages.
    #[test]
    fn github_create_repo_step_label() {
        let step = github_create_repo_step("/tmp/terry", "z");
        assert_eq!(step.name(), "Create GitHub repository");
    }
}
