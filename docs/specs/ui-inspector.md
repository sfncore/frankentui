# UI Inspector Spec + UX Flow

> **Bead**: bd-17h9.1
> **Status**: In Progress
> **Author**: SilverBay
> **Epic**: bd-17h9 (Demo Showcase: UI Inspector + Hit-Test Overlay)

## Overview

The UI Inspector is an interactive debugging tool that visualizes the widget tree, hit-test regions, and layout constraints. It helps developers understand widget composition, debug click handling, and verify layout behavior.

## Goals

1. **Widget Tree Visualization**: Show hierarchical structure of rendered widgets
2. **Hit-Test Debugging**: Visualize clickable regions with their HitId, HitRegion, and HitData
3. **Layout Inspection**: Display widget bounds, constraints, and size negotiations
4. **Interactive Exploration**: Hover and click to select widgets for detailed inspection
5. **Minimal Overhead**: Lazy evaluation, no impact when disabled

## Architecture

### Integration Point

The UI Inspector integrates at the Frame level, leveraging existing infrastructure:

```
┌──────────────────────────────────────────────────────────────┐
│                        Runtime Loop                           │
│   Event → Model::update() → Model::view() → Frame            │
└──────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────┐
│                     Frame (with Inspector)                    │
│   Buffer + HitGrid + InspectorState                          │
│   ├── Widget renders → registers hits → buffer cells         │
│   └── Inspector overlay → reads HitGrid → draws annotations  │
└──────────────────────────────────────────────────────────────┘
```

### Core Types

```rust
// crates/ftui-widgets/src/inspector.rs

/// Inspector display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorMode {
    #[default]
    Off,
    /// Show hit regions with colored overlays
    HitRegions,
    /// Show widget boundaries and names
    WidgetBounds,
    /// Show both hit regions and widget bounds
    Full,
}

/// Information about a widget for inspector display
#[derive(Debug, Clone)]
pub struct WidgetInfo {
    /// Human-readable widget name (e.g., "List", "Button")
    pub name: String,
    /// Allocated render area
    pub area: Rect,
    /// Hit ID if widget is interactive
    pub hit_id: Option<HitId>,
    /// Registered hit regions within this widget
    pub hit_regions: Vec<(Rect, HitRegion, HitData)>,
    /// Render time in microseconds (if profiling enabled)
    pub render_time_us: Option<u64>,
    /// Child widgets (for tree view)
    pub children: Vec<WidgetInfo>,
}

/// Inspector overlay state (shared across frames)
#[derive(Debug)]
pub struct InspectorState {
    /// Current display mode
    pub mode: InspectorMode,
    /// Mouse position for hover detection
    pub hover_pos: Option<(u16, u16)>,
    /// Selected widget (clicked)
    pub selected: Option<HitId>,
    /// Collected widget info for current frame
    pub widgets: Vec<WidgetInfo>,
    /// Show detailed panel
    pub show_detail_panel: bool,
}

/// Configuration for inspector appearance
#[derive(Debug, Clone)]
pub struct InspectorStyle {
    /// Border colors for widget bounds (cycles through for nesting)
    pub bound_colors: [PackedRgba; 6],
    /// Hit region overlay color (semi-transparent)
    pub hit_overlay: PackedRgba,
    /// Hovered hit region color
    pub hit_hover: PackedRgba,
    /// Selected widget highlight
    pub selected_highlight: PackedRgba,
    /// Label text color
    pub label_fg: PackedRgba,
    /// Label background color
    pub label_bg: PackedRgba,
}
```

### Default Style

```rust
impl Default for InspectorStyle {
    fn default() -> Self {
        Self {
            bound_colors: [
                PackedRgba::rgb(255, 100, 100),  // Red
                PackedRgba::rgb(100, 255, 100),  // Green
                PackedRgba::rgb(100, 100, 255),  // Blue
                PackedRgba::rgb(255, 255, 100),  // Yellow
                PackedRgba::rgb(255, 100, 255),  // Magenta
                PackedRgba::rgb(100, 255, 255),  // Cyan
            ],
            hit_overlay: PackedRgba::rgba(255, 165, 0, 80),   // Orange 30%
            hit_hover: PackedRgba::rgba(255, 255, 0, 120),    // Yellow 47%
            selected_highlight: PackedRgba::rgba(0, 200, 255, 150), // Cyan 60%
            label_fg: PackedRgba::WHITE,
            label_bg: PackedRgba::rgba(0, 0, 0, 200),
        }
    }
}
```

