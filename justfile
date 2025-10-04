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

# Lint and format all code
lint:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check
    pnpm -w lint

# Format code
fmt:
    cargo fmt
    pnpm -w run format

fmt-check:
    cargo fmt --all -- --check
    pnpm -w run format:check

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
