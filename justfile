# Scryforge Justfile
# Commands for building, running, and testing the Scryforge information stream aggregator

# Default recipe - show available commands
default:
    @just --list

# ========================================
# Build Commands
# ========================================

# Build all workspace crates
build:
    cargo build

# Build with release optimizations
build-release:
    cargo build --release

# Build the daemon only
build-daemon:
    cargo build -p scryforge-daemon

# Build the TUI client only
build-tui:
    cargo build -p scryforge-tui

# Clean build artifacts
clean:
    cargo clean

# ========================================
# Running
# ========================================

# Run both daemon and TUI in parallel (main development command)
run: daemon tui

# Run the daemon
daemon:
    #!/usr/bin/env bash
    echo "Starting Scryforge daemon..."
    cargo run -p scryforge-daemon

# Run the TUI client
tui:
    #!/usr/bin/env bash
    # Wait a moment for daemon to start
    sleep 1
    echo "Starting Scryforge TUI..."
    cargo run -p scryforge-tui

# Run daemon in background, then TUI (foreground)
run-bg:
    #!/usr/bin/env bash
    echo "Starting daemon in background..."
    cargo run -p scryforge-daemon > /tmp/scryforge-daemon.log 2>&1 &
    DAEMON_PID=$!
    echo "Daemon PID: $DAEMON_PID"

    sleep 2

    echo "Starting TUI..."
    cargo run -p scryforge-tui

    # Kill daemon when TUI exits
    echo "Stopping daemon..."
    kill $DAEMON_PID 2>/dev/null || true

# Run daemon and TUI in separate tmux panes
dev:
    #!/usr/bin/env bash
    if ! command -v tmux &> /dev/null; then
        echo "Error: tmux not found. Install tmux or use 'just run-bg' instead."
        exit 1
    fi

    SESSION="scryforge-dev"

    if tmux has-session -t $SESSION 2>/dev/null; then
        echo "Attaching to existing session: $SESSION"
        tmux attach-session -t $SESSION
    else
        echo "Creating new tmux session: $SESSION"
        # Create session with daemon in first pane
        tmux new-session -d -s $SESSION -n "scryforge" "cargo run -p scryforge-daemon"
        # Split window and run TUI in second pane
        tmux split-window -h -t $SESSION "sleep 2 && cargo run -p scryforge-tui"
        # Select layout and attach
        tmux select-layout -t $SESSION even-horizontal
        tmux attach-session -t $SESSION
    fi

# Run daemon + TUI using existing release build (no rebuild)
run-release:
    #!/usr/bin/env bash
    set -euo pipefail

    BIN_DIR="${CARGO_TARGET_DIR:-target}/release"
    BIN_DIR="${BIN_DIR/#\~/$HOME}"

    if [ ! -x "$BIN_DIR/scryforge-daemon" ] || [ ! -x "$BIN_DIR/scryforge-tui" ]; then
        echo "Release binaries not found in $BIN_DIR. Build them first with: just build-release"
        exit 1
    fi

    echo "Starting daemon (release)..."
    "$BIN_DIR/scryforge-daemon" > /tmp/scryforge-daemon.log 2>&1 &
    DAEMON_PID=$!

    sleep 2

    echo "Starting TUI (release)..."
    "$BIN_DIR/scryforge-tui"

    echo "Stopping daemon..."
    kill $DAEMON_PID 2>/dev/null || true

