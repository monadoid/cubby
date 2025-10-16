#!/bin/bash

# Service Manager Integration Test Script
# Tests the cubby service manager functionality

set -e  # Exit on any error

echo "🧪 Testing Service Manager Integration"
echo "======================================"

# Configuration
BINARY="./target/release/cubby"
DEBUG_BINARY="./target/debug/cubby"
SERVICE_LABEL="com.tabsandtabs.cubby"
HEALTH_PORT=3030
TEST_PORT=3031

# Check for debug mode
DEBUG_MODE=false
if [[ "$1" == "--debug" ]]; then
    DEBUG_MODE=true
    log_info "Debug mode enabled - will show detailed output"
fi

# Cross-platform timeout function
run_with_timeout() {
    local timeout_seconds=$1
    shift
    
    if command -v gtimeout >/dev/null 2>&1; then
        # GNU coreutils installed
        gtimeout "$timeout_seconds" "$@"
    elif command -v timeout >/dev/null 2>&1; then
        # Standard timeout (Linux)
        timeout "$timeout_seconds" "$@"
    else
        # macOS fallback using background process + sleep
        "$@" &
        local pid=$!
        sleep "$timeout_seconds"
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null
            return 124  # Timeout exit code
        fi
        wait "$pid"
    fi
}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    
    # Stop and uninstall service if it exists
    if $BINARY --uninstall 2>/dev/null; then
        log_success "Service uninstalled during cleanup"
    else
        log_warning "No service to uninstall during cleanup"
    fi
    
    # Kill ALL cubby processes (more aggressive)
    log_info "Killing any remaining cubby processes..."
    pkill -9 -f "cubby" 2>/dev/null || true
    killall -9 cubby 2>/dev/null || true
    
    # Additional cleanup for macOS LaunchAgent
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # Unload any stuck services first
        launchctl unload ~/Library/LaunchAgents/${SERVICE_LABEL}.plist 2>/dev/null || true
        launchctl remove ${SERVICE_LABEL} 2>/dev/null || true
        
        # Remove any leftover plist files
        rm -f ~/Library/LaunchAgents/${SERVICE_LABEL}.plist
    fi
    
    # Additional cleanup for Linux systemd
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        systemctl --user stop ${SERVICE_LABEL}.service 2>/dev/null || true
        systemctl --user disable ${SERVICE_LABEL}.service 2>/dev/null || true
        rm -f ~/.config/systemd/user/${SERVICE_LABEL}.service
        systemctl --user daemon-reload 2>/dev/null || true
    fi
    
    # Wait a moment for cleanup
    sleep 2
}

# Test functions
test_build() {
    log_info "Building release binary..."
    if cargo build --release --package cubby-server --bin cubby; then
        log_success "Build successful"
    else
        log_error "Build failed"
        exit 1
    fi
}

test_clean_uninstall() {
    log_info "Test 1: Clean uninstall (should not fail if no service exists)..."
    if $BINARY --uninstall 2>/dev/null; then
        log_success "Uninstall completed (service existed)"
    else
        log_success "Uninstall completed (no service existed)"
    fi
}

test_fresh_install() {
    log_info "Test 2: Fresh service installation..."
    
    # Run cubby (should install and start service)
    log_info "Running: $BINARY"
    log_info "This should install the service, start it, and exit cleanly"
    
    if [[ "$DEBUG_MODE" == "true" ]]; then
        log_info "Debug mode: Running without timeout to see full output"
        log_info "Press Ctrl+C if it hangs..."
        if $BINARY; then
            log_success "Service installation and startup completed"
        else
            local exit_code=$?
            log_error "Service installation failed with exit code $exit_code"
            return 1
        fi
    else
        # Capture output for debugging
        local output_file=$(mktemp)
        if run_with_timeout 30 $BINARY > "$output_file" 2>&1; then
            log_success "Service installation and startup completed"
            log_info "Installation output:"
            cat "$output_file" | sed 's/^/    /'
        else
            local exit_code=$?
            log_error "Service installation failed with exit code $exit_code"
            log_info "Installation output:"
            cat "$output_file" | sed 's/^/    /'
            rm -f "$output_file"
            
            if [[ $exit_code -eq 124 ]]; then
                log_error "Service installation timed out after 30 seconds"
                log_info "Try running with --debug flag to see what's happening: ./test_service_manager.sh --debug"
            else
                log_error "Service installation failed with exit code $exit_code"
            fi
            return 1
        fi
        
        rm -f "$output_file"
    fi
}

