# PLAN_TO_CREATE_FRANKENTUI__CODEX.md

## Executive Summary
FrankenTUI (ftui) is a deliberately minimal, high-performance terminal UI kernel that fuses
three Rust stacks into one coherent system:
- rich_rust: segment pipeline, theming, layout measurement, export
- charmed_rust: Bubbletea runtime, Lipgloss ergonomics, widgets
- opentui_rust: diff renderer, optimized buffers, input parser

This is not a port. It is a new, optimal kernel that is:
- flicker-free by construction
- inline-first (native scrollback by default)
- safe-only (no unsafe in ftui crates)
- deterministic and testable
- fast under real loads

This document is the authoritative blueprint: architecture, invariants, APIs,
algorithms, performance budgets, migration strategy, and decision gates.

## Mission and Non-Goals
### Mission
- A tiny kernel that powers both classic TUIs and agent harness UIs.
- A stable, minimal API surface that is easy to reason about.
- A rendering engine that is always diffed, buffered, and cursor-correct.

### Non-Goals
- Backwards compatibility with upstream libraries.
- A monolithic crate with every widget and integration bundled.
- Any unsafe code in ftui crates.

## Deep Source Synthesis (what survives)
### rich_rust (concrete takeaways)
- Renderables output `Segment` streams; optional control codes are embedded in segments.
- `Text` uses Span ranges with overlap precedence.
- ThemeStack is a push/pop stack with optional inheritance.
- Console hooks exist (Live uses a RenderHook to intercept segments).
- Measurement trait produces min/max width for layout.
- Style uses explicit attribute masks for deterministic merges.

### charmed_rust (concrete takeaways)
- Bubbletea Program handles raw mode + alt screen + event loop.
- Model is `init -> update -> view` with view returning String.
- Message type is type-erased, with built-in system messages.
- Program supports custom IO (SSH/remote input injection).
- Lipgloss Style is fluent and also carries layout (padding/margin/border).
- Theme uses semantic slots (primary, background, text, etc).

### opentui_rust (concrete takeaways)
- Renderer uses double buffer with diff + cached scratch buffers.
- `Cell` stores `CellContent` + Style; GraphemeId encodes width in bits.
- Buffer supports scissor + opacity stacks and alpha blending.
- AnsiWriter tracks cursor, fg/bg, attrs to minimize output.
- Input parser supports CSI sequences, bracketed paste, size limits.
- HitGrid provides fast mouse hit testing.

## System Model (mental model)
Three rings:
1) Kernel: Renderer + Buffer + Style + Text + Runtime + Event.
2) Widgets: reusable components built on the kernel.
3) Extras: markdown, syntax, forms, SSH, export.

Kernel is sacred: tiny, stable, deterministic.

## Kernel Architecture (workspace layout)
1) ftui-core
   - terminal IO, raw mode lifecycle, capability detection
   - input parsing into canonical Event
   - timing utilities and signals

2) ftui-render
   - Buffer/Frame, Cell, GraphemePool, LinkPool, HitGrid
   - diff engine + Renderer::present
   - optional threaded renderer

3) ftui-style
   - Style/Color/Theme, ANSI encoding, color downgrade
   - theme stack + semantic style resolution
   - style -> ANSI caching

4) ftui-text
   - Text/Span/Segment, wrapping, truncation, alignment
   - accurate width measurement

5) ftui-layout
   - Rect, constraints, layout engine (row/column/grid)
   - measurement protocol (min/max width)

6) ftui-runtime
   - Program + Model + Message + Cmd + scheduler
   - deterministic simulator

7) ftui-widgets (feature-gated)
   - ported widgets and new components

8) ftui-extras (feature-gated)
   - markdown, syntax, forms, SSH, export

## Kernel Invariants (must always hold)
- All output is diffed and buffered (no raw full-screen writes).
- Inline mode never clears the full screen.
- Grapheme width accounting is correct for all drawn glyphs.
- Style resolution is deterministic and order-preserving.
- Event parsing is lossless for supported sequences.
- No unsafe blocks in ftui crates.

## Key Design Decisions (resolve and lock early)
1) **Frame is the canonical render target**.
   - Segment pipeline is a text/layout intermediate, not the final target.
   - All rendering converges on `Frame` to enable diff.

2) **Style is purely visual**.
   - Layout concerns (padding/margin/border) live in widgets/layout layer.
   - This avoids conflating text styling with box layout.

3) **Message is typed, system events are always injectable**.
   - `type Message: From<Event> + Send + 'static`.
   - Keeps ergonomics while preserving runtime control.

4) **Inline mode is default**.
   - AltScreen is optional and explicit.
   - Inline mode has a strict cursor policy.

5) **Unsafe is forbidden**.
   - If raw mode needs unsafe, wrap it in a non-ftui helper crate.

