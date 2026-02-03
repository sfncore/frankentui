# E2E Playbook

This playbook describes how to run the PTY-backed E2E suite, interpret logs,
and add or debug scenarios.

## Entry Points

```bash
# Main entry point (wraps tests/e2e/scripts/run_all.sh)
./scripts/e2e_test.sh

# Quick run (inline + cleanup only)
./scripts/e2e_test.sh --quick

# Direct invocation
tests/e2e/scripts/run_all.sh
```

## Suite Structure

- `tests/e2e/scripts/` — individual suites (inline, cleanup, input, resize, etc.)
- `tests/e2e/lib/pty.sh` — PTY runner (python-based capture)
- `tests/e2e/lib/logging.sh` — structured logs + JSON results
- `tests/e2e/fixtures/` — input fixtures for unicode/paste/etc.

## Environment Controls

Common variables:

- `E2E_LOG_DIR` — log directory (default: `/tmp/ftui_e2e_<timestamp>`)
- `E2E_RESULTS_DIR` — JSON results directory
- `E2E_HARNESS_BIN` — path to `ftui-harness` binary (skip build)
- `LOG_LEVEL=DEBUG` — verbose suite logging
- `E2E_ONLY_CASE` — run a single case in scripts that support filtering

PTY runner controls (from `tests/e2e/lib/pty.sh`):

- `PTY_COLS` / `PTY_ROWS` — initial terminal size
- `PTY_RESIZE_COLS` / `PTY_RESIZE_ROWS` / `PTY_RESIZE_DELAY_MS` — scheduled resize
- `PTY_SEND` / `PTY_SEND_FILE` — input payloads
- `PTY_TIMEOUT` / `PTY_DRAIN_TIMEOUT_MS` — timeouts and drain windows

## Capability Profile Matrix

Use `FTUI_TEST_PROFILE` to force a terminal capability profile in tests and
snapshots. When set, snapshot filenames are automatically suffixed with
`__<profile>` so each profile keeps its own baselines.

Examples:

```bash
# Run the demo showcase snapshots as a dumb terminal
FTUI_TEST_PROFILE=dumb cargo test -p ftui-demo-showcase

# Run harness widget snapshots as tmux
FTUI_TEST_PROFILE=tmux cargo test -p ftui-harness widget_snapshots
```

Cross-profile comparison mode (for tests that use `profile_matrix_text`):

```bash
# Report diffs without failing
FTUI_TEST_PROFILE_COMPARE=report cargo test -p ftui-harness profile_matrix

# Fail fast on differences
FTUI_TEST_PROFILE_COMPARE=strict cargo test -p ftui-harness profile_matrix
```

### CI Integration Guide

Run the test suite across a profile matrix to catch capability regressions:

```yaml
strategy:
  matrix:
    profile: [modern, xterm-256color, screen, tmux, dumb, windows-console]

steps:
  - run: FTUI_TEST_PROFILE=${{ matrix.profile }} cargo test -p ftui-demo-showcase
```

## Artifacts

Each run produces:

- `00_environment.log` — environment + git state
- `e2e.log` — suite log
- `results/*.json` — per-test JSON entries
- `results/summary.json` — aggregated pass/fail counts
- `*.pty` — raw PTY captures (binary)

On failures, the suite prints:

- Hex dumps of the first 512 bytes of PTY output
- Printable tail excerpts (via `strings`)

## Running a Single Suite

```bash
E2E_HARNESS_BIN=target/debug/ftui-harness \
tests/e2e/scripts/test_resize.sh
```

Other useful suites:

- `tests/e2e/scripts/test_mouse_sgr.sh`
- `tests/e2e/scripts/test_focus_events.sh`
- `tests/e2e/scripts/test_paste.sh`
- `tests/e2e/scripts/test_kitty_keyboard.sh`

## Troubleshooting

- **Missing python**: ensure `python3` is available.
- **No PTY output**: increase `PTY_TIMEOUT` or `PTY_DRAIN_TIMEOUT_MS`.
- **Mux interference**: unset `TMUX`, `ZELLIJ`, or `STY` in tests that rely on
  scroll regions or pass-through behavior.
- **Binary missing**: build once or set `E2E_HARNESS_BIN`.

## Adding a New Scenario

1. Create a new script in `tests/e2e/scripts/`.
2. Use `pty_run` from `tests/e2e/lib/pty.sh`.
3. Add the suite to `tests/e2e/scripts/run_all.sh`.
4. Add any fixtures to `tests/e2e/fixtures/`.
5. Document the new scenario in `docs/testing/e2e-gap-analysis.md` if needed.