## UX Flow

### 1. Activation

| Key | Action |
|-----|--------|
| `F12` | Toggle inspector on/off |
| `Ctrl+Shift+I` | Alternative toggle (familiar from browser devtools) |

When activated, the inspector overlays the current view without disrupting the underlying UI state.

### 2. Mode Cycling

| Key | Action |
|-----|--------|
| `i` | Cycle through modes: Off → HitRegions → WidgetBounds → Full → Off |
| `1` | Jump to HitRegions mode |
| `2` | Jump to WidgetBounds mode |
| `3` | Jump to Full mode |
| `0` | Turn off inspector |

### 3. Navigation

| Key | Action |
|-----|--------|
| Mouse hover | Highlight widget/hit region under cursor |
| Mouse click | Select widget for detailed inspection |
| `Escape` | Clear selection |
| `Tab` | Cycle to next widget in tree order |
| `Shift+Tab` | Cycle to previous widget |
| `Enter` | Expand/collapse selected widget's children in detail panel |

### 4. Detail Panel

| Key | Action |
|-----|--------|
| `d` | Toggle detail panel visibility |
| `↑`/`↓` | Scroll detail panel content |
| `c` | Copy selected widget info to clipboard (if clipboard feature enabled) |

### 5. Visual Layers (in Full mode)

| Key | Action |
|-----|--------|
| `h` | Toggle hit regions overlay |
| `b` | Toggle widget bounds overlay |
| `n` | Toggle widget name labels |
| `t` | Toggle render time display |

## Visual Design

### Hit Region Overlay

```
┌─────────────────────────────────────────────────────────────┐
│  Normal View                                                 │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ List Widget                                              ││
│  │ ┌─────────────────────────────────────────────────────┐ ││
│  │ │ Item 1                                      [btn]   │ ││
│  │ ├─────────────────────────────────────────────────────┤ ││
│  │ │ Item 2                                      [btn]   │ ││
│  │ ├─────────────────────────────────────────────────────┤ ││
│  │ │ Item 3                                      [btn]   │ ││
│  │ └─────────────────────────────────────────────────────┘ ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Inspector: HitRegions Mode                                  │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ List Widget                     ┌── Content (orange) ──┐││
│  │ ┌───────────────────────────────│─────────────────────┐│││
│  │ │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│▓▓▓▓▓▓▓▓▓▓▓▓▓▓[████]││││
│  │ ├───────────────────────────────│─────────────────────┤│││
│  │ │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│▓▓▓▓▓▓▓▓▓▓▓▓▓▓[████]│││← Button
│  │ ├───────────────────────────────│─────────────────────┤│││  (cyan)
│  │ │░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│░░░░░░░░░░░░░░[████]││││
│  │ └───────────────────────────────│─────────────────────┘│││
│  └─────────────────────────────────└──────────────────────┘┘│
│                                    ▲ Hover = yellow highlight│
└─────────────────────────────────────────────────────────────┘
```

### Widget Bounds Overlay

```
┌─────────────────────────────────────────────────────────────┐
│  Inspector: WidgetBounds Mode                                │
│  ┌─[List]─────────────────────────────────────────────────┐ │
│  │ ┌─[ListItem]────────────────────────────────────────┐  │ │
│  │ │ ┌─[Text]──────────────────────┐ ┌─[Button]──────┐ │  │ │
│  │ │ │ Item 1                      │ │ Click Me      │ │  │ │
│  │ │ └─────────────────────────────┘ └───────────────┘ │  │ │
│  │ └───────────────────────────────────────────────────┘  │ │
│  │ ┌─[ListItem]────────────────────────────────────────┐  │ │
│  │ │ ┌─[Text]──────────────────────┐ ┌─[Button]──────┐ │  │ │
│  │ │ │ Item 2                      │ │ Click Me      │ │  │ │
│  │ │ └─────────────────────────────┘ └───────────────┘ │  │ │
│  │ └───────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘│
│  ^ Colors cycle per nesting level (red → green → blue → ...)│
└─────────────────────────────────────────────────────────────┘
```

