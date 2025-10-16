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

# Use bash for nicer scripting
set shell := ["bash", "-euo", "pipefail", "-c"]

# Default version: latest tag (without the v). Fallback to 0.0.0-local
VERSION := `git tag --sort=-version:refname | head -1 | sed 's/^v//' || echo 0.0.0-local`

# Compute macOS SDK path once
SDKROOT := `xcrun --sdk macosx --show-sdk-path`

# Helpful: print the version this run will use
@info:
	echo "Version: {{VERSION}}"


# ----- Internal helpers ------------------------------------------------------

# Ensure rust targets exist
ensure-rust-targets:
	rustup target add aarch64-apple-darwin
	rustup target add x86_64-apple-darwin

# Deduce Homebrew ffmpeg pkg-config paths (handles both arm64 / intel prefixes)
# If you don't use Homebrew/ffmpeg locally, you can remove PKG_CONFIG_PATH exports below.
brew-pkgconfig-path:
	# Try both common locations; ignore if missing
	ARM_OPT="/opt/homebrew/opt/ffmpeg/lib/pkgconfig"
	INTEL_OPT="/usr/local/opt/ffmpeg/lib/pkgconfig"
	if [[ -d "$ARM_OPT" ]]; then echo -n "$ARM_OPT:"; fi
	if [[ -d "$INTEL_OPT" ]]; then echo -n "$INTEL_OPT:"; fi

# ----- Build recipes ---------------------------------------------------------

# Build arm64 (Apple Silicon) first
build-arm64: ensure-rust-targets
	# Stop ggml from probing host; target a safe arch; keep metal feature
	# Note: whisper-rs-sys will pick these up through CMAKE_ARGS
	export SDKROOT="{{SDKROOT}}" && \
	export MACOSX_DEPLOYMENT_TARGET="11.0" && \
	export CMAKE_ARGS="-DGGML_NATIVE=OFF -DGGML_CPU_ARM_MATMUL_INT8=OFF -DCMAKE_OSX_ARCHITECTURES=arm64 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0" && \
	export PKG_CONFIG_PATH="/opt/homebrew/opt/ffmpeg/lib/pkgconfig:/usr/local/opt/ffmpeg/lib/pkgconfig:${PKG_CONFIG_PATH:-}" && \
	cargo build --release --features metal --target aarch64-apple-darwin

# Then build x86_64 (Intel) via cross-compile from Apple Silicon
build-x86_64: ensure-rust-targets
	export SDKROOT="{{SDKROOT}}" && \
	export MACOSX_DEPLOYMENT_TARGET="11.0" && \
	export CMAKE_ARGS="-DGGML_NATIVE=OFF -DGGML_CPU_ARM_MATMUL_INT8=OFF -DCMAKE_OSX_ARCHITECTURES=x86_64 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0" && \
	export PKG_CONFIG_PATH="/opt/homebrew/opt/ffmpeg/lib/pkgconfig:/usr/local/opt/ffmpeg/lib/pkgconfig:${PKG_CONFIG_PATH:-}" && \
	cargo build --release --features metal --target x86_64-apple-darwin

# Package both builds into dist/, produce checksums
package:
	mkdir -p "dist/{{VERSION}}/aarch64-apple-darwin/bin" "dist/{{VERSION}}/x86_64-apple-darwin/bin"
	cp "target/aarch64-apple-darwin/release/cubby" "dist/{{VERSION}}/aarch64-apple-darwin/bin/"
	cp "target/x86_64-apple-darwin/release/cubby" "dist/{{VERSION}}/x86_64-apple-darwin/bin/"
	( cd "dist/{{VERSION}}/aarch64-apple-darwin" && tar -czf "../../cubby-{{VERSION}}-aarch64-apple-darwin.tar.gz" . )
	( cd "dist/{{VERSION}}/x86_64-apple-darwin" && tar -czf "../../cubby-{{VERSION}}-x86_64-apple-darwin.tar.gz" . )
	( cd dist && shasum -a 256 cubby-{{VERSION}}-*.tar.gz | tee "cubby-{{VERSION}}-SHA256SUMS.txt" )


# ----- Top-level: what you asked for ----------------------------------------

# Build both architectures sequentially, then package
release: info build-arm64 build-x86_64 package
	@echo "Release artifacts are in ./dist/{{VERSION}} and ./dist/"

# Release with git tagging and pushing
release-tag VERSION:
	@echo "🏗️  building release artifacts..."
	just release
	@echo "🏷️  creating tag v{{VERSION}}..."
	git tag v{{VERSION}}
	@echo "📤 pushing tag v{{VERSION}}..."
	git push origin v{{VERSION}}
	@echo "📦 creating github release and uploading macOS binaries..."
	gh release create v{{VERSION}} --title {{VERSION}} --generate-notes \
		dist/cubby-{{VERSION}}-aarch64-apple-darwin.tar.gz \
		dist/cubby-{{VERSION}}-x86_64-apple-darwin.tar.gz
	@echo "✅ release v{{VERSION}} complete! workflow will upload to r2 automatically"

