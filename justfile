# build all projects
build:
    cargo build --workspace
    pnpm -r build || true

# test all projects  
test:
    cargo test --workspace

# run development servers
cubby-start:
    cargo run -- start

cubby-uninstall:
    cargo run -- uninstall

api-dev:
    cd cubby-api && pnpm dev

api-typecheck:
    cd cubby-api && pnpm type-check

api-types:
    cd cubby-api && pnpm cf-typegen

api-migrate:
    cd cubby-api && pnpm db:migrate

example-dev:
    cd exampleco-website && pnpm dev

get-cubby-dev:
    cd cubby-installer && pnpm dev

# install dependencies
install:
    pnpm install

# combined code checks for rust and typescript
check:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    pnpm typecheck || true
    pnpm format:check || true

# apply automatic fixes across rust and typescript codebases
fix:
    cargo fmt --all
    pnpm format || true

# clean build artifacts
clean:
    cargo clean
    rm -rf **/dist **/node_modules/.cache cubby-api/node_modules cubby-installer/node_modules exampleco-website/node_modules

# show available commands
help:
    @just --list