## Canonical Type Definitions (Rust-ish spec)
### Input events
```
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize { width: u16, height: u16 },
    Paste(PasteEvent),
    Focus(bool),
    Clipboard(ClipboardEvent),
}

pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}

pub enum KeyCode {
    Char(char),
    Enter, Esc, Tab, Backspace,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    Insert, Delete,
    F(u8),
}

bitflags! { pub struct KeyModifiers: u8 { SHIFT | ALT | CTRL | SUPER } }

pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
    pub x: u16,
    pub y: u16,
}
```

### Rendering types
```
pub struct Frame {
    buffer: Buffer,
    hit_grid: Option<HitGrid>,
}

pub struct Cell {
    pub content: CellContent,
    pub style: Style,
}

pub enum CellContent {
    Char(char),
    Grapheme(GraphemeId),
    Empty,
    Continuation,
}

pub struct Renderer {
    front: Buffer,
    back: Buffer,
    ansi: AnsiWriter,
    link_pool: LinkPool,
    grapheme_pool: GraphemePool,
}
```

### Styling types
```
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attrs: Attrs,
    pub mask: Attrs,
    pub link_id: Option<u32>,
    pub meta: Option<Vec<u8>>,
}

pub struct ThemeStack { ... }
```

### Text types
```
pub struct Text { plain: String, spans: Vec<Span>, ... }

pub struct Span { start: usize, end: usize, style: Style }

pub struct Segment { text: Cow<'a, str>, style: Option<Style> }
```

## Data Flow (end-to-end)
Input:
```
stdin bytes -> InputParser -> Event -> Program -> Model::update -> Cmd -> Message -> ...
```
Output:
```
Model::view -> Frame -> Renderer::present (diff) -> ANSI -> stdout
```
Text:
```
Text/Span -> Segment stream -> Frame draw -> Renderer diff -> stdout
```

## Rendering Engine (deep design)
### Buffer Model
- Flat Vec<Cell> for cache locality.
- GraphemePool with cached width (hot path avoids string lookups).
- Scissor + opacity stacks for compositing.
- Alpha blending option for overlays (opt-in).

### GraphemeId Encoding (from opentui)
- 24 bits for pool id, 7 bits for width, 1 reserved bit.
- Width cached in id to avoid pool lookup during render.

### Diff Engine
- Compare front/back buffers with bits_eq (fast integer comparison).
- Reuse scratch buffers and pre-allocated vectors.
- Optional dirty-region tracking for sub-rect diff.
- Emit updates grouped by rows and style runs to minimize ANSI churn.

### Output Strategy
- Buffer ANSI into a single write per frame.
- Track cursor state to avoid redundant moves.
- Explicit handling for hyperlink open/close (OSC 8).
- Separate cursor moves from cell writes for speed.

### Threaded Renderer (feature-gated)
- Main thread builds buffers.
- Render thread performs diff + output.
- Channel-based buffer swap.
- Shutdown barrier ensures terminal cleanup.

## Input System
- Parse bytes into Event (Key/Mouse/Resize/Paste/Focus).
- Key normalization: stable Key + modifiers, CSI mapping.
- Hard bounds on sequence length to avoid DoS.
- Coalesce noisy streams (mouse move, resize) optionally.
- Explicit event injection for tests and remote IO.

## Style System (merged semantics)
### Requirements
- Lipgloss-style fluent builder ergonomics.
- rich_rust-style explicit attribute masks for deterministic merges.
- ThemeStack for semantic styles.
- Color profiles with downgrade.
- Hyperlinks (OSC 8) supported via link IDs.

### Proposed Style structure
- `fg: Option<Color>`, `bg: Option<Color>`
- `attrs: Attrs`, `mask: Attrs` (explicit set mask)
- `link_id: Option<u32>`
- `meta: Option<Vec<u8>>`

### Style merge algorithm
- If `rhs.mask` includes attr bit, it overrides `lhs.attrs`.
- Colors override only when Some(...).
- link_id overrides only when Some(...).

### Theme system
- ThemeStack push/pop with optional inheritance.
- Semantic slots (primary, accent, muted, error, surface, text).
- Named styles mapped to slots and widget defaults.

## Text + Layout
### Text Pipeline
- Text contains plain string + spans.
- Overlapping spans resolved deterministically (later wins).
- Segment stream is boundary between text and renderer.
- Wrapping/truncation uses accurate grapheme width.

### Layout
- Measurement protocol: min/max width in cells.
- Constraints: fixed, percentage, expand.
- Row/column/grid layouts.
- Widgets implement measure() and render(rect, frame).

## Runtime Model (Bubbletea lineage)
### Model contract
```
trait Model {
    type Message: From<Event> + Send + 'static;
    fn init(&self) -> Option<Cmd<Self::Message>> { None }
    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message>;
    fn view(&self, frame: &mut Frame);
}
```

