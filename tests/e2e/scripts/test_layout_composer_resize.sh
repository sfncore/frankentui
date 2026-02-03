#!/bin/bash
set -euo pipefail

# E2E tests for Layout Laboratory resize regressions (Demo Showcase)
# bd-32my.2: Layout Composer — Resize Regression Tests
#
# Scenarios:
# 1. Resize down: 120x40 -> 80x24
# 2. Resize up: 80x24 -> 200x50
# 3. Resize tiny: 120x40 -> 40x10
#
# Logging: JSONL with env/capabilities, seed, timings, checksums.
# Optional benchmarks: set E2E_BENCHMARK=1 to run hyperfine baseline.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

JSONL_FILE="$E2E_RESULTS_DIR/layout_composer_resize.jsonl"
RUN_ID="layout_resize_$(date +%Y%m%d_%H%M%S)_$$"
SEED="${LAYOUT_RESIZE_SEED:-0}"

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
    if command -v shasum >/dev/null 2>&1 && [[ -f "$file" ]]; then
        shasum -a 256 "$file" | awk '{print $1}'
        return 0
    fi
    echo ""
    return 0
}

collect_env_json() {
    if command -v jq >/dev/null 2>&1; then
        jq -nc \
            --arg os "$(uname -s)" \
            --arg arch "$(uname -m)" \
            --arg term "${TERM:-}" \
            --arg colorterm "${COLORTERM:-}" \
            --arg tmux "${TMUX:-}" \
            --arg zellij "${ZELLIJ:-}" \
            '{os:$os,arch:$arch,term:$term,colorterm:$colorterm,tmux:$tmux,zellij:$zellij}'
    else
        printf '{"os":"%s","arch":"%s","term":"%s"}' "$(uname -s)" "$(uname -m)" "${TERM:-}"
    fi
}

detect_capabilities_json() {
    local truecolor="false" color256="false" mux="none"
    [[ "${COLORTERM:-}" == "truecolor" || "${COLORTERM:-}" == "24bit" ]] && truecolor="true"
    [[ "${TERM:-}" == *"256color"* ]] && color256="true"
    [[ -n "${TMUX:-}" ]] && mux="tmux"
    [[ -n "${ZELLIJ:-}" ]] && mux="zellij"
    if command -v jq >/dev/null 2>&1; then
        jq -nc \
            --argjson truecolor "$truecolor" \
            --argjson color256 "$color256" \
            --arg mux "$mux" \
            '{truecolor:$truecolor,color_256:$color256,mux:$mux}'
    else
        printf '{"truecolor":%s,"color_256":%s,"mux":"%s"}' "$truecolor" "$color256" "$mux"
    fi
}

ENV_JSON="$(collect_env_json)"
CAPS_JSON="$(detect_capabilities_json)"

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

detect_layout_screen() {
    local bin="$1"
    local help
    help="$($bin --help 2>/dev/null || true)"
    if [[ -z "$help" ]]; then
        return 1
    fi
    local line
    line=$(printf '%s\n' "$help" | command grep -E "Layout Lab|Layout Laboratory" | head -n 1 || true)
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
    local name="$1" send_label="$2" start_cols="$3" start_rows="$4" resize_cols="$5" resize_rows="$6"
    shift 6
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
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"status\":\"passed\",\"duration_ms\":$duration_ms,\"output_bytes\":$size,\"output_sha256\":\"$output_sha\",\"send\":\"$send_label\",\"cols\":$start_cols,\"rows\":$start_rows,\"resize_cols\":$resize_cols,\"resize_rows\":$resize_rows,\"seed\":\"$SEED\",\"env\":$ENV_JSON,\"capabilities\":$CAPS_JSON}"
        return 0
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    local output_sha
    output_sha="$(sha256_file "$output_file")"
    log_test_fail "$name" "assertion failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "assertion failed"
    jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$name\",\"status\":\"failed\",\"duration_ms\":$duration_ms,\"output_sha256\":\"$output_sha\",\"send\":\"$send_label\",\"cols\":$start_cols,\"rows\":$start_rows,\"resize_cols\":$resize_cols,\"resize_rows\":$resize_rows,\"seed\":\"$SEED\",\"env\":$ENV_JSON,\"capabilities\":$CAPS_JSON}"
    return 1
}

