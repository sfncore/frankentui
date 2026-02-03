# Windows Compatibility (v1)

This document records what FrankenTUI targets for Windows support in v1 and the
known limitations. Behavior varies by terminal emulator and by backend.

Status note: This project is still early. The items below are the **targeted**
v1 behavior, not a guarantee for every Windows terminal.

## CI Validation Status

**Last validated:** 2026-02-03 (bd-31go)

### Cross-Platform CI Matrix

| Platform | Runner | Rust Toolchain | Status |
|----------|--------|----------------|--------|
| Linux | `ubuntu-latest` | stable, nightly | Validated |
| macOS | `macos-latest` | stable, nightly | Validated |
| Windows | `windows-latest` | stable | Validated |

See `.github/workflows/ci.yml` for the full configuration.

### Automated Testing

- **Format/Clippy/Build**: All platforms
- **Unit tests**: All crates (`ftui-core`, `ftui-render`, `ftui-widgets`, etc.)
- **Platform-specific capability detection**: `#[cfg(target_os)]` tests
  - Windows Terminal detection via `WT_SESSION`
  - macOS iTerm2 detection via `TERM_PROGRAM`
  - Linux terminal detection via standard env vars
- **Snapshot tests**: Platform-independent golden snapshots
- **E2E tests**: PTY-based tests on Linux/macOS (see `scripts/cross_platform_e2e.sh`)

### Manual Validation Required

- Terminal-specific quirks (Terminal.app, iTerm2, Windows Terminal)
- Edge cases in legacy consoles (cmd.exe, ConHost)

## Supported Features (v1 target)

- Raw mode enter/exit with cleanup on panic (best effort via the backend)
- Basic key input handling (letters, arrows, modifiers)
- Resize events (where the backend provides them)
- Basic mouse support when the terminal supports SGR mouse encoding
- Color output:
  - 16 colors (baseline)
  - 256 colors (Windows Terminal, modern ConHost)
  - TrueColor (Windows Terminal)

## Known Limitations (v1)

- DEC synchronized output (mode 2026) is not widely supported on Windows
- OSC 8 hyperlinks: Windows Terminal only; cmd.exe and legacy ConHost do not
- Bracketed paste: varies by terminal emulator
- Focus events: may be missing or unreliable in some terminals
- Kitty keyboard protocol: limited/absent support on Windows
- Scroll-region optimization (DECSTBM): behavior varies by terminal
- Mouse SGR mode may fall back to legacy encoding on some terminals

## Terminal Compatibility Matrix (Windows)

| Feature | Windows Terminal | cmd.exe | ConHost | PowerShell |
|---------|------------------|---------|---------|------------|
| TrueColor | Yes | No | Partial | Depends |
| OSC 8 Links | Yes | No | No | Partial |
| Mouse (SGR) | Yes | No | Partial | Partial |
| Sync Output (DEC 2026) | No | No | No | No |

## Configuration Recommendations

- Prefer **Windows Terminal** for the best experience.
- Use a Unicode-capable font (Cascadia Mono, JetBrains Mono, Fira Code).
- If using legacy consoles, ensure UTF-8 mode is enabled.
- `WT_SESSION` (Windows Terminal) is treated as authoritative even if `TERM` is missing.
- If `TERM`/`COLORTERM` are missing and `WT_SESSION` is not set, expect reduced feature detection.

## Troubleshooting

- Colors do not show: verify terminal supports the color depth; check `COLORTERM`.
- Mouse not working: enable mouse support in the terminal settings.
- Cleanup not working: legacy ConHost may not restore modes reliably.
- Unicode display broken: verify font and codepage; avoid cmd.exe for complex text.

## Future Improvements

- Deeper ConHost support (where technically possible)
- WSL-specific notes and validation
- Expanded PTY tests on Windows CI runners
- More explicit capability probes for missing env vars

## Cross-References

- ADR-004 (Windows v1 scope) — pending
- Terminal compatibility matrix (bd-1un) — pending
- Capability detection: `crates/ftui-core/src/terminal_capabilities.rs`

## Validation Checklist (bd-31go)

- [x] Linux tests passing (CI: ubuntu-latest)
- [x] macOS tests passing (CI: macos-latest)
- [x] Windows tests passing (CI: windows-latest)
- [x] CI matrix configured (multi-OS, multi-Rust)
- [x] Platform-specific capability detection tested
- [x] Snapshot tests run across platforms
- [x] docs/WINDOWS.md updated with validated status