### Commands
- Cmd::none, Cmd::quit, Cmd::batch, Cmd::sequence
- Async commands feature-gated (thread pool or tokio)
- Tick commands for animation

### Scheduler
- Frame-rate cap (fps).
- Render on event or when dirty.
- Option to force render on tick.

### Simulator
- Deterministic ProgramSimulator with injected events and captured frames.

## Inline Mode (native scrollback) vs AltScreen
### Screen modes
- Inline (default): preserves scrollback.
- AltScreen: full-screen mode.

### Inline policy
- UI rendered into bounded region (top or bottom anchored).
- Cursor restored after render.
- Distinguish "log area" (append-only) vs "UI area".

Inline render sequence:
1) Save cursor
2) Move to UI region anchor
3) Clear UI region lines
4) Render UI frame
5) Restore cursor

## API Ergonomics (target surface)
### Minimal example
```rust
use ftui::{App, Cmd, Event, Frame, Model, Style, Text};

struct AppState { count: u64 }

impl Model for AppState {
    type Message = Event;

    fn update(&mut self, msg: Event) -> Cmd<Event> {
        match msg {
            Event::Key(k) if k.is_char('q') => Cmd::quit(),
            Event::Key(k) if k.is_char('+') => { self.count += 1; Cmd::none() }
            _ => Cmd::none(),
        }
    }

    fn view(&self, frame: &mut Frame) {
        let t = Text::new(format!("count: {}", self.count))
            .style(Style::new().bold());
        frame.draw_text(0, 0, &t);
    }
}

fn main() -> ftui::Result<()> {
    App::new(AppState { count: 0 })
        .screen_mode(ftui::ScreenMode::Inline)
        .run()
}
```

### Optional string adapter
For trivial apps:
```
fn view_string(&self) -> String
```
Internally becomes Text -> Segment -> Frame render.

## Widget Priorities (v1)
- Viewport
- List
- Table
- Panel/Box
- TextInput
- TextArea
- Progress
- Spinner
- Tabs
- Tree

## Export/Extras
- HTML/SVG export via segment pipeline.
- Markdown renderer (glamour).
- Syntax highlighting (feature-gated).
- Forms (huh).
- SSH integration (wish).

## Performance Budgets (explicit)
- 120x40 diff present: < 1 ms.
- Input parse + dispatch: < 100 us/event.
- Wrap 200 lines: < 2 ms.
- Buffer allocation: amortized zero.

## Testing Strategy
- Unit tests: diff engine, grapheme width, style rendering.
- Property tests: diff correctness, wrap invariants.
- Snapshot tests: widgets, markdown, theme output.
- PTY tests: raw mode, input parsing, cursor control.
- Simulator tests: deterministic update/view cycles.
- Perf tests: diff + render budgets.

## Migration Map (source -> ftui)
### rich_rust -> ftui
- Text/Span/Segment -> ftui-text
- ThemeStack -> ftui-style
- Renderables -> ftui-widgets
- Export -> ftui-extras

### charmed_rust -> ftui
- Program/Model/Cmd -> ftui-runtime
- Lipgloss Style -> ftui-style (merged semantics)
- Bubbles -> ftui-widgets
- Glamour -> ftui-extras
- ProgramSimulator -> ftui-runtime

### opentui_rust -> ftui
- Renderer/Buffer/Cell/GraphemePool -> ftui-render
- Input parser + caps -> ftui-core
- HitGrid -> ftui-render
- Threaded renderer -> ftui-render (feature)

## Phased Implementation Plan (detailed)
### Phase 0 - Contracts
- Decide workspace layout.
- Define public API contracts for core types.
- Add README + API overview.

### Phase 1 - Render Kernel
- Port Buffer/Cell/GraphemePool.
- Implement diff engine + Renderer::present.
- Add ScreenMode and Inline policy.
- Minimal ANSI writer + output buffering.

Exit criteria:
- Render a static Frame without flicker.
- Inline mode preserves scrollback.

### Phase 2 - Input + Terminal
- Port input parser and capability detection.
- Implement raw mode lifecycle (safe crates).
- Unify Event type.

Exit criteria:
- Key/mouse/resize events flow into Program.

### Phase 3 - Style + Text
- Implement Style/Color/Theme stack.
- Add Text/Span/Segment pipeline.
- Measurement helpers (width/height).

Exit criteria:
- Styled text renders correctly with wrapping and alignment.

### Phase 4 - Runtime
- Implement Program + Model + Cmd.
- Add scheduler for async/tick commands.
- Implement ProgramSimulator.

Exit criteria:
- Deterministic update/view loop with snapshot tests.

