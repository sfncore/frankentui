#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

E2E_SUITE_SCRIPT="$SCRIPT_DIR/test_mouse_sgr.sh"
export E2E_SUITE_SCRIPT
ONLY_CASE="${E2E_ONLY_CASE:-}"

if [[ ! -x "${E2E_HARNESS_BIN:-}" ]]; then
    LOG_FILE="$E2E_LOG_DIR/mouse_missing.log"
    for t in mouse_click_release mouse_move_event mouse_scroll_events; do
        log_test_skip "$t" "ftui-harness binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
    done
    exit 0
fi

run_case() {
    local name="$1"
    shift
    if [[ -n "$ONLY_CASE" && "$ONLY_CASE" != "$name" ]]; then
        LOG_FILE="$E2E_LOG_DIR/${name}.log"
        log_test_skip "$name" "filtered (E2E_ONLY_CASE=$ONLY_CASE)"
        record_result "$name" "skipped" 0 "$LOG_FILE" "filtered"
        return 0
    fi
    local start_ms
    start_ms="$(date +%s%3N)"

    if "$@"; then
        local end_ms
        end_ms="$(date +%s%3N)"
        local duration_ms=$((end_ms - start_ms))
        log_test_pass "$name"
        record_result "$name" "passed" "$duration_ms" "$LOG_FILE"
        return 0
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    log_test_fail "$name" "mouse SGR assertions failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "mouse SGR assertions failed"
    return 1
}

mouse_click_release() {
    LOG_FILE="$E2E_LOG_DIR/mouse_click_release.log"
    local output_file="$E2E_LOG_DIR/mouse_click_release.pty"

    log_test_start "mouse_click_release"

    PTY_SEND=$'\x1b[<0;10;5M\x1b[<0;10;5m' \
    PTY_SEND_DELAY_MS=300 \
    FTUI_HARNESS_ENABLE_MOUSE=1 \
    FTUI_HARNESS_EXIT_AFTER_MS=1500 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    grep -a -q "Mouse: Down" "$output_file" || return 1
    grep -a -q "Mouse: Up" "$output_file" || return 1
}

mouse_move_event() {
    LOG_FILE="$E2E_LOG_DIR/mouse_move_event.log"
    local output_file="$E2E_LOG_DIR/mouse_move_event.pty"

    log_test_start "mouse_move_event"

    PTY_SEND=$'\x1b[<32;12;6M' \
    PTY_SEND_DELAY_MS=300 \
    FTUI_HARNESS_ENABLE_MOUSE=1 \
    FTUI_HARNESS_EXIT_AFTER_MS=1500 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    grep -a -q "Mouse: Moved" "$output_file" || return 1
}

mouse_scroll_events() {
    LOG_FILE="$E2E_LOG_DIR/mouse_scroll_events.log"
    local output_file="$E2E_LOG_DIR/mouse_scroll_events.pty"

    log_test_start "mouse_scroll_events"

    PTY_SEND=$'\x1b[<64;15;7M\x1b[<65;15;7M' \
    PTY_SEND_DELAY_MS=300 \
    FTUI_HARNESS_ENABLE_MOUSE=1 \
    FTUI_HARNESS_EXIT_AFTER_MS=1500 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    grep -a -q "Mouse: ScrollUp" "$output_file" || return 1
    grep -a -q "Mouse: ScrollDown" "$output_file" || return 1
}

FAILURES=0
run_case "mouse_click_release" mouse_click_release   || FAILURES=$((FAILURES + 1))
run_case "mouse_move_event" mouse_move_event         || FAILURES=$((FAILURES + 1))
run_case "mouse_scroll_events" mouse_scroll_events   || FAILURES=$((FAILURES + 1))
exit "$FAILURES"