test_service_running() {
    log_info "Test 3: Verifying service is running..."
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS LaunchAgent check
        if launchctl list | grep -q "$SERVICE_LABEL"; then
            log_success "Service is loaded in launchctl"
        else
            log_error "Service NOT found in launchctl"
            return 1
        fi
        
        # Check if process is actually running
        if pgrep -f "cubby.*--no-service" > /dev/null; then
            log_success "cubby process is running"
        else
            log_error "cubby process not found"
            return 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux systemd check
        if systemctl --user is-active "$SERVICE_LABEL.service" >/dev/null 2>&1; then
            log_success "Service is active in systemd"
        else
            log_error "Service NOT active in systemd"
            return 1
        fi
    fi
}

test_health_endpoint() {
    log_info "Test 4: Health endpoint check..."
    
    # Wait a bit for service to fully start
    sleep 3
    
    # Try health endpoint multiple times with backoff
    for i in {1..10}; do
        if curl -s "http://127.0.0.1:$HEALTH_PORT/health" > /dev/null 2>&1; then
            log_success "Health endpoint responding"
            return 0
        fi
        log_info "Attempt $i/10: Health endpoint not ready, waiting..."
        sleep 2
    done
    
    log_error "Health endpoint not responding after 20 seconds"
    return 1
}

test_logs_exist() {
    log_info "Test 5: Checking log files..."
    
    # Wait a moment for logs to be created
    sleep 2
    
    if ls ~/.cubby/cubby-*.log 2>/dev/null | head -1 > /dev/null; then
        log_success "Log files exist"
        
        # Show recent log content
        local log_file=$(ls -t ~/.cubby/cubby-*.log 2>/dev/null | head -1)
        if [[ -n "$log_file" ]]; then
            log_info "Recent log content from $log_file:"
            tail -5 "$log_file" | sed 's/^/    /'
        fi
    else
        log_error "No log files found"
        return 1
    fi
}

test_service_restart() {
    log_info "Test 6: Service restart capability..."
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # Stop service via launchctl
        launchctl stop "$SERVICE_LABEL" 2>/dev/null || true
        sleep 2
        
        # Check if it auto-restarted
        if launchctl list | grep -q "$SERVICE_LABEL"; then
            log_success "Service auto-restarted after stop"
        else
            log_warning "Service did not auto-restart (may be expected behavior)"
        fi
        
        # Restart via cubby command
        if run_with_timeout 15 $BINARY; then
            log_success "Service restart via cubby command successful"
        else
            local exit_code=$?
            if [[ $exit_code -eq 124 ]]; then
                log_error "Service restart timed out after 15 seconds"
            else
                log_error "Service restart failed with exit code $exit_code"
            fi
            return 1
        fi
    fi
    
    # Clean up the service after this test to avoid conflicts with foreground mode test
    log_info "Cleaning up service before next test..."
    $BINARY --uninstall 2>/dev/null || true
    pkill -9 -f "cubby" 2>/dev/null || true
    sleep 2
}

test_foreground_mode() {
    log_info "Test 7: Foreground mode (--no-service)..."
    
    # CRITICAL: Clean up any existing service/processes first
    log_info "Ensuring clean state before foreground mode test..."
    $BINARY --uninstall 2>/dev/null || true
    
    # Kill ALL cubby processes
    pkill -9 -f "cubby" 2>/dev/null || true
    killall -9 cubby 2>/dev/null || true
    
    # Wait for processes to die
    sleep 3
    
    # Verify no service exists
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if launchctl list | grep -q "$SERVICE_LABEL"; then
            log_error "Service still exists after cleanup - cannot test foreground mode"
            launchctl list | grep "$SERVICE_LABEL"
            return 1
        fi
    fi
    
    # Verify no processes are running
    if pgrep -f "cubby" > /dev/null; then
        log_error "cubby processes still running after cleanup"
        pgrep -fl "cubby"
        return 1
    fi
    
    log_info "Clean state verified - starting foreground mode test..."
    
    # Test that --no-service doesn't install a service
    log_info "Running in foreground mode for 5 seconds..."
    
    # Start foreground mode in background and kill it after 5 seconds
    run_with_timeout 5 $BINARY --no-service &
    local fg_pid=$!
    
    sleep 2
    
    # Check that no service was installed
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if ! launchctl list | grep -q "$SERVICE_LABEL"; then
            log_success "No service installed in foreground mode"
        else
            log_error "Service was installed in foreground mode (unexpected)"
            log_info "Service status:"
            launchctl list | grep "$SERVICE_LABEL" | sed 's/^/    /'
            return 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if ! systemctl --user is-active "$SERVICE_LABEL.service" >/dev/null 2>&1; then
            log_success "No service installed in foreground mode"
        else
            log_error "Service was installed in foreground mode (unexpected)"
            return 1
        fi
    fi
    
    # Kill the foreground process
    kill $fg_pid 2>/dev/null || true
    wait $fg_pid 2>/dev/null || true
    
    # Additional cleanup
    pkill -9 -f "cubby" 2>/dev/null || true
    
    log_success "Foreground mode test completed"
}

