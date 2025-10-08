# Build all projects
build:
    cargo build --workspace
    pnpm -w build

# Test all projects  
test:
    cargo test --workspace
    pnpm -w test

# Run development servers
cubby-start:
    cd apps/cubby && cargo run -- start

cubby-uninstall:
    cd apps/cubby && cargo run -- uninstall

server-dev:
    cd apps/cubby-server && pnpm dev

server-typecheck:
    cd apps/cubby-server && pnpm type-check

server-types:
    cd apps/cubby-server && pnpm cf-typegen

example-dev:
    cd apps/exampleco_website && pnpm dev

get-cubby-dev:
    cd apps/cubby-installer && pnpm dev

# Install dependencies
install:
    pnpm install

# Combined code checks for Rust and TypeScript
check:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    pnpm -w typecheck
    pnpm -w run format:check

# Apply automatic fixes across Rust and TypeScript codebases
fix:
    cargo fmt --all
    pnpm -w run format

# Clean build artifacts
clean:
    cargo clean
    pnpm -w run clean || true
    rm -rf apps/*/dist apps/*/node_modules/.cache

# Check screenpipe database status
check-db:
    ./check_screenpipe_db.sh

# Show available commands
help:
    @just --list
