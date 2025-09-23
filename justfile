# Load environment variables from cubby-api .env file
set dotenv-path := "apps/cubby-api/.env"

# Build all projects
build:
    cargo build --workspace
    pnpm -w build

# Test all projects  
test:
    cargo test --workspace
    pnpm -w test

# Generate types and schemas
gen:
    pnpm -w gen

# Run development servers
dev-api:
    cd apps/cubby-api && cargo loco start

dev-worker:
    cd apps/exampleCoWebsite && pnpm dev

# Run Bruno tests against already running server
bruno-test:
    cd apps/cubby-api/bruno && bru run . -r --env local

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