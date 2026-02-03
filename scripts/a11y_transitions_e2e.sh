#!/bin/bash
# Accessibility Modes Transition E2E Tests (bd-2o55.2)
#
# Runs targeted a11y transition regression tests with JSONL logging.
#
# Usage:
#   ./scripts/a11y_transitions_e2e.sh [--verbose] [--quick]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

VERBOSE=false
QUICK=false

for arg in "$@"; do
    case "$arg" in
        --verbose|-v) VERBOSE=true ;;
        --quick)      QUICK=true ;;
        --help|-h)
            echo "Usage: $0 [--verbose] [--quick]"
            echo "  --verbose  Show full output"
            echo "  --quick    Skip compilation, run tests only"
            exit 0
            ;;
    esac
done

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
LOG_DIR="${LOG_DIR:-/tmp/ftui-a11y-transitions-${TIMESTAMP}}"
mkdir -p "$LOG_DIR"

PASSED=0
FAILED=0
SKIPPED=0

# ---------------------------------------------------------------------------
# JSONL logging
# ---------------------------------------------------------------------------

jsonl() {
    local step="$1"
    shift
    local fields="\"ts\":\"$(date -Iseconds)\",\"step\":\"$step\""
    while (( $# >= 2 )); do
        fields="${fields},\"$1\":\"$2\""
        shift 2
    done
    echo "{${fields}}" >> "$LOG_DIR/e2e.jsonl"
    if $VERBOSE; then
        echo "{${fields}}" >&2
    fi
}

run_step() {
    local name="$1"
    shift
    local step_start
    step_start=$(date +%s%3N)

    jsonl "step_start" "name" "$name"

    local exit_code=0
    local output_file="$LOG_DIR/${name}.log"

    if $VERBOSE; then
        "$@" 2>&1 | tee "$output_file" || exit_code=$?
    else
        "$@" > "$output_file" 2>&1 || exit_code=$?
    fi

    local step_end
    step_end=$(date +%s%3N)
    local elapsed=$(( step_end - step_start ))

    if [ "$exit_code" -eq 0 ]; then
        PASSED=$((PASSED + 1))
        jsonl "step_pass" "name" "$name" "elapsed_ms" "$elapsed"
        printf "  %-50s  PASS  (%s ms)\n" "$name" "$elapsed"
    else
        FAILED=$((FAILED + 1))
        jsonl "step_fail" "name" "$name" "elapsed_ms" "$elapsed" "exit_code" "$exit_code"
        printf "  %-50s  FAIL  (exit %s, %s ms)\n" "$name" "$exit_code" "$elapsed"
        echo "    Log: $output_file"
    fi
}

skip_step() {
    local name="$1"
    SKIPPED=$((SKIPPED + 1))
    jsonl "step_skip" "name" "$name"
    printf "  %-50s  SKIP\n" "$name"
}

echo "=========================================="
echo " Accessibility Modes Transition E2E (bd-2o55.2)"
echo "=========================================="
echo ""

jsonl "env" \
    "project_root" "$PROJECT_ROOT" \
    "log_dir" "$LOG_DIR" \
    "rust_version" "$(rustc --version 2>/dev/null || echo N/A)" \
    "cargo_version" "$(cargo --version 2>/dev/null || echo N/A)" \
    "term" "${TERM:-unknown}" \
    "colorterm" "${COLORTERM:-unknown}" \
    "seed" "${A11Y_TEST_SEED:-0}"

echo "  Log directory: $LOG_DIR"
echo ""

if ! $QUICK; then
    run_step "cargo_check" \
        cargo check -p ftui-demo-showcase --tests --quiet

    run_step "cargo_clippy" \
        cargo clippy -p ftui-demo-showcase --tests -- -D warnings --quiet
else
    skip_step "cargo_check"
    skip_step "cargo_clippy"
fi

run_step "a11y_transition_tests" bash -c "
    cd '$PROJECT_ROOT' &&
    E2E_JSONL=1 A11Y_TEST_SEED=\${A11Y_TEST_SEED:-0} \
        cargo test -p ftui-demo-showcase --test a11y_snapshots -- a11y_transition --nocapture
"

echo ""
echo "=========================================="
TOTAL=$((PASSED + FAILED + SKIPPED))
echo "  Total: $TOTAL  Passed: $PASSED  Failed: $FAILED  Skipped: $SKIPPED"
echo "=========================================="
echo ""

jsonl "summary" \
    "total" "$TOTAL" \
    "passed" "$PASSED" \
    "failed" "$FAILED" \
    "skipped" "$SKIPPED"

[[ $FAILED -eq 0 ]]
