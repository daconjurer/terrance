# Terry (Terrance) — install and verify system dependencies via Just
# https://just.systems/man/en/
#
# Dependency set: git, GitHub CLI (gh), 1Password CLI (op).
#   gh — required for `terry github` (subprocess to `gh repo create` / `gh repo view`).
#   cargo is not installed here; `just verify` checks it if present.

# Default recipe (shows help)
default:
    @just --list

# Install all system dependencies (macOS and Linux only): git, gh, op
[unix]
install-deps: install-git install-gh install-1password-cli
    @echo "✓ All dependencies installed (git, gh, op)"

[macos]
install-git:
    @just _install-git-macos

[linux]
install-git:
    @just _install-git-linux

[macos]
install-gh:
    @just _install-gh-macos

[linux]
install-gh:
    @just _install-gh-linux

[macos]
install-1password-cli:
    @just _install-op-macos

[linux]
install-1password-cli:
    @just _install-op-linux

# --- git --------------------------------------------------------------------

[macos]
_install-git-macos:
    #!/usr/bin/env sh
    echo "Checking git installation..."
    if command -v git >/dev/null 2>&1; then
        echo "✓ git already installed: $(git --version)"
    else
        brew install git
    fi

[linux]
_install-git-linux:
    #!/usr/bin/env sh
    echo "Checking git installation..."
    if command -v git >/dev/null 2>&1; then
        echo "✓ git already installed: $(git --version)"
    else
        sudo apt-get update && sudo apt-get install -y git
    fi

# --- GitHub CLI -------------------------------------------------------------

[macos]
_install-gh-macos:
    #!/usr/bin/env sh
    echo "Installing GitHub CLI..."
    if command -v gh >/dev/null 2>&1; then
        echo "✓ GitHub CLI already installed: $(gh --version | head -n1)"
    else
        brew install gh
    fi

[linux]
_install-gh-linux:
    #!/usr/bin/env sh
    echo "Installing GitHub CLI..."
    if command -v gh >/dev/null 2>&1; then
        echo "✓ GitHub CLI already installed: $(gh --version | head -n1)"
    else
        sudo apt-get update && sudo apt-get install -y gh
    fi

# --- 1Password CLI ----------------------------------------------------------

[macos]
_install-op-macos:
    #!/usr/bin/env sh
    echo "Installing 1Password CLI..."
    if command -v op >/dev/null 2>&1; then
        echo "✓ 1Password CLI already installed: $(op --version)"
    else
        brew update && brew install --cask 1password-cli
    fi

[linux]
_install-op-linux:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Installing 1Password CLI..."
    if command -v op >/dev/null 2>&1; then
        echo "✓ 1Password CLI already installed: $(op --version)"
    else
        curl -sS https://downloads.1password.com/linux/keys/1password.asc \
            | sudo gpg --dearmor --output /usr/share/keyrings/1password-archive-keyring.gpg
        echo 'deb [arch=amd64 signed-by=/usr/share/keyrings/1password-archive-keyring.gpg] https://downloads.1password.com/linux/debian/amd64 stable main' \
            | sudo tee /etc/apt/sources.list.d/1password.list
        sudo apt-get update
        sudo apt-get install -y 1password-cli
    fi

# Verify all dependencies are installed
[unix]
verify:
    #!/usr/bin/env sh
    echo "Verifying dependencies..."
    if command -v git >/dev/null 2>&1; then
        echo "✓ git: $(git --version)"
    else
        echo "❌ git not found"
    fi
    if command -v gh >/dev/null 2>&1; then
        echo "✓ gh: $(gh --version | head -n1)"
    else
        echo "❌ gh not found"
    fi
    if command -v op >/dev/null 2>&1; then
        echo "✓ op: $(op --version)"
    else
        echo "❌ op not found"
    fi
    if command -v cargo >/dev/null 2>&1; then
        echo "✓ cargo: $(cargo --version)"
    else
        echo "❌ cargo not found"
    fi

# Update all dependencies (macOS and Linux only; best-effort)
[unix]
update-deps: update-git update-gh update-1password-cli
    @echo "✓ All dependencies updated"

[macos]
update-git:
    @brew upgrade git 2>/dev/null || true

[linux]
update-git:
    @sudo apt-get update && sudo apt-get install -y --only-upgrade git || true

[macos]
update-gh:
    @brew upgrade gh 2>/dev/null || true

[linux]
update-gh:
    @sudo apt-get update && sudo apt-get install -y --only-upgrade gh || true

[macos]
update-1password-cli:
    #!/usr/bin/env sh
    brew update
    brew upgrade --cask 1password-cli 2>/dev/null || true

[linux]
update-1password-cli:
    @sudo apt-get update && sudo apt-get install -y --only-upgrade 1password-cli || true
