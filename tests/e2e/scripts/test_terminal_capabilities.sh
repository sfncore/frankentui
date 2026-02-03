#!/bin/bash
set -euo pipefail

# E2E tests for Terminal Capability Explorer (Demo Showcase)
# bd-2sog.4: Terminal Capability Explorer — E2E PTY Tests (Verbose Logs)
#
# Scenarios:
# 1. Smoke test: render screen and verify capability display
# 2. View mode cycling: Tab cycles Matrix → Evidence → Simulation
# 3. Capability selection: Up/Down navigates capability list
# 4. Profile simulation: P cycles through profiles, R resets
#
# Keybindings:
# - Tab: Cycle view (matrix/evidence/simulation)
# - ↑/↓ or j/k: Select capability
# - P: Cycle simulated profile
# - R: Reset to detected profile

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

JSONL_FILE="$E2E_RESULTS_DIR/terminal_capabilities.jsonl"
RUN_ID="terminal_caps_$(date +%Y%m%d_%H%M%S)_$$"
SEED="${FTUI_DEMO_SEED:-0}"

jsonl_log() {
    local line="$1"
    mkdir -p "$E2E_RESULTS_DIR"
    printf '%s\n' "$line" >> "$JSONL_FILE"
}

sha256_file() {
    local file="$1"
    if command -v sha256sum >/dev/null 2>&1 && [[ -f "$file" ]]; then
        sha256sum "$file" | awk '{print $1}'
        return 0
    fi
    echo ""
    return 0
}

ensure_demo_bin() {
    local target_dir="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
    local bin="$target_dir/debug/ftui-demo-showcase"
    if [[ -x "$bin" ]]; then
        echo "$bin"
        return 0
    fi
    log_info "Building ftui-demo-showcase (debug)..." >&2
    (cd "$PROJECT_ROOT" && cargo build -p ftui-demo-showcase >/dev/null)
    if [[ -x "$bin" ]]; then
        echo "$bin"
        return 0
    fi
    return 1
}

detect_caps_screen() {
    local bin="$1"
    local help
    help="$($bin --help 2>/dev/null || true)"
    if [[ -z "$help" ]]; then
        return 1
    fi
    local line
    # Look for "Terminal Caps" (the short name in help)
    # Use 'command grep' to bypass any shell aliases (e.g., to ripgrep)
    line=$(printf '%s\n' "$help" | command grep "Terminal Caps" | head -n 1 || true)
    if [[ -z "$line" ]]; then
        return 1
    fi
    local screen
    screen=$(printf '%s' "$line" | awk '{print $1}')
    if [[ ! "$screen" =~ ^[0-9]+$ ]]; then
        return 1
    fi
    printf '%s' "$screen"
    return 0
}

run_case() {
    local name="$1"
    local send_label="$2"
    local view_mode="$3"
    shift 3
    local start_ms
    start_ms="$(date +%s%3N)"

    LOG_FILE="$E2E_LOG_DIR/${name}.log"
    local output_file="$E2E_LOG_DIR/${name}.pty"

    log_test_start "$name"

    if "$@"; then
        local end_ms
        end_ms="$(date +%s%3N)"
        local duration_ms=$((end_ms - start_ms))
        local size
        size=$(wc -c < "$output_file" | tr -d ' ')
        local output_sha
        output_sha="$(sha256_file "$output_file")"
        log_test_pass "$name"
        record_result "$name" "passed" "$duration_ms" "$LOG_FILE"
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"status\":\"passed\",\"duration_ms\":$duration_ms,\"output_bytes\":$size,\"output_sha256\":\"$output_sha\",\"send\":\"$send_label\",\"view_mode\":\"$view_mode\",\"cols\":120,\"rows\":40,\"seed\":\"$SEED\",\"screen\":\"${CAPS_SCREEN:-}\",\"term\":\"${TERM:-}\",\"colorterm\":\"${COLORTERM:-}\",\"no_color\":\"${NO_COLOR:-}\"}"
        return 0
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    local output_sha
    output_sha="$(sha256_file "$output_file")"
    log_test_fail "$name" "assertion failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "assertion failed"
    jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"status\":\"failed\",\"duration_ms\":$duration_ms,\"output_sha256\":\"$output_sha\",\"send\":\"$send_label\",\"view_mode\":\"$view_mode\",\"cols\":120,\"rows\":40,\"seed\":\"$SEED\",\"screen\":\"${CAPS_SCREEN:-}\",\"term\":\"${TERM:-}\",\"colorterm\":\"${COLORTERM:-}\",\"no_color\":\"${NO_COLOR:-}\"}"
    return 1
}

DEMO_BIN="$(ensure_demo_bin || true)"
if [[ -z "$DEMO_BIN" ]]; then
    LOG_FILE="$E2E_LOG_DIR/terminal_caps_missing.log"
    for t in caps_smoke caps_view_cycle caps_navigation caps_profile_cycle caps_profile_reset; do
        log_test_skip "$t" "ftui-demo-showcase binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$t\",\"status\":\"skipped\",\"reason\":\"binary missing\",\"seed\":\"$SEED\",\"screen\":\"${CAPS_SCREEN:-}\",\"term\":\"${TERM:-}\",\"colorterm\":\"${COLORTERM:-}\",\"no_color\":\"${NO_COLOR:-}\"}"
    done
    exit 0
fi

