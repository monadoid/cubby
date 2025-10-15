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

# completely nuke all cubby services, permissions, and caches
nuke:
    @echo "🔥 nuking all cubby services and permissions..."
    @echo ""
    @echo "⚠️  this will:"
    @echo "  - stop and uninstall all cubby services"
    @echo "  - remove all plist files"
    @echo "  - kill any running cubby processes"
    @echo "  - reset screen recording & microphone permissions"
    @echo "  - clean cache and model files"
    @echo ""
    @echo "stopping cubby service..."
    -launchctl bootout gui/$(id -u)/com.tabsandtabs.cubby 2>/dev/null || true
    @echo "stopping cloudflared service..."
    -launchctl bootout gui/$(id -u)/com.example.cubby.cloudflared 2>/dev/null || true
    -launchctl bootout gui/$(id -u)/com.example.cubby.screenpipe 2>/dev/null || true
    @echo "removing plist files..."
    -rm -f ~/Library/LaunchAgents/com.tabsandtabs.cubby.plist
    -rm -f ~/Library/LaunchAgents/com.example.cubby.cloudflared.plist
    -rm -f ~/Library/LaunchAgents/com.example.cubby.screenpipe.plist
    @echo "killing any stray cubby processes..."
    -pkill -9 -f "cubby.*--no-service" || true
    -pkill -9 -f cloudflared || true
    @echo "cleaning cache and model files..."
    -rm -rf ~/.cubby/*.log
    -rm -rf ~/Library/Caches/cubby/models/*.onnx

# show available commands
help:
    @just --list

