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

profile-flamegraph:
    DATA_DIR="${CUBBY_DEV_DATA_DIR:-$PWD/.cubby-dev}" && \
    mkdir -p "$DATA_DIR" && \
    CARGO_PROFILE_RELEASE_DEBUG=true \
    CARGO_PROFILE_RELEASE_STRIP=false \
    CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO=packed \
    SAVE_RESOURCE_USAGE=${SAVE_RESOURCE_USAGE:-1} \
    cargo flamegraph --release --bin cubby -- \
        service \
        --data-dir "$DATA_DIR" \
        --debug \
        --disable-telemetry \
        --port "${CUBBY_DEV_PORT:-43030}" \
        --audio-transcription-engine speech-analyzer \
        --enable-realtime-audio-transcription

profile-instruments template="Allocations":
    #!/usr/bin/env bash
    DATA_DIR="${CUBBY_DEV_DATA_DIR:-$PWD/.cubby-dev}"; mkdir -p "$DATA_DIR"
    OUTPUT_DIR="${CUBBY_INSTRUMENTS_DIR:-target/instruments}"
    mkdir -p "$OUTPUT_DIR"
    CMD=(cargo instruments -t "$template" --release --bin cubby)
    if [[ "${INSTRUMENTS_NO_OPEN:-0}" == "1" ]]; then
        CMD+=(--no-open)
    fi
    if [[ -n "${INSTRUMENTS_TIME_LIMIT:-}" ]]; then
        CMD+=(--time-limit "${INSTRUMENTS_TIME_LIMIT}")
    fi
    if [[ -n "${INSTRUMENTS_OUTPUT:-}" ]]; then
        CMD+=(-o "${INSTRUMENTS_OUTPUT}")
    fi
    CMD+=(--)
    CMD+=(
        --data-dir "$DATA_DIR"
        --debug
        --disable-telemetry
        --port "${CUBBY_DEV_PORT:-43030}"
        --audio-transcription-engine speech-analyzer
        --enable-realtime-audio-transcription
    )
    echo "Running: ${CMD[*]}"
    CARGO_PROFILE_RELEASE_DEBUG=true \
    CARGO_PROFILE_RELEASE_STRIP=false \
    CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO=packed \
        "${CMD[@]}"
# build notify-helper for both macOS architectures and copy to sidecars
build-notify-helper: ensure-rust-targets
    @echo "building notify-helper for aarch64..."
    cargo build --release --package notify-helper --target aarch64-apple-darwin
    @echo "building notify-helper for x86_64..."
    cargo build --release --package notify-helper --target x86_64-apple-darwin
    @echo "copying binaries to sidecars..."
    cp target/aarch64-apple-darwin/release/notify-helper cubby-server/sidecars/macos-aarch64/notify-helper
    cp target/x86_64-apple-darwin/release/notify-helper cubby-server/sidecars/macos-x86_64/notify-helper
    @echo "✅ notify-helper binaries updated in cubby-server/sidecars/"

api-dev:
    cd cubby-api && pnpm dev

api-typecheck:
    cd cubby-api && pnpm type-check

api-types:
    cd cubby-api && pnpm cf-typegen

api-migrate:
    cd cubby-api && pnpm db:migrate

frontend-build-css:
    cd cubby-frontend && ./tailwindcss -i public/input.css -o public/output.css

frontend-dev:
    cd cubby-frontend && pnpm dev

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

# get dev token for email (legacy - use update-credentials instead)
token EMAIL:
    ./test-dev-token.sh {{EMAIL}}