### Detail Panel

When a widget is selected (clicked), show a detail panel:

```
┌──────────────────────────────────────────┬──────────────────┐
│  Main UI                                  │  Inspector Panel │
│                                          │                  │
│  [Selected widget highlighted]           │ ┌──────────────┐ │
│                                          │ │ Widget: List │ │
│  ┌────────────────────────────────────┐  │ │ ID: 0x1001   │ │
│  │ ████████████████████████████████  │  │ │              │ │
│  │ ████████████████████████████████  │  │ │ Area:        │ │
│  │ ████████████████████████████████  │  │ │  x: 10       │ │
│  └────────────────────────────────────┘  │ │  y: 5        │ │
│                                          │ │  w: 40       │ │
│                                          │ │  h: 12       │ │
│                                          │ │              │ │
│                                          │ │ Hit Regions: │ │
│                                          │ │  3 Content   │ │
│                                          │ │  3 Button    │ │
│                                          │ │              │ │
│                                          │ │ Render: 42µs │ │
│                                          │ └──────────────┘ │
└──────────────────────────────────────────┴──────────────────┘
```

## Implementation Plan

### Phase 1: Core Types (bd-17h9.2)

1. Create `crates/ftui-widgets/src/inspector.rs`
2. Define `InspectorMode`, `InspectorState`, `InspectorStyle`
3. Create `InspectorOverlay` widget that renders overlays
4. Add exports to `ftui-widgets/src/lib.rs`

### Phase 2: Hit-Test Visualization (bd-17h9.3)

1. Implement `render_hit_regions()` method
2. Read from `frame.hit_grid` and overlay colored cells
3. Handle hover highlighting via mouse position
4. Integrate with demo-showcase

### Phase 3: Widget Bounds Visualization (bd-17h9.4)

1. Create `WidgetRegistry` to collect widget info during render
2. Implement `render_widget_bounds()` with colored borders
3. Show widget name labels
4. Handle nesting with color cycling

### Phase 4: Detail Panel (bd-17h9.5)

1. Create `InspectorPanel` widget for side panel
2. Display selected widget info
3. Implement tree navigation

### Phase 5: Demo Integration (bd-17h9.6)

1. Add inspector as demo-showcase feature
2. Wire up keybindings
3. Ensure works with all screen types

### Phase 6: E2E Tests (bd-17h9.7-9)

1. Snapshot tests for each mode
2. Input handling tests
3. Performance regression tests

## Keybinding Summary

| Key | Action | Context |
|-----|--------|---------|
| `F12` | Toggle inspector | Global |
| `Ctrl+Shift+I` | Toggle inspector | Global |
| `i` | Cycle mode | Inspector active |
| `0`-`3` | Jump to mode | Inspector active |
| `h` | Toggle hit regions | Inspector active |
| `b` | Toggle widget bounds | Inspector active |
| `n` | Toggle names | Inspector active |
| `t` | Toggle times | Inspector active |
| `d` | Toggle detail panel | Inspector active |
| `Tab` | Next widget | Inspector active |
| `Shift+Tab` | Previous widget | Inspector active |
| `Enter` | Expand/collapse | Detail panel |
| `Escape` | Clear selection | Inspector active |
| `↑`/`↓` | Scroll panel | Detail panel |
| `c` | Copy widget info | Widget selected |

## Performance Considerations

1. **Lazy collection**: Only collect widget info when inspector is active
2. **Frame-local state**: Clear widget registry each frame to avoid stale data
3. **Efficient overlay**: Use buffer's existing cells, just add overlay colors
4. **Hit grid reuse**: Don't duplicate hit test data; read from Frame's HitGrid

## Testing Plan

### Unit Tests

- `InspectorMode` cycling logic
- `InspectorStyle` defaults
- `WidgetInfo` construction