# Clean, rebuild from scratch, then run daemon + TUI (release binaries)
fresh-run:
    #!/usr/bin/env bash
    set -euo pipefail

    BIN_DIR="${CARGO_TARGET_DIR:-target}/release"
    BIN_DIR="${BIN_DIR/#\~/$HOME}"

    echo "Cleaning build..."
    pkill -f scryforge-daemon 2>/dev/null || true
    pkill -f scryforge-tui 2>/dev/null || true
    cargo clean

    echo "Building release binaries..."
    cargo build --release -p scryforge-daemon -p scryforge-tui

    if [ ! -x "$BIN_DIR/scryforge-daemon" ] || [ ! -x "$BIN_DIR/scryforge-tui" ]; then
        echo "Release binaries not found in $BIN_DIR after build."
        exit 1
    fi

    echo "Starting daemon (release)..."
    "$BIN_DIR/scryforge-daemon" > /tmp/scryforge-daemon.log 2>&1 &
    DAEMON_PID=$!

    sleep 2

    echo "Starting TUI (release)..."
    "$BIN_DIR/scryforge-tui"

    echo "Stopping daemon..."
    kill $DAEMON_PID 2>/dev/null || true

# ========================================
# Testing
# ========================================

# Check all crates for errors
check:
    cargo check --workspace

# Run tests for all crates
test:
    cargo test --workspace

# Run tests with output
test-verbose:
    cargo test --workspace -- --nocapture

# Run tests for a specific crate
test-crate CRATE:
    cargo test -p {{CRATE}}

# Run daemon tests
test-daemon:
    cargo test -p scryforge-daemon

# Run TUI tests
test-tui:
    cargo test -p scryforge-tui

# Run core library tests
test-core:
    cargo test -p fusabi-streams-core
    cargo test -p fusabi-tui-core
    cargo test -p fusabi-tui-widgets

# Run provider tests
test-providers:
    cargo test -p provider-dummy

# ========================================
# Code Quality
# ========================================

# Format code
fmt:
    cargo fmt --all

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Run clippy lints
clippy:
    cargo clippy --workspace -- -D warnings

# Run clippy with fixes
clippy-fix:
    cargo clippy --workspace --fix --allow-dirty --allow-staged

# Run all quality checks (format, clippy, test)
ci: fmt-check clippy test

# Quick iteration: check + test
quick: check test

# ========================================
# Documentation
# ========================================

# Build Rust API documentation
doc:
    cargo doc --workspace --no-deps --open

# Build documentation including private items
doc-private:
    cargo doc --workspace --no-deps --document-private-items --open

# ========================================
# Dependencies
# ========================================

# Check dependency tree
tree:
    cargo tree

# Update dependencies
update:
    cargo update

# Audit dependencies for security issues
audit:
    cargo audit

# ========================================
# Provider Development
# ========================================

# Build all providers
build-providers:
    cargo build -p provider-dummy

# Test all providers
test-providers-all:
    cargo test -p provider-dummy

# ========================================
# Utility Commands
# ========================================

# Kill any running scryforge processes
kill:
    #!/usr/bin/env bash
    echo "Killing scryforge processes..."
    pkill -f scryforge-daemon || true
    pkill -f scryforge-tui || true
    echo "Done"

# Full cleanup (kill processes + cargo clean)
nuke: kill clean

# Show build status
status:
    @echo "Scryforge Build Status"
    @echo "======================"
    @cargo --version
    @rustc --version
    @echo ""
    @echo "Workspace crates:"
    @cargo metadata --no-deps --format-version 1 | grep -o '"name":"[^"]*"' | cut -d'"' -f4

# Build specific crate
build-crate CRATE:
    cargo build -p {{CRATE}}

# Run specific crate
run-crate CRATE:
    cargo run -p {{CRATE}}

# Watch and rebuild on changes (requires cargo-watch)
watch:
    cargo watch -x check -x test

# Install cargo-watch if not present
install-watch:
    cargo install cargo-watch

# Install cargo-audit if not present
install-audit:
    cargo install cargo-audit

# Full rebuild from scratch
rebuild: clean build

# Run benchmarks
bench:
    cargo bench --workspace

# ========================================
# Aliases
# ========================================

# Alias for build-release
alias br := build-release

# Alias for test
alias t := test

# Alias for fmt
alias f := fmt

# Alias for clippy
alias l := clippy

# Alias for check
alias c := check

# Alias for doc
alias d := doc
