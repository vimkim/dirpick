# List available commands.
default:
    @just --list

# Install this checkout into ~/.local/bin and verify the command.
sync:
    #!/usr/bin/env bash
    set -eu
    if ! command -v cargo >/dev/null 2>&1; then
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
    cargo install --locked --path . --root "$HOME/.local" --bin dirpick
    "$HOME/.local/bin/dirpick" --version
