# Terry (Terrance)

[![pre-tests](https://github.com/daconjurer/terrance/actions/workflows/pre-tests.yaml/badge.svg)](https://github.com/daconjurer/terrance/actions/workflows/pre-tests.yaml)

A CLI tool for managing and configuring development environments. My cat's name is
Terry, wouldn't it be cool if he could do all these things for me? You know, as pals.

## System dependencies

`terry` targets **macOS and Linux** only. It expects these tools on your PATH for
full workflows:

- **git** — `terry project init` and other git-backed steps
- **GitHub CLI** (`gh`) — **`terry github`** (`gh repo create`, `gh repo view`; Terry sets `GH_TOKEN` from `terry config sync`). Not installed by Cargo; use Just or [install `gh`](https://cli.github.com/) yourself
- **1Password CLI** (`op`) — `terry config sync`
- **Rust** (`cargo`) — build `terry` from source (checked by `just verify`, not installed by the Justfile)

The `just install-deps` recipe installs **git**, **`gh`**, and **`op`** where they are missing (macOS via Homebrew; Linux via `apt-get`).

### Install dependencies with Just

Install [Just](https://github.com/casey/just) first:

```bash
# macOS
brew install just

# Linux (needs super user permissions if this target folder is used)
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin
```

Then install the system tools terry integrates with:

```bash
just install-deps
```

Check everything is present:

```bash
just verify
```

List all recipes (default target):

```bash
just
```

## Continuous integration

GitHub Actions runs **`cargo fmt --check`**, **`cargo clippy -- -D warnings`**, and **`cargo test`** on pushes to `main` and on pull requests (workflow: [pre-tests.yaml](.github/workflows/pre-tests.yaml)). The job uses the [Docker-based action](.github/actions/pre-test/) defined in this repository so the toolchain matches CI.

## Installation

```bash
cargo install --path .
```

## Getting started

Prepare your chosen **1Password vault** so `terry config sync` can find GitHub credentials. Terry expects **one login item** in that vault:

| What | Value |
|------|--------|
| Item title | **`Github`** (must match exactly) |
| `username` | Your GitHub username |
| `token` | Fine-grained PAT — **Metadata** read-only and **Contents** read-only (`gh repo view`, read workflows) |
| `token_write` | Fine-grained PAT — **Metadata** read-only and **Administration** read/write (`gh repo create`) |

Use **concealed** fields for both **`token`** and **`token_write`**. Field **labels** must match the names above.

Then run **`terry config sync`** before other Terry commands so settings and tokens (for example `GH_TOKEN` for GitHub via `gh`) are loaded from 1Password. Use your real vault name:

```bash
terry config sync --vault "Your Vault"
```

If configuration already exists, pass **`--force`** to overwrite it from the vault.


## Usage

### Project Commands

#### Initialize a new project

Initialize a git repository for a new project with optional GitHub-backed `origin` and planning submodule:

```bash
# Initialize in current directory; set origin from synced GitHub user + slug, create private repo on GitHub via gh
terry project init --name my-project --repo-slug my-project

# Initialize in a specific directory
terry project init --name my-project --path /path/to/project

# Local repository only (no origin, no gh)
terry project init --name my-project

# Initialize with planning submodule
terry project init --name my-project --with-planning
# Will prompt for Planning repository URL when --with-planning is set
```

The `init` command will:

1. Create the project directory if needed, initialize a Git repository, set the default branch to `main`, add an empty `README.md`, and create an initial commit with message **`initial commit`** (so `main` is never an empty branch).
2. Optionally add `origin` as `git@github.com:<synced_user>/<repo-slug>.git` and create that private GitHub repository when `--repo-slug` is passed (requires `terry config sync` and `token_write`).
3. Optionally add a planning directory as a git submodule (if `--with-planning` is used).
4. When `origin` was configured in step 2, push `main` to the remote after any GitHub repo creation.

**Git author:** Terry does **not** set `git config user.name` / `user.email` or `GIT_AUTHOR_*` / `GIT_COMMITTER_*` for that first commit. Use your normal Git configuration so `git commit` can run. In this repository, **unit tests only** set author environment variables (in the test process or on a [`Step`](src/steps.rs)) so `cargo test` succeeds without a globally configured Git user.

## Development

Build the project:

```bash
cargo build
```

Run the project:

```bash
cargo run -- --help
```

Run tests:

```bash
cargo test
```

Run examples:

```bash
cargo run --example step_manager_example
```

## Architecture

The project uses:
- **Clap**: Command-line argument parsing with derive macros
- **thiserror**: Custom error types with descriptive messages
- **Step System**: Modular command execution with sequential dependency
  - `Step`: Individual command with template variables
  - `StepManager`: Orchestrates sequential step execution with early abort on failure

### Step System

Commands are broken into discrete steps that execute sequentially. If any step fails, execution stops immediately and reports which step failed.

See `examples/step_manager_example.rs` for usage examples.