CAPS_SCREEN="$(detect_caps_screen "$DEMO_BIN" || true)"
if [[ -z "$CAPS_SCREEN" ]]; then
    LOG_FILE="$E2E_LOG_DIR/terminal_caps_missing.log"
    for t in caps_smoke caps_view_cycle caps_navigation caps_profile_cycle caps_profile_reset; do
        log_test_skip "$t" "Terminal Capabilities screen not registered in --help"
        record_result "$t" "skipped" 0 "$LOG_FILE" "screen missing"
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$t\",\"status\":\"skipped\",\"reason\":\"screen missing\",\"seed\":\"$SEED\",\"screen\":\"${CAPS_SCREEN:-}\",\"term\":\"${TERM:-}\",\"colorterm\":\"${COLORTERM:-}\",\"no_color\":\"${NO_COLOR:-}\"}"
    done
    exit 0
fi

# Control bytes
TAB=$'\t'

# Export demo config so it's visible to subprocesses
export FTUI_DEMO_SCREEN="$CAPS_SCREEN"

# Test 1: Smoke test (render Terminal Capabilities screen)
# Verifies the screen renders and shows capability-related content
caps_smoke() {
    LOG_FILE="$E2E_LOG_DIR/caps_smoke.log"
    local output_file="$E2E_LOG_DIR/caps_smoke.pty"

    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_SEND_DELAY_MS=200 \
    PTY_SEND="" \
    FTUI_DEMO_EXIT_AFTER_MS=1200 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1

    # Expect capability-related content to appear
    # Should show "Terminal Capabilities" or capability names like "TrueColor", "Synchronized"
    command grep -a -qi "Terminal Capabilities\|TrueColor\|Synchronized\|Hyperlinks\|Capability" "$output_file" || return 1
}

# Test 2: View mode cycling (Tab cycles through Matrix → Evidence → Simulation)
# Sends Tab key to cycle view modes
caps_view_cycle() {
    LOG_FILE="$E2E_LOG_DIR/caps_view_cycle.log"
    local output_file="$E2E_LOG_DIR/caps_view_cycle.pty"

    # Send Tab twice to cycle through views
    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_SEND_DELAY_MS=300 \
    PTY_SEND="${TAB}${TAB}" \
    FTUI_DEMO_EXIT_AFTER_MS=1800 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1

    # Should show view mode labels: Matrix, Evidence, or Simulation
    # At least one should appear after cycling
    command grep -a -qi "Matrix\|Evidence\|Simulation" "$output_file" || return 1
}

# Test 3: Capability navigation (Up/Down or j/k selects capabilities)
# Sends j/k to navigate capability list
caps_navigation() {
    LOG_FILE="$E2E_LOG_DIR/caps_navigation.log"
    local output_file="$E2E_LOG_DIR/caps_navigation.pty"

    # Send j to move down, k to move up
    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_SEND_DELAY_MS=300 \
    PTY_SEND="jjkk" \
    FTUI_DEMO_EXIT_AFTER_MS=1800 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1

    # Navigation should still show capability content
    command grep -a -qi "Capability\|Terminal\|TrueColor\|Detected" "$output_file" || return 1
}

# Test 4: Profile cycling (P cycles through simulated profiles)
# Sends P to cycle through different terminal profiles
caps_profile_cycle() {
    LOG_FILE="$E2E_LOG_DIR/caps_profile_cycle.log"
    local output_file="$E2E_LOG_DIR/caps_profile_cycle.pty"

    # Send P twice to cycle profiles
    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_SEND_DELAY_MS=300 \
    PTY_SEND="PP" \
    FTUI_DEMO_EXIT_AFTER_MS=1800 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1

    # Should still display capability information after profile change
    command grep -a -qi "Capability\|Terminal\|Profile\|Detected" "$output_file" || return 1
}

# Test 5: Profile reset (R resets to detected profile)
# Sends P to change profile, then R to reset
caps_profile_reset() {
    LOG_FILE="$E2E_LOG_DIR/caps_profile_reset.log"
    local output_file="$E2E_LOG_DIR/caps_profile_reset.pty"

    # Send P to change profile, then R to reset
    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_SEND_DELAY_MS=300 \
    PTY_SEND="PR" \
    FTUI_DEMO_EXIT_AFTER_MS=1800 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1

    # Should still display capability information after reset
    command grep -a -qi "Capability\|Terminal\|Detected" "$output_file" || return 1
}

# Test 6: Small terminal test (80x24)
# Verifies screen renders correctly on minimal terminal size
caps_small_terminal() {
    LOG_FILE="$E2E_LOG_DIR/caps_small_terminal.log"
    local output_file="$E2E_LOG_DIR/caps_small_terminal.pty"

    PTY_COLS=80 \
    PTY_ROWS=24 \
    PTY_SEND_DELAY_MS=200 \
    PTY_SEND="" \
    FTUI_DEMO_EXIT_AFTER_MS=1200 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    # Should produce output even in small terminal
    [[ "$size" -gt 200 ]] || return 1
}

FAILURES=0
run_case "caps_smoke" "" "initial" caps_smoke || FAILURES=$((FAILURES + 1))
run_case "caps_view_cycle" "TAB,TAB" "cycle" caps_view_cycle || FAILURES=$((FAILURES + 1))
run_case "caps_navigation" "j,j,k,k" "matrix" caps_navigation || FAILURES=$((FAILURES + 1))
run_case "caps_profile_cycle" "P,P" "matrix" caps_profile_cycle || FAILURES=$((FAILURES + 1))
run_case "caps_profile_reset" "P,R" "matrix" caps_profile_reset || FAILURES=$((FAILURES + 1))
run_case "caps_small_terminal" "" "initial_80x24" caps_small_terminal || FAILURES=$((FAILURES + 1))

if [[ "$FAILURES" -gt 0 ]]; then
    log_error "Terminal Capabilities E2E tests: $FAILURES failure(s)"
    exit 1
fi

log_info "Terminal Capabilities E2E tests: all passed"
exit 0