### Integration Tests

- Overlay renders correctly over content
- Mouse hover updates highlight
- Selection persists across frames
- Mode transitions work

### Snapshot Tests

- Each mode with sample widget tree
- Detail panel with various widget types
- Theme variations (dark/light)

### E2E Tests

- Full activation → navigation → deactivation flow
- Performance: <1ms overhead when disabled
- Clipboard integration (if available)

## Usage + Demo Presets (Harness)

The inspector demo is implemented in the harness view `widget-inspector`. This view renders a deterministic widget tree plus hit regions so the overlay can be verified in screenshots and PTY captures.

### Quick Demo (Interactive)

```bash
FTUI_HARNESS_VIEW=widget-inspector \
FTUI_HARNESS_SCREEN_MODE=inline \
FTUI_HARNESS_UI_HEIGHT=10 \
FTUI_HARNESS_SUPPRESS_WELCOME=1 \
cargo run -p ftui-harness
```

### Preset Sizes (PTY Capture Friendly)

- **120x40 (wide)**: `PTY_COLS=120 PTY_ROWS=40`
- **80x24 (classic)**: `PTY_COLS=80 PTY_ROWS=24`

Example smoke run with auto-exit:

```bash
PTY_COLS=120 PTY_ROWS=40 \
FTUI_HARNESS_VIEW=widget-inspector \
FTUI_HARNESS_EXIT_AFTER_MS=1200 \
cargo run -p ftui-harness
```

## E2E Script + JSONL Logging

The canonical E2E script lives at `tests/e2e/scripts/test_ui_inspector.sh`. It runs two PTY captures (120x40 + 80x24) and emits JSONL logs containing environment, capabilities, timings, seed, and output checksums.

```bash
E2E_HARNESS_BIN=target/debug/ftui-harness \
E2E_LOG_DIR=/tmp/ftui_e2e_logs \
E2E_RESULTS_DIR=/tmp/ftui_e2e_results \
tests/e2e/scripts/test_ui_inspector.sh
```

JSONL output (default: `/tmp/ftui_e2e_results/ui_inspector.jsonl`) includes:

- `run_id`, `case`, `status`
- `duration_ms`, `output_bytes`, `output_sha256`
- `seed`, `view`, `cols`, `rows`
- `term`, `colorterm`, `no_color`
- `capabilities` (screen mode, input mode, ui height, mouse/focus/kitty flags)

Deterministic mode: keep the harness flags stable (`FTUI_HARNESS_SCREEN_MODE`, `FTUI_HARNESS_UI_HEIGHT`, input + capability toggles) and fix `FTUI_HARNESS_EXIT_AFTER_MS` to limit runtime jitter.

## Evidence Ledger (Docs + Demo)

- **Demo target**: `ftui-harness` `widget-inspector` view because it supplies a deterministic widget tree + hit regions that exercise overlay rendering and panel text.
- **Preset sizes**: 120x40 (wide) + 80x24 (classic) to cover label wrapping and panel constraints at common terminal sizes.
- **Smoke assertions**: Check for `Inspector`, `Region:`, and `LogPanel` strings in the PTY capture to confirm overlay header, hit region labels, and widget tree naming are present.

## Invariants

1. Inspector MUST NOT mutate underlying widget state
2. Inspector MUST NOT interfere with event handling when off
3. Inspector overlays MUST be drawn after all widget rendering
4. Inspector MUST handle zero-size and off-screen widgets gracefully
5. Inspector MUST work with any DegradationLevel

## Failure Modes

| Condition | Behavior |
|-----------|----------|
| HitGrid not enabled | Show warning, disable hit regions mode |
| No widgets registered | Show "No widgets" message |
| Mouse outside frame | Clear hover, keep selection |
| Selected widget removed | Clear selection, show notice |
| Render time tracking off | Hide timing column |

## Future Extensions

1. **Widget Tree Panel**: Collapsible tree view of all widgets
2. **Property Editor**: Edit widget properties live
3. **Style Inspector**: Show computed styles
4. **Event Debugger**: Show event propagation path
5. **Performance Profiler**: Flame graph of render times