# generate M2M credentials and update all example .env files
update-credentials EMAIL PASSWORD="example_pw!":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "generating m2m credentials for {{EMAIL}}..."
    CREDS=$(API_URL="${API_URL:-http://localhost:8787}" ./generate-m2m-credentials.sh {{EMAIL}} {{PASSWORD}})
    CLIENT_ID=$(echo "$CREDS" | cut -d'|' -f1)
    CLIENT_SECRET=$(echo "$CREDS" | cut -d'|' -f2)
    echo ""
    echo "updating credentials in example .env files..."
    # update or create .env files in all examples
    for dir in cubby-js/examples/*/; do
        if [ -d "$dir" ] && [ ! -d "$dir/node_modules" ] || [ -f "$dir/package.json" ]; then
            env_file="${dir}.env"
            echo "  updating $env_file..."
            # create or update .env file
            if [ -f "$env_file" ]; then
                # update existing file
                if grep -q "CUBBY_CLIENT_ID" "$env_file"; then
                    sed -i.bak "s/CUBBY_CLIENT_ID=.*/CUBBY_CLIENT_ID=$CLIENT_ID/" "$env_file"
                else
                    echo "CUBBY_CLIENT_ID=$CLIENT_ID" >> "$env_file"
                fi
                if grep -q "CUBBY_CLIENT_SECRET" "$env_file"; then
                    sed -i.bak "s/CUBBY_CLIENT_SECRET=.*/CUBBY_CLIENT_SECRET=$CLIENT_SECRET/" "$env_file"
                else
                    echo "CUBBY_CLIENT_SECRET=$CLIENT_SECRET" >> "$env_file"
                fi
                # ensure base url is set
                if ! grep -q "CUBBY_API_BASE_URL" "$env_file"; then
                    echo "CUBBY_API_BASE_URL=http://localhost:8787" >> "$env_file"
                fi
                # remove old token if it exists
                sed -i.bak '/CUBBY_API_TOKEN/d' "$env_file"
                rm -f "${env_file}.bak"
            else
                # create new file
                cat > "$env_file" << EOF
    CUBBY_API_BASE_URL=http://localhost:8787
    CUBBY_CLIENT_ID=$CLIENT_ID
    CUBBY_CLIENT_SECRET=$CLIENT_SECRET
    EOF
            fi
        fi
    done
    echo ""
    echo "✅ m2m credentials updated in all example .env files"
    echo "   client_id: $CLIENT_ID"
    echo "   client_secret: ${CLIENT_SECRET:0:20}..."

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

# Package both builds into dist/, produce checksums, and prepare for R2 upload
package:
	mkdir -p "dist/{{VERSION}}/aarch64-apple-darwin/bin" "dist/{{VERSION}}/x86_64-apple-darwin/bin" "dist/{{VERSION}}/r2-ready"
	cp "target/aarch64-apple-darwin/release/cubby" "dist/{{VERSION}}/aarch64-apple-darwin/bin/"
	cp "target/x86_64-apple-darwin/release/cubby" "dist/{{VERSION}}/x86_64-apple-darwin/bin/"
	( cd "dist/{{VERSION}}/aarch64-apple-darwin" && tar -czf "../../cubby-{{VERSION}}-aarch64-apple-darwin.tar.gz" . )
	( cd "dist/{{VERSION}}/x86_64-apple-darwin" && tar -czf "../../cubby-{{VERSION}}-x86_64-apple-darwin.tar.gz" . )
	( cd dist && shasum -a 256 cubby-{{VERSION}}-*.tar.gz | tee "cubby-{{VERSION}}-SHA256SUMS.txt" )
	@echo ""
	@echo "preparing binaries for r2 upload..."
	cp "target/aarch64-apple-darwin/release/cubby" "dist/{{VERSION}}/r2-ready/cubby-darwin-aarch64"
	cp "target/x86_64-apple-darwin/release/cubby" "dist/{{VERSION}}/r2-ready/cubby-darwin-x86_64"
	@echo ""
	@echo "📦 r2-ready binaries are in: dist/{{VERSION}}/r2-ready/"
	@echo "   - cubby-darwin-aarch64"
	@echo "   - cubby-darwin-x86_64"


# ----- Top-level: what you asked for ----------------------------------------

# Build both architectures sequentially, then package
release: info build-arm64 build-x86_64 package
	@echo "Release artifacts are in ./dist/{{VERSION}} and ./dist/"

# Release with git tagging and pushing
release-tag NEW_VERSION:
	@echo "🏷️  creating tag v{{NEW_VERSION}}..."
	git tag v{{NEW_VERSION}}
	@echo "🏗️  building release artifacts for v{{NEW_VERSION}}..."
	just VERSION={{NEW_VERSION}} release
	@echo "📤 pushing tag v{{NEW_VERSION}}..."
	git push origin v{{NEW_VERSION}}
	@echo "📦 creating github release and uploading macOS binaries..."
	gh release create v{{NEW_VERSION}} --title {{NEW_VERSION}} --generate-notes \
		dist/cubby-{{NEW_VERSION}}-aarch64-apple-darwin.tar.gz \
		dist/cubby-{{NEW_VERSION}}-x86_64-apple-darwin.tar.gz
	@echo ""
	@echo "✅ release v{{NEW_VERSION}} complete!"
	@echo ""
	@echo "📤 next steps:"
	@echo "   1. drag and drop these files to r2://cubby-releases/latest/"
	@echo "      • dist/{{NEW_VERSION}}/r2-ready/cubby-darwin-aarch64"
	@echo "      • dist/{{NEW_VERSION}}/r2-ready/cubby-darwin-x86_64"
	@echo "   2. linux binaries will be uploaded automatically by github actions"
