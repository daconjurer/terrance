# Terry (Terrance)

A CLI tool for managing and configuring development environments. My cat's name is
Terry, wouldn't it be cool if he could do all these things for me? You know, as pals.

## System dependencies

`terry` targets **macOS and Linux** only. It expects these tools on your PATH for
full workflows:

- **git**
- **GitHub CLI** (`gh`)
- **1Password CLI** (`op`)
- **Rust** (`cargo`) - for building `terry` from source

### Install dependencies with Just

Install [Just](https://github.com/casey/just) first:

```bash
# macOS
brew install just

# Linux
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

## Installation

```bash
cargo install --path .
```

## Usage

### Project Commands

#### Initialize a new project

Initialize a git repository for a new project with optional remote and planning submodule:

```bash
# Initialize in current directory with remote
terry project init --name my-project --remote https://github.com/user/repo.git

# Initialize in specific directory
terry project init --name my-project --path /path/to/project

# Initialize without remote (will be prompted)
terry project init --name my-project

# Initialize with planning submodule
terry project init --name my-project --with-planning
# Will prompt for: Git remote URL (optional) and Planning repository URL (required)
```

The `init` command will:
1. Initialize a git repository at the specified path (or current directory)
2. Optionally add a remote origin (if provided or entered at prompt)
3. Optionally add a planning directory as a git submodule (if --with-planning flag is used)

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