test_custom_port() {
    log_info "Test 8: Custom port configuration..."
    
    # Clean up first
    log_info "Cleaning up before custom port test..."
    $BINARY --uninstall 2>/dev/null || true
    pkill -9 -f "cubby" 2>/dev/null || true
    sleep 3
    
    # Install with custom port
    if run_with_timeout 30 $BINARY --port $TEST_PORT; then
        log_success "Service installed with custom port $TEST_PORT"
    else
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log_error "Service installation with custom port timed out after 30 seconds"
        else
            log_error "Service installation with custom port failed with exit code $exit_code"
        fi
        return 1
    fi
    
    # Test health on custom port
    sleep 3
    if curl -s "http://127.0.0.1:$TEST_PORT/health" > /dev/null 2>&1; then
        log_success "Health endpoint responding on custom port $TEST_PORT"
    else
        log_error "Health endpoint not responding on custom port $TEST_PORT"
        return 1
    fi
    
    # Clean up after custom port test
    log_info "Cleaning up after custom port test..."
    $BINARY --uninstall 2>/dev/null || true
    pkill -9 -f "cubby" 2>/dev/null || true
    sleep 2
}

test_uninstall() {
    log_info "Test 9: Service uninstall..."
    
    # First, install a service so we have something to uninstall
    log_info "Installing service for uninstall test..."
    if run_with_timeout 30 $BINARY; then
        log_success "Service installed for uninstall test"
    else
        log_error "Failed to install service for uninstall test"
        return 1
    fi
    
    sleep 2
    
    # Now test the uninstall
    if $BINARY --uninstall; then
        log_success "Service uninstall command succeeded"
    else
        log_error "Service uninstall command failed"
        return 1
    fi
    
    # Verify service is gone
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if ! launchctl list | grep -q "$SERVICE_LABEL"; then
            log_success "Service successfully removed from launchctl"
        else
            log_error "Service still present in launchctl after uninstall"
            return 1
        fi
        
        # Check plist file is gone
        if [[ ! -f ~/Library/LaunchAgents/${SERVICE_LABEL}.plist ]]; then
            log_success "Service plist file removed"
        else
            log_error "Service plist file still exists after uninstall"
            return 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if ! systemctl --user is-active "$SERVICE_LABEL.service" >/dev/null 2>&1; then
            log_success "Service successfully removed from systemd"
        else
            log_error "Service still active in systemd after uninstall"
            return 1
        fi
    fi
}

test_cloudflared_install() {
    log_info "Test 10: Cloudflared tunnel installation..."
    
    # Check if TEST_CLOUDFLARED_TOKEN is set
    if [[ -z "$TEST_CLOUDFLARED_TOKEN" ]]; then
        log_warning "Skipping cloudflared tests - TEST_CLOUDFLARED_TOKEN not set"
        return 0
    fi
    
    # Clean up first
    log_info "Cleaning up before cloudflared test..."
    $BINARY --uninstall 2>/dev/null || true
    pkill -9 -f "cubby\|cloudflared" 2>/dev/null || true
    sleep 3
    
    # Install with cloudflared enabled
    log_info "Installing service with cloudflared tunnel..."
    export CLOUDFLARED_TUNNEL_TOKEN="$TEST_CLOUDFLARED_TOKEN"
    if run_with_timeout 60 $BINARY --enable-cloudflared; then
        log_success "Service installed with cloudflared"
    else
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log_error "Service installation with cloudflared timed out after 60 seconds"
        else
            log_error "Service installation with cloudflared failed with exit code $exit_code"
        fi
        unset CLOUDFLARED_TUNNEL_TOKEN
        return 1
    fi
    unset CLOUDFLARED_TUNNEL_TOKEN
    
    sleep 3
    
    # Check that cloudflared binary was downloaded
    if ls ~/.cubby/bin/cloudflared-*/cloudflared 2>/dev/null | head -1 > /dev/null; then
        log_success "Cloudflared binary downloaded"
    else
        log_error "Cloudflared binary not found in ~/.cubby/bin/"
        return 1
    fi
    
    # Check that cloudflared service exists
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if [[ -f ~/Library/LaunchAgents/com.cloudflare.cloudflared.plist ]] || \
           [[ -f /Library/LaunchDaemons/com.cloudflare.cloudflared.plist ]]; then
            log_success "Cloudflared service plist exists"
        else
            log_error "Cloudflared service plist not found"
            return 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if [[ -f ~/.config/systemd/user/cloudflared.service ]] || \
           [[ -f /etc/systemd/system/cloudflared.service ]] || \
           [[ -f /lib/systemd/system/cloudflared.service ]]; then
            log_success "Cloudflared systemd service exists"
        else
            log_error "Cloudflared systemd service not found"
            return 1
        fi
    fi
    
    # Check that cloudflared process is running
    sleep 2
    if pgrep -f cloudflared > /dev/null; then
        log_success "Cloudflared process is running"
    else
        log_warning "Cloudflared process not detected (may take time to start)"
    fi
}

