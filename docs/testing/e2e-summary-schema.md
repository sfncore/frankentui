# E2E Summary JSON Schema

The E2E runner writes a structured summary to:

```
<E2E_RESULTS_DIR>/summary.json
```

## Top-level fields

- `timestamp`: ISO-8601 timestamp for the run.
- `total`, `passed`, `failed`, `skipped`: counts by status.
- `duration_ms`: total suite duration in milliseconds.
- `run`:
  - `command`: command used to invoke the suite (or null if unknown).
  - `log_dir`: root log directory for this run.
  - `results_dir`: directory containing per-test result JSON files.
  - `cases_dir`: directory containing per-case bundles.
- `environment`:
  - `date`, `user`, `hostname`, `cwd`, `rustc`, `cargo`, `git_status`, `git_commit`.
- `tests`: array of per-test records (see below).

## Per-test fields

- `name`: case name (string).
- `status`: `passed` | `failed` | `skipped`.
- `duration_ms`: case duration in milliseconds.
- `log_file`: primary log file for the case.
- `case_dir`: per-case bundle directory.
- `bundled_log`: copied case log inside the bundle.
- `pty_file`: original PTY capture path.
- `bundled_pty`: copied PTY capture inside the bundle.
- `pty_hex`: full hex dump of the PTY capture.
- `pty_text`: decoded/printable text from the PTY capture.
- `pty_head_hex`: first N bytes (hex) for failed cases (null otherwise).
- `pty_tail_text`: last N lines of decoded text for failed cases (null otherwise).
- `failure_summary`: summary file for failed cases (null otherwise).
- `env_log`: environment log text file.
- `env_json`: environment JSON file.
- `repro_cmd`: reproduction command for the case (null if unknown).
- `error`: failure reason (null for pass/skip).

## Per-case bundle layout

Each case has a bundle directory under `cases_dir` containing:

- `case.log` (test logs)
- `capture.pty` (raw PTY output)
- `capture.hex` (full hex dump)
- `capture.txt` (decoded text)
- `capture.head.hex` (failed cases, first N bytes)
- `capture.tail.txt` (failed cases, last N lines)
- `failure_summary.txt` (failed cases)
