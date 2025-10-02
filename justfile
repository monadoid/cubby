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

dev-server:
    cd apps/cubby-server && pnpm dev

dev-example:
    cd apps/exampleco_website && pnpm dev

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
    pnpm -w run format || true

# Clean build artifacts
clean:
    cargo clean
    pnpm -w run clean || true
    rm -rf apps/*/dist apps/*/node_modules/.cache

# Show available commands
help:
    @just --list