test_cloudflared_uninstall() {
    log_info "Test 11: Cloudflared tunnel uninstall..."
    
    # Skip if we didn't install cloudflared
    if [[ -z "$TEST_CLOUDFLARED_TOKEN" ]]; then
        log_warning "Skipping cloudflared uninstall test - TEST_CLOUDFLARED_TOKEN not set"
        return 0
    fi
    
    # Run uninstall
    if $BINARY --uninstall; then
        log_success "Uninstall command completed"
    else
        log_error "Uninstall command failed"
        return 1
    fi
    
    sleep 2
    
    # Verify cloudflared service is removed
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if [[ ! -f ~/Library/LaunchAgents/com.cloudflare.cloudflared.plist ]] && \
           [[ ! -f /Library/LaunchDaemons/com.cloudflare.cloudflared.plist ]]; then
            log_success "Cloudflared service plist removed"
        else
            log_error "Cloudflared service plist still exists after uninstall"
            return 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if [[ ! -f ~/.config/systemd/user/cloudflared.service ]]; then
            log_success "Cloudflared systemd service removed"
        else
            log_error "Cloudflared systemd service still exists after uninstall"
            return 1
        fi
    fi
    
    # Verify cloudflared process is stopped
    if ! pgrep -f cloudflared > /dev/null; then
        log_success "Cloudflared process stopped"
    else
        log_warning "Cloudflared process still running (may be stopping)"
        # Give it a moment and check again
        sleep 2
        if ! pgrep -f cloudflared > /dev/null; then
            log_success "Cloudflared process stopped after delay"
        else
            log_error "Cloudflared process still running after uninstall"
            return 1
        fi
    fi
    
    # Verify binaries were cleaned up
    if ! ls ~/.cubby/bin/cloudflared-* 2>/dev/null | head -1 > /dev/null; then
        log_success "Cloudflared binaries cleaned up"
    else
        log_warning "Cloudflared binaries still present (this is optional)"
    fi
}

# Main test execution
main() {
    # Set up cleanup trap
    trap cleanup EXIT
    
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]] || [[ ! -d "cubby-server" ]]; then
        log_error "Please run this script from the cubby root directory"
        exit 1
    fi
    
    # Show usage if help requested
    if [[ "$1" == "--help" ]] || [[ "$1" == "-h" ]]; then
        echo "Usage: $0 [--debug]"
        echo ""
        echo "Options:"
        echo "  --debug    Run without timeouts to see detailed output"
        echo "  --help     Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0              # Run tests with timeouts"
        echo "  $0 --debug      # Run tests in debug mode"
        exit 0
    fi
    
    # Check if cargo is available
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Cargo not found. Please install Rust toolchain."
        exit 1
    fi
    
    # Check if curl is available
    if ! command -v curl >/dev/null 2>&1; then
        log_error "curl not found. Please install curl."
        exit 1
    fi
    
    log_info "Starting service manager tests..."
    log_info "Platform: $OSTYPE"
    log_info "Binary: $BINARY"
    
    # Run tests
    test_build
    test_clean_uninstall
    test_fresh_install
    test_service_running
    test_health_endpoint
    test_logs_exist
    test_service_restart
    test_foreground_mode
    test_custom_port
    test_uninstall
    test_cloudflared_install
    test_cloudflared_uninstall
    
    log_success "🎉 All tests passed!"
    log_info "Service manager integration is working correctly."
}

# Run main function
main "$@"