DEMO_BIN="$(ensure_demo_bin || true)"
if [[ -z "$DEMO_BIN" ]]; then
    LOG_FILE="$E2E_LOG_DIR/layout_resize_missing.log"
    for t in layout_resize_down layout_resize_up layout_resize_tiny; do
        log_test_skip "$t" "ftui-demo-showcase binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$t\",\"status\":\"skipped\",\"reason\":\"binary missing\",\"seed\":\"$SEED\"}"
    done
    exit 0
fi

LAYOUT_SCREEN="$(detect_layout_screen "$DEMO_BIN" || true)"
if [[ -z "$LAYOUT_SCREEN" ]]; then
    LOG_FILE="$E2E_LOG_DIR/layout_resize_missing.log"
    for t in layout_resize_down layout_resize_up layout_resize_tiny; do
        log_test_skip "$t" "Layout Laboratory screen not registered in --help"
        record_result "$t" "skipped" 0 "$LOG_FILE" "screen missing"
        jsonl_log "{\"run_id\":\"$RUN_ID\",\"case\":\"$t\",\"status\":\"skipped\",\"reason\":\"screen missing\",\"seed\":\"$SEED\"}"
    done
    exit 0
fi

export FTUI_DEMO_SCREEN="$LAYOUT_SCREEN"
export FTUI_DEMO_SEED="$SEED"

layout_resize_down() {
    LOG_FILE="$E2E_LOG_DIR/layout_resize_down.log"
    local output_file="$E2E_LOG_DIR/layout_resize_down.pty"

    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_RESIZE_DELAY_MS=300 \
    PTY_RESIZE_COLS=80 \
    PTY_RESIZE_ROWS=24 \
    FTUI_DEMO_EXIT_AFTER_MS=1600 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1
    command grep -a -qi "Layout Laboratory\|Preset" "$output_file" || return 1
}

layout_resize_up() {
    LOG_FILE="$E2E_LOG_DIR/layout_resize_up.log"
    local output_file="$E2E_LOG_DIR/layout_resize_up.pty"

    PTY_COLS=80 \
    PTY_ROWS=24 \
    PTY_RESIZE_DELAY_MS=300 \
    PTY_RESIZE_COLS=200 \
    PTY_RESIZE_ROWS=50 \
    FTUI_DEMO_EXIT_AFTER_MS=1600 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 300 ]] || return 1
    command grep -a -qi "Layout Laboratory\|Preset" "$output_file" || return 1
}

layout_resize_tiny() {
    LOG_FILE="$E2E_LOG_DIR/layout_resize_tiny.log"
    local output_file="$E2E_LOG_DIR/layout_resize_tiny.pty"

    PTY_COLS=120 \
    PTY_ROWS=40 \
    PTY_RESIZE_DELAY_MS=300 \
    PTY_RESIZE_COLS=40 \
    PTY_RESIZE_ROWS=10 \
    FTUI_DEMO_EXIT_AFTER_MS=1600 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$DEMO_BIN"

    local size
    size=$(wc -c < "$output_file" | tr -d ' ')
    [[ "$size" -gt 200 ]] || return 1
    command grep -a -qi "Layout Laboratory\|Preset" "$output_file" || return 1
}

FAILURES=0
run_case "layout_resize_down" "" 120 40 80 24 layout_resize_down || FAILURES=$((FAILURES + 1))
run_case "layout_resize_up" "" 80 24 200 50 layout_resize_up || FAILURES=$((FAILURES + 1))
run_case "layout_resize_tiny" "" 120 40 40 10 layout_resize_tiny || FAILURES=$((FAILURES + 1))

