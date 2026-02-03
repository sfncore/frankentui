# Command Palette Specification

## Overview
The Command Palette is a modal overlay that allows users to discover and execute actions via fuzzy search. It serves as the primary navigation and command execution interface for power users.

## 1. UX & Behavior

### Activation
- **Open:** `Ctrl+P` (primary), `/` (optional, if context allows).
- **Close:** `Esc` (restores focus to previous widget).
- **Mode:** Modal overlay, centered, width 60% (min 60 cols), max height 80% (min 10 rows).

### Interaction
- **Query Input:**
  - Auto-focused on open.
  - Real-time filtering as user types.
  - `Backspace` deletes characters.
  - Empty query shows "Top Actions" (recently used or pinned).
- **Navigation:**
  - `Up` / `Down`: Select previous/next item (wraps around).
  - `PageUp` / `PageDown`: Jump by viewport height.
  - `Home` / `End`: Jump to first/last item.
  - `Tab`: Toggle focus between results list and preview panel (if preview enabled).
- **Execution:**
  - `Enter`: Execute selected action and close palette.
  - If action requires arguments, it may open a secondary dialog (out of scope for v1 palette, but forward-compatible).

### UI Layout
```
+------------------------------------------------------+
| > Type a command...                        [12 matches]|
+------------------------------------------------------+
| > Open Settings             [General]      Ctrl+,    | <- Selected
|   Switch to Dashboard       [Nav]          1         |
|   Toggle Dark Mode          [View]         Ctrl+T    |
|   ...                                                |
+------------------------------------------------------+
| [Enter] Run  [Esc] Close  [Tab] Preview              |
+------------------------------------------------------+
```
- **Preview Panel (Optional/Right-side):**
  - Displays description, usage examples, or detailed metadata for the selected action.
  - Visible only if the action has rich metadata.

### Empty States
- **No Query:** Show "Top Actions" or a help tip ("Type to search...").
- **No Results:** Show "No matching commands" with a suggestion ("Try 'help'").

## 2. Scoring & Ranking (Deterministic)

The fuzzy matcher must be **deterministic** and **stable** to prevent muscle-memory breakage.

### Scoring Rules (Priority Order)
1. **Exact Match:** Query equals Action Title (case-insensitive).
2. **Prefix Match:** Action Title starts with Query.
3. **Word Boundary:** Query matches start of words (e.g., "st" matches "**S**witch **T**heme").
4. **Contiguous Match:** "od" matches "C**od**e" > "C**o**mman**d**".
5. **Distance:** Matches closer to the start of the string score higher.

### Tie-Breaking (Critical for Stability)
If two actions have the same score:
1. **Recency:** Recently used actions score higher (if history tracking enabled).
2. **Length:** Shorter titles score higher.
3. **Alphabetical:** Stable sort by Title.
4. **Registry Index:** Registration order as final fallback.

**Constraint:** The ranking function `score(query, action) -> i32` must be pure and unit-testable.

## 3. Data Model

```rust
struct ActionItem {
    id: ActionId,           // Unique identifier (e.g., "view.toggle_theme")
    title: String,          // Display text (e.g., "Toggle Dark Mode")
    description: Option<String>, // Helper text for preview
    category: Option<String>,    // Grouping (e.g., "View", "Navigation")
    keywords: Vec<String>,       // Hidden search terms
    shortcut: Option<KeyCombo>,  // Displayed shortcut hint
    enabled: bool,               // If false, hidden or dimmed
    run: Box<dyn Fn(&mut AppState)>, // Execution callback
}
```

## 4. Accessibility (A11y)

- **Contrast:** Selection highlight must meet WCAG AA (4.5:1) against background.
- **Indicators:**
  - Selected item must use more than just color (e.g., prefix `> ` or bold text).
  - Match characters in the title should be underlined or highlighted.
- **Screen Readers:**
  - Arrow navigation should trigger aria-live announcements (if supported by terminal/harness).

## 5. Performance Budgets

- **Open Latency:** < 50ms (p99).
- **Query Latency:** < 16ms (1 frame) for 1000 items.
- **Allocations:** Zero per-frame allocations during navigation (reuse buffers).

## 6. Test Plan

### Unit Tests
- `test_matcher_exact_vs_fuzzy`: Verify scoring prioritization.
- `test_ranking_stability`: Verify tie-breaking rules.
- `test_navigation_wrap`: Up from top goes to bottom.

### Snapshot Tests
- `snapshot_palette_empty`: Initial state.
- `snapshot_palette_results`: Query "test" with mock results.
- `snapshot_palette_no_results`: Query "xyz" with no matches.
- `snapshot_palette_preview`: Selection with preview panel.

### E2E Tests (PTY)
- **Scenario:**
  1. Open palette (`Ctrl+P`).
  2. Type "dash".
  3. Verify "Switch to Dashboard" is selected.
  4. Press `Enter`.
  5. Verify Dashboard screen is active.
- **Logs:** Capture query latency and result counts in JSONL.