### Phase 5 - Layout + Widgets
- Implement layout primitives.
- Port core widgets (viewport/list/table/input/textarea).
- Add hit testing (optional).

Exit criteria:
- Widgets render correctly and pass snapshot tests.

### Phase 6 - Extras + Export
- Markdown renderer, syntax highlighting, forms.
- Export to HTML/SVG via segment pipeline.

Exit criteria:
- Export produces stable outputs suitable for docs/tests.

### Phase 7 - Stabilization
- Performance baselines + CI enforcement.
- Reference demos (agent harness, markdown pager, mini editor).

## Open Questions (resolve early)
- Inline mode: exact cursor restore policy and safe region anchoring.
- Style model: precise rule order when spans overlap.
- Raw mode crate choice: crossterm vs termion vs custom safe wrapper.
- Async commands: thread pool vs tokio feature split.

## Definition of Done (v1)
- Inline mode default and stable.
- Flicker-free diff renderer with minimal output.
- Unified Style/Text/Theme.
- Bubbletea runtime integrated with Frame rendering.
- Core widgets shipped and tested.
- Performance budgets enforced.

## Immediate Next Steps
1) Decide crate layout (single crate vs workspace)
2) Create stub modules with public contracts
3) Port Buffer/Cell/GraphemePool into ftui-render
4) Build minimal Renderer::present output to stdout
5) Prove Inline mode with a simple demo (box + text) without flicker

## Essence of the Kernel (what must be perfect)
The kernel is the intersection of three pipelines:
1) **Input -> Event -> Update** (runtime discipline)
2) **State -> Frame** (rendering discipline)
3) **Frame -> Diff -> Output** (performance discipline)

If these three are perfect, everything else is optional.

## Agent Harness Considerations (first-class target)
- Inline mode must never destroy scrollback.
- Output must never flicker even when partial updates occur.
- A log stream and a UI stream must coexist without corruption.
- External tools (SSH, PTY) must be able to inject events.

## Detailed Algorithms (appendix)
### Diff algorithm (conceptual)
```
fn compute_diff(old: &Buffer, new: &Buffer) -> Diff {
    assert_eq!(old.size, new.size);
    for each cell in grid:
        if !old[cell].bits_eq(new[cell]):
            record changed cell
    merge changed cells into row regions
    return diff
}
```

### Inline rendering algorithm
```
fn render_inline(ui_frame: &Frame, ui_height_prev: u16) {
    save_cursor();
    move_cursor_up(ui_height_prev);
    clear_lines(ui_height_prev);
    draw_ui_frame(ui_frame);
    restore_cursor();
}
```

### Span resolution algorithm
```
fn resolve_spans(spans: &[Span]) -> Vec<StyledRun> {
    sort spans by start asc, then by priority
    for each char index:
        apply latest span that covers index
    coalesce adjacent runs with same style
}
```

### Layout distribution (min/max protocol)
```
fn distribute(columns, width) {
    // 1) sum mins, sum maxes
    // 2) if width < sum_min -> shrink proportionally
    // 3) if width > sum_max -> expand proportionally
    // 4) clamp within min/max
}
```

## API Surface (expanded sketch)
### Renderer
- `Renderer::new(width, height, options)`
- `Renderer::present()`
- `Renderer::buffer()`
- `Renderer::clear(color)`
- `Renderer::stats()`
- `Renderer::set_screen_mode(ScreenMode)`

### Frame
- `draw_text(x, y, &Text)`
- `fill_rect(x, y, w, h, style)`
- `draw_box(rect, BoxStyle)`
- `push_scissor(rect)` / `pop_scissor()`
- `push_opacity(alpha)` / `pop_opacity()`

### Text
- `Text::new(str)`
- `Text::styled(str, Style)`
- `Text::append(str)`
- `Text::append_styled(str, Style)`
- `Text::wrap(width)`

### Style
- `Style::new()`
- `Style::fg(Color)` / `Style::bg(Color)`
- `Style::bold()` / `Style::italic()` / `Style::underline()`
- `Style::link(url)`

### Program
- `Program::new(model)`
- `Program::run()`
- `Program::with_custom_io()`
- `Program::with_alt_screen()`
- `Program::with_fps(n)`

## Testing Depth (what we enforce)
- Diff correctness with randomized buffers (property tests)
- Cursor correctness in Inline mode (PTY tests)
- Grapheme width handling for emoji and ZWJ sequences
- Style merge precedence tests
- Renderer output length benchmarks
- Widget snapshots across themes

## Decision Gates (explicit)
- **Gate 1:** Frame + diff renderer produces stable output in Inline mode.
- **Gate 2:** Event parser handles bracketed paste + mouse + resize robustly.
- **Gate 3:** Style/Theme system demonstrates deterministic merges.
- **Gate 4:** ProgramSimulator produces stable snapshots across runs.