TOTAL_TESTS=3
PASSED=$((TOTAL_TESTS - FAILURES))
if command -v jq >/dev/null 2>&1; then
    jq -nc \
        --arg run_id "$RUN_ID" \
        --arg event "run_end" \
        --arg ts "$(date -Iseconds)" \
        --arg seed "$SEED" \
        --argjson total_tests "$TOTAL_TESTS" \
        --argjson passed "$PASSED" \
        --argjson failed "$FAILURES" \
        '{run_id:$run_id,event:$event,ts:$ts,seed:$seed,total_tests:$total_tests,passed:$passed,failed:$failed}' \
        >> "$JSONL_FILE"
else
    jsonl_log "{\"run_id\":\"$RUN_ID\",\"event\":\"run_end\",\"ts\":\"$(date -Iseconds)\",\"seed\":\"$SEED\",\"total_tests\":$TOTAL_TESTS,\"passed\":$PASSED,\"failed\":$FAILURES}"
fi

# Optional: Hyperfine baseline (p50/p95/p99) for startup+render.
if [[ "${E2E_BENCHMARK:-}" == "1" ]]; then
    BENCH_RESULTS="$E2E_RESULTS_DIR/layout_resize_bench.json"
    if command -v hyperfine >/dev/null 2>&1; then
        log_info "Running hyperfine benchmarks for layout lab startup..."
        hyperfine \
            --warmup 2 \
            --runs 10 \
            --export-json "$BENCH_RESULTS" \
            --export-markdown "$E2E_RESULTS_DIR/layout_resize_bench.md" \
            "FTUI_DEMO_SCREEN=$LAYOUT_SCREEN FTUI_DEMO_EXIT_AFTER_MS=200 $DEMO_BIN" \
            2>&1 | tee "$E2E_LOG_DIR/hyperfine.log" || true

        if [[ -f "$BENCH_RESULTS" ]] && command -v jq >/dev/null 2>&1; then
            stats=$(jq -r '
                def pct(p):
                    . as $t
                    | ($t | length) as $n
                    | ( ($n - 1) * p | floor ) as $i
                    | $t[$i];
                .results[0].times
                | sort
                | {p50: pct(0.5), p95: pct(0.95), p99: pct(0.99)}
            ' "$BENCH_RESULTS" 2>/dev/null || echo "")
            p50_ms=$(printf '%s' "$stats" | jq -r '.p50 * 1000 | floor' 2>/dev/null || echo 0)
            p95_ms=$(printf '%s' "$stats" | jq -r '.p95 * 1000 | floor' 2>/dev/null || echo 0)
            p99_ms=$(printf '%s' "$stats" | jq -r '.p99 * 1000 | floor' 2>/dev/null || echo 0)

            jq -nc \
                --arg run_id "$RUN_ID" \
                --arg event "benchmark" \
                --arg ts "$(date -Iseconds)" \
                --arg seed "$SEED" \
                --arg benchmark "startup" \
                --argjson p50_ms "$p50_ms" \
                --argjson p95_ms "$p95_ms" \
                --argjson p99_ms "$p99_ms" \
                '{run_id:$run_id,event:$event,ts:$ts,seed:$seed,benchmark:$benchmark,p50_ms:$p50_ms,p95_ms:$p95_ms,p99_ms:$p99_ms}' \
                >> "$JSONL_FILE"

            log_info "Benchmark percentiles: p50=${p50_ms}ms, p95=${p95_ms}ms, p99=${p99_ms}ms"
        fi
    else
        log_warn "hyperfine not found, skipping benchmarks (install with: cargo install hyperfine)"
    fi
fi

# Print seed for reproducibility
log_info "Run completed with seed: $SEED (use LAYOUT_RESIZE_SEED=$SEED to reproduce)"

exit "$FAILURES"
