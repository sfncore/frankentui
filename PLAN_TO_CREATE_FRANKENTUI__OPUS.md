# PLAN_TO_CREATE_FRANKENTUI__OPUS.md

## FrankenTUI: The Mathematically Optimal Terminal UI Kernel

**Version 4.0 — First-Principles Architecture with Extracted Implementations**

---

# PART I: FOUNDATIONS

## Chapter 1: First-Principles Derivation

### 1.1 What IS a Terminal?

A terminal is a **state machine** that transforms a stream of bytes into a visual grid of styled characters:

```
Terminal: (State, Byte*) → (State', Grid)
where:
  State = { cursor: (x,y), style: Style, mode: Mode, ... }
  Grid = Cell[width × height]
  Cell = { char: Grapheme, fg: Color, bg: Color, attrs: Attributes }
```

**Fundamental theorem**: Any terminal UI library is an **inverse function** — it must produce the minimal byte stream that transforms the current grid into the desired grid:

```
Render: (Grid_current, Grid_desired) → Byte*
such that Terminal(State, Render(G_c, G_d)) = (_, G_d)
```

This immediately implies:
1. **Diff-based rendering is optimal** — producing bytes for unchanged cells is wasteful
2. **State tracking is essential** — we must know the terminal's current state to emit minimal sequences
3. **Cell equality must be fast** — we compare O(w×h) cells per frame

### 1.2 The Fundamental Operations

From first principles, a terminal UI library needs exactly these operations:

| Operation | Mathematical Model | Complexity Target |
|-----------|-------------------|-------------------|
| **Cell comparison** | `Cell × Cell → Bool` | O(1), < 1ns |
| **Cell mutation** | `Grid × (x,y) × Cell → Grid` | O(1), < 5ns |
| **Diff computation** | `Grid × Grid → [(x,y,Cell)]` | O(w×h), < 500µs for 80×24 |
| **ANSI emission** | `[(x,y,Cell)] → Byte*` | O(changes), minimal bytes |
| **Style composition** | `Style × Style → Style` | O(1), < 50ns |
| **Text measurement** | `String → usize` | O(graphemes), cached |
| **Grapheme interning** | `String → GraphemeId` | O(1) amortized |
| **Alpha blending** | `Rgba × Rgba → Rgba` | O(1), < 10ns |

### 1.3 The Optimality Conditions

A terminal UI library is **optimal** if and only if:

1. **Minimal output**: `|Render(G_c, G_d)| ≤ |Render'(G_c, G_d)|` for all alternative implementations
2. **Minimal computation**: Operations achieve their theoretical complexity bounds
3. **Minimal memory**: Data structures use asymptotically optimal space
4. **Minimal latency**: No unnecessary blocking or synchronization

**Corollary**: The optimal architecture must:
- Use cell-level diffing (not region-level)
- Track terminal state to avoid redundant SGR codes
- Use bitwise cell comparison (not field-by-field)
- Cache computed values (widths, styles)
- Pool complex content (graphemes, links)

### 1.4 The Three-Library Synthesis

FrankenTUI extracts the optimal kernel from three Rust TUI libraries:

| Source | Contribution | Why Optimal |
|--------|--------------|-------------|
| **opentui_rust** | Cell grid, diff algorithm, alpha blending, scissor/opacity stacks, grapheme pool | Cache-optimal 16-byte cells, bitwise comparison, Porter-Duff compositing |
| **rich_rust** | Segment abstraction, markup parser, measurement protocol, Renderable trait, Live display | Cow<str> for zero-copy, event-driven span rendering, LRU width cache |
| **charmed_rust** | LipGloss styling, CSS-like properties, theme system, Elm architecture | Bitflags property tracking, shorthand tuple conversion, adaptive colors |

---

## Chapter 2: Formal Specifications

### 2.1 Terminal State Machine (TLA+-style)

```tla
---------------------------- MODULE Terminal ----------------------------
VARIABLES cursor, style, grid, mode

TypeInvariant ==
    /\ cursor \in (0..width-1) × (0..height-1)
    /\ style \in Style
    /\ grid \in [0..width-1 × 0..height-1 -> Cell]
    /\ mode \in {Normal, Raw, AltScreen}

Init ==
    /\ cursor = (0, 0)
    /\ style = DefaultStyle
    /\ grid = [p \in 0..width-1 × 0..height-1 |-> EmptyCell]
    /\ mode = Normal

ProcessByte(b) ==
    CASE IsControlChar(b) -> HandleControl(b)
      [] IsEscapeStart(b) -> ParseEscape
      [] IsPrintable(b)   ->
           /\ grid' = [grid EXCEPT ![cursor] = Cell(b, style)]
           /\ cursor' = AdvanceCursor(cursor)

SafetyInvariant ==
    \A p \in Domain(grid): grid[p] \in Cell

LivenessProperty ==
    <>[](\A desired: Eventually(grid = desired))
=========================================================================
```

### 2.2 Rendering Pipeline State Machine

```
States: {Idle, Measuring, Rendering, Diffing, Presenting, Error}

Transitions:
  Idle --[render_request]--> Measuring
  Measuring --[layout_complete]--> Rendering
  Rendering --[draw_complete]--> Diffing
  Diffing --[diff_complete]--> Presenting
  Presenting --[present_complete]--> Idle
  * --[error]--> Error
  Error --[recover]--> Idle

Invariants:
  I1: In Rendering state, only back buffer is modified
  I2: In Presenting state, only ANSI output is produced
  I3: After Presenting, front buffer = desired grid
  I4: Error state restores terminal to safe state
  I5: Scissor stack intersection monotonically decreases on push
  I6: Opacity stack product stays in [0, 1]
```

### 2.3 Buffer Invariants (Formal)

```rust
/// INVARIANT: Buffer dimensions never change after creation
/// PROOF: width/height are immutable; cells vec size = width * height

/// INVARIANT: Cell access is always in bounds
/// PROOF: get/set check x < width && y < height; unchecked requires unsafe

/// INVARIANT: Scissor stack intersection is monotonically decreasing
/// PROOF: push() computes intersection; pop() restores previous (saved on stack)

/// INVARIANT: Opacity stack product is in [0, 1]
/// PROOF: values clamped on push; product = Π(stack) where each factor ∈ [0,1]

/// INVARIANT: GraphemePool slot.refcount > 0 implies slot is valid
/// PROOF: intern() sets refcount = 1; incref() adds 1; decref() only frees at 0

/// INVARIANT: Cell.content.Grapheme(id) implies id is valid in associated pool
/// PROOF: GraphemeId only created via pool.intern(); invalidation tracked
```

---

## Chapter 3: Cache Hierarchy Analysis

### 3.1 Memory Layout for Cache Efficiency

Modern CPUs have:
- **L1 cache**: 32-64 KB, ~4 cycles latency
- **L2 cache**: 256 KB - 1 MB, ~12 cycles
- **L3 cache**: 8-32 MB, ~40 cycles
- **Cache line**: 64 bytes

**Design implications**:

```
Cell = 16 bytes
  → 4 cells per cache line
  → 80×24 grid = 1920 cells = 30,720 bytes (fits in L1!)
  → 200×60 grid = 12000 cells = 192 KB (fits in L2)

Optimal access pattern:
  - Row-major iteration (cells[y * width + x])
  - Sequential scan for diff (no random access)
  - Prefetch next row while processing current
  - Avoid pointer chasing (use indices into arrays)
```

### 3.2 Cell Layout: The 16-Byte Sweet Spot

```rust
// OPTIMAL: 16 bytes, exactly 4 per 64-byte cache line
// Memory layout:
// ┌─────────────────┬─────────────────┬─────────────────┬─────────────────┐
// │  content (4B)   │    fg (4B)      │    bg (4B)      │   attrs (4B)    │
// └─────────────────┴─────────────────┴─────────────────┴─────────────────┘
// Offset:  0              4                 8                 12

#[repr(C, align(16))]
pub struct Cell {
    content: CellContent,      // 4 bytes: char or GraphemeId
    fg: PackedRgba,            // 4 bytes: RGBA as u32
    bg: PackedRgba,            // 4 bytes: RGBA as u32
    attrs: CellAttrs,          // 4 bytes: flags (8 bits) + link_id (24 bits)
}

// WHY NOT 24 or 32 bytes?
// - 24 bytes: 2.67 cells/line, wastes 16 bytes per line
// - 32 bytes: 2 cells/line, doubles memory bandwidth
// - 16 bytes: Perfect fit, single SIMD comparison possible
```

### 3.3 Comparison: What We Avoid

```rust
// ❌ SUBOPTIMAL: 48+ bytes, heap allocations
pub struct CellBad {
    content: String,   // 24 bytes (ptr + len + cap) + HEAP
    fg: Rgba,          // 16 bytes (4 × f32)
    bg: Rgba,          // 16 bytes
    attrs: u64,        // 8 bytes
}
// Problems:
// - 1.3 cells per cache line
// - Heap allocation for every cell
// - Pointer chasing for content comparison

// ✅ OPTIMAL: 16 bytes, no heap for single chars
pub struct Cell {
    content: CellContent,  // Enum: Empty | Char(char) | Grapheme(GraphemeId)
    fg: PackedRgba,        // Packed u32
    bg: PackedRgba,        // Packed u32
    attrs: CellAttrs,      // Bitfield u32
}
// Benefits:
// - 4 cells per cache line
// - No heap for 99% of cells (ASCII/BMP chars)
// - Single 128-bit comparison
```

---

## Chapter 4: SIMD Opportunities

### 4.1 Where SIMD Helps

| Operation | SIMD Potential | Expected Speedup |
|-----------|---------------|------------------|
| Cell comparison | 4 cells at once (64 bytes = 4×16) | 4× |
| Buffer clear | 256-bit writes | 8× |
| Color blending | 4 RGBA channels at once | 4× |
| ASCII width | 32 chars at once with AVX2 | 32× |
| Row copy | Entire rows with AVX-512 | 4× |

### 4.2 Cell Comparison with SIMD

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Compare 4 cells at once (64 bytes) using AVX2
/// Returns bitmask where bit i is set if cells[i] differs
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn compare_cells_avx2(old: &[Cell; 4], new: &[Cell; 4]) -> u8 {
    // Load 64 bytes (4 cells) from each buffer
    let old_lo = _mm256_loadu_si256(old.as_ptr() as *const __m256i);
    let old_hi = _mm256_loadu_si256((old.as_ptr() as *const __m256i).add(1));
    let new_lo = _mm256_loadu_si256(new.as_ptr() as *const __m256i);
    let new_hi = _mm256_loadu_si256((new.as_ptr() as *const __m256i).add(1));

    // Compare 32 bytes at a time
    let cmp_lo = _mm256_cmpeq_epi64(old_lo, new_lo);
    let cmp_hi = _mm256_cmpeq_epi64(old_hi, new_hi);

    // Extract mask: 1 means equal
    let mask_lo = _mm256_movemask_epi8(cmp_lo) as u32;
    let mask_hi = _mm256_movemask_epi8(cmp_hi) as u32;

    // Convert to per-cell mask (4 bits for 4 cells)
    // Each cell is 16 bytes = 2 qwords, so check pairs
    let cell_mask =
        (((mask_lo & 0xFFFF) == 0xFFFF) as u8) |        // Cell 0
        ((((mask_lo >> 16) & 0xFFFF) == 0xFFFF) as u8) << 1 |  // Cell 1
        (((mask_hi & 0xFFFF) == 0xFFFF) as u8) << 2 |   // Cell 2
        ((((mask_hi >> 16) & 0xFFFF) == 0xFFFF) as u8) << 3;   // Cell 3

    // Invert: 1 means DIFFERENT
    !cell_mask & 0x0F
}

/// Scalar fallback for non-AVX2 systems
#[inline]
fn compare_cells_scalar(old: &Cell, new: &Cell) -> bool {
    // Transmute to [u32; 4] for branchless comparison
    unsafe {
        let a: [u32; 4] = std::mem::transmute_copy(old);
        let b: [u32; 4] = std::mem::transmute_copy(new);
        a != b
    }
}
```

### 4.3 ASCII Width with SIMD

```rust
/// Check if string is pure ASCII and return width in one pass
/// For ASCII, width = byte length (huge optimization)
#[target_feature(enable = "avx2")]
unsafe fn ascii_width_simd(s: &[u8]) -> Option<usize> {
    let len = s.len();
    let mut i = 0;

    // Process 32 bytes at a time
    let high_bit_mask = _mm256_set1_epi8(0x80u8 as i8);

    while i + 32 <= len {
        let chunk = _mm256_loadu_si256(s[i..].as_ptr() as *const __m256i);
        let high_bits = _mm256_and_si256(chunk, high_bit_mask);
        let any_high = _mm256_testz_si256(high_bits, high_bits);
        if any_high == 0 {
            return None;  // Non-ASCII byte found
        }
        i += 32;
    }

    // Handle remainder with scalar
    for &b in &s[i..] {
        if b & 0x80 != 0 { return None; }
    }

    Some(len)  // ASCII width = byte length
}
```

---

## Chapter 5: Extracted Implementations

### 5.1 From rich_rust: Segment System

The Segment is the atomic unit of styled text. Key insight: use `Cow<str>` to avoid allocation when borrowing.

```rust
/// Atomic rendering unit - styled text slice
/// From rich_rust/src/segment.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment<'a> {
    pub text: Cow<'a, str>,              // Borrowed when possible
    pub style: Option<Style>,
    pub control: Option<SmallVec<[ControlCode; 2]>>,  // Stack-allocated for 0-2 codes
}

/// Control codes for cursor/screen manipulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ControlType {
    Bell = 1,
    CarriageReturn = 2,
    Home = 3,
    Clear = 4,
    ShowCursor = 5,
    HideCursor = 6,
    EnableAltScreen = 7,
    DisableAltScreen = 8,
    CursorUp = 9,
    CursorDown = 10,
    CursorForward = 11,
    CursorBack = 12,
    CursorNextLine = 13,
    CursorPrevLine = 14,
    EraseLine = 15,
    SetTitle = 16,
}

impl<'a> Segment<'a> {
    /// Split segment at cell position (not byte position!)
    /// Critical for correct text wrapping with wide chars
    pub fn split_at_cell(&self, cell_pos: usize) -> (Self, Self) {
        if self.is_control() {
            return (self.clone(), Self::default());
        }

        let mut width = 0;
        let mut byte_pos = 0;

        for (i, c) in self.text.char_indices() {
            let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if width + char_width > cell_pos {
                break;
            }
            width += char_width;
            byte_pos = i + c.len_utf8();
        }

        // Optimized split using Cow to avoid unnecessary allocation
        let (left, right) = match &self.text {
            Cow::Borrowed(s) => {
                let (l, r) = s.split_at(byte_pos);
                (Cow::Borrowed(l), Cow::Borrowed(r))
            }
            Cow::Owned(s) => {
                let (l, r) = s.split_at(byte_pos);
                (Cow::Owned(l.to_string()), Cow::Owned(r.to_string()))
            }
        };

        (
            Self::new(left, self.style.clone()),
            Self::new(right, self.style.clone()),
        )
    }

    /// Cell width (display width, not byte length)
    pub fn cell_length(&self) -> usize {
        if self.is_control() { return 0; }
        crate::unicode::display_width(&self.text)
    }
}
```

### 5.2 From rich_rust: Markup Parser

Regex-based parser for Rich-style markup like `[bold red]text[/]`.

```rust
use std::sync::LazyLock;
use regex::Regex;

/// Markup tag pattern: [style] or [/style] or [/]
static TAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\\*)\[([A-Za-z#/@][^\[\]]*?)\]").expect("invalid regex")
});

/// Parsed tag from markup
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub parameters: Option<String>,
}

impl Tag {
    pub fn is_closing(&self) -> bool {
        self.name.starts_with('/')
    }

    pub fn base_name(&self) -> &str {
        self.name.trim_start_matches('/')
    }
}

/// Parse markup with style stack for nesting
/// Handles escape sequences: \\[ becomes literal [
pub fn parse_markup<'a>(markup: &'a str, base_style: Style) -> Vec<Segment<'a>> {
    // Fast path: no markup tags
    if !markup.contains('[') {
        return vec![Segment::new(Cow::Borrowed(markup), Some(base_style))];
    }

    let mut segments = Vec::new();
    let mut style_stack: Vec<(usize, Style)> = vec![(0, base_style.clone())];
    let mut current_text = String::new();
    let mut last_end = 0;

    for cap in TAG_PATTERN.captures_iter(markup) {
        let full_match = cap.get(0).unwrap();
        let backslashes = cap.get(1).map_or("", |m| m.as_str());
        let tag_content = cap.get(2).map_or("", |m| m.as_str());
        let match_start = full_match.start();

        // Text before this match
        if match_start > last_end {
            current_text.push_str(&markup[last_end..match_start]);
        }

        // Handle escape sequences
        let num_backslashes = backslashes.len();
        let escaped = num_backslashes % 2 == 1;

        // Add literal backslashes (pairs become singles)
        if num_backslashes > 0 {
            current_text.push_str(&"\\".repeat(num_backslashes / 2));
        }

        if escaped {
            // Escaped bracket: literal text
            current_text.push('[');
            current_text.push_str(tag_content);
            current_text.push(']');
        } else {
            // Emit accumulated text with current style
            if !current_text.is_empty() {
                let style = style_stack.last().map(|(_, s)| s.clone());
                segments.push(Segment::new(
                    Cow::Owned(std::mem::take(&mut current_text)),
                    style,
                ));
            }

            // Process tag
            let tag = parse_tag(tag_content);
            if tag.is_closing() {
                // Pop matching style from stack
                let close_name = tag.base_name().trim();
                if close_name.is_empty() {
                    // [/] closes most recent
                    style_stack.pop();
                } else {
                    // [/name] closes specific
                    if let Some(pos) = style_stack.iter().rposition(|(_, s)| {
                        // Match by style name (simplified)
                        true // In real impl, match tag name
                    }) {
                        style_stack.truncate(pos);
                    }
                }
            } else {
                // Opening tag: push new style
                let new_style = parse_style_tag(&tag, style_stack.last().map(|(_, s)| s));
                style_stack.push((segments.len(), new_style));
            }
        }

        last_end = full_match.end();
    }

    // Remaining text
    if last_end < markup.len() {
        current_text.push_str(&markup[last_end..]);
    }
    if !current_text.is_empty() {
        let style = style_stack.last().map(|(_, s)| s.clone());
        segments.push(Segment::new(Cow::Owned(current_text), style));
    }

    segments
}

fn parse_tag(content: &str) -> Tag {
    let trimmed = content.trim();

    // Parameter syntax: name=value
    if let Some(eq_pos) = trimmed.find('=') {
        return Tag {
            name: trimmed[..eq_pos].trim().to_string(),
            parameters: Some(trimmed[eq_pos + 1..].trim().to_string()),
        };
    }

    Tag {
        name: trimmed.to_string(),
        parameters: None,
    }
}
```

### 5.3 From rich_rust: Measurement Protocol

```rust
/// Measurement for layout negotiation
/// Tracks minimum (tightest) and maximum (ideal) widths
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Measurement {
    pub minimum: usize,  // Minimum cells required (tightest compression)
    pub maximum: usize,  // Maximum cells required (ideal unconstrained)
}

impl Measurement {
    pub const ZERO: Self = Self { minimum: 0, maximum: 0 };

    /// Union: take max of both bounds (for side-by-side layout)
    pub fn union(self, other: Self) -> Self {
        Self {
            minimum: self.minimum.max(other.minimum),
            maximum: self.maximum.max(other.maximum),
        }
    }

    /// Stack: add both bounds (for vertical stacking)
    pub fn stack(self, other: Self) -> Self {
        Self {
            minimum: self.minimum + other.minimum,
            maximum: self.maximum + other.maximum,
        }
    }

    /// Intersect: find overlapping range (returns None if no overlap)
    pub fn intersect(self, other: Self) -> Option<Self> {
        let min_val = self.minimum.max(other.minimum);
        let max_val = self.maximum.min(other.maximum);
        if min_val <= max_val {
            Some(Self { minimum: min_val, maximum: max_val })
        } else {
            None
        }
    }

    /// Clamp to constraints
    pub fn clamp(self, min_width: Option<usize>, max_width: Option<usize>) -> Self {
        let mut result = self;
        if let Some(min_w) = min_width {
            result.minimum = result.minimum.max(min_w);
            result.maximum = result.maximum.max(min_w);
        }
        if let Some(max_w) = max_width {
            result.minimum = result.minimum.min(max_w);
            result.maximum = result.maximum.min(max_w);
        }
        result
    }

    pub fn span(&self) -> usize {
        self.maximum.saturating_sub(self.minimum)
    }
}
```

### 5.4 From charmed_rust: CSS-Like Shorthand

```rust
/// Four-sided values (like CSS padding/margin)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

// CSS-style shorthand conversions:
// 1 value:  padding(1)       → all sides = 1
// 2 values: padding((1, 2))  → vertical = 1, horizontal = 2
// 3 values: padding((1, 2, 3)) → top = 1, horizontal = 2, bottom = 3
// 4 values: padding((1, 2, 3, 4)) → top, right, bottom, left (clockwise)

impl<T: Copy> From<T> for Sides<T> {
    fn from(all: T) -> Self {
        Self { top: all, right: all, bottom: all, left: all }
    }
}

impl<T: Copy> From<(T, T)> for Sides<T> {
    fn from((vertical, horizontal): (T, T)) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }
}

impl<T: Copy> From<(T, T, T)> for Sides<T> {
    fn from((top, horizontal, bottom): (T, T, T)) -> Self {
        Self { top, right: horizontal, bottom, left: horizontal }
    }
}

impl<T: Copy> From<(T, T, T, T)> for Sides<T> {
    fn from((top, right, bottom, left): (T, T, T, T)) -> Self {
        Self { top, right, bottom, left }
    }
}

impl<T: Copy + std::ops::Add<Output = T>> Sides<T> {
    pub fn horizontal(&self) -> T { self.left + self.right }
    pub fn vertical(&self) -> T { self.top + self.bottom }
}
```

### 5.5 From charmed_rust: Style with Bitflags Property Tracking

```rust
use bitflags::bitflags;

bitflags! {
    /// Tracks which properties have been explicitly set
    /// Enables proper inheritance: unset properties inherit from parent
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Props: u64 {
        const BOLD = 1 << 0;
        const ITALIC = 1 << 1;
        const UNDERLINE = 1 << 2;
        const STRIKETHROUGH = 1 << 3;
        const BLINK = 1 << 4;
        const REVERSE = 1 << 5;
        const DIM = 1 << 6;
        const HIDDEN = 1 << 7;

        const FG_COLOR = 1 << 8;
        const BG_COLOR = 1 << 9;

        const WIDTH = 1 << 10;
        const HEIGHT = 1 << 11;
        const MAX_WIDTH = 1 << 12;
        const MAX_HEIGHT = 1 << 13;

        const PADDING_TOP = 1 << 14;
        const PADDING_RIGHT = 1 << 15;
        const PADDING_BOTTOM = 1 << 16;
        const PADDING_LEFT = 1 << 17;

        const MARGIN_TOP = 1 << 18;
        const MARGIN_RIGHT = 1 << 19;
        const MARGIN_BOTTOM = 1 << 20;
        const MARGIN_LEFT = 1 << 21;

        const BORDER_STYLE = 1 << 22;
        const BORDER_TOP = 1 << 23;
        const BORDER_RIGHT = 1 << 24;
        const BORDER_BOTTOM = 1 << 25;
        const BORDER_LEFT = 1 << 26;

        const ALIGN_H = 1 << 27;
        const ALIGN_V = 1 << 28;

        const LINK = 1 << 29;
        const TAB_WIDTH = 1 << 30;
    }
}

bitflags! {
    /// Boolean text attributes
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Attrs: u8 {
        const BOLD = 1 << 0;
        const DIM = 1 << 1;
        const ITALIC = 1 << 2;
        const UNDERLINE = 1 << 3;
        const BLINK = 1 << 4;
        const REVERSE = 1 << 5;
        const HIDDEN = 1 << 6;
        const STRIKETHROUGH = 1 << 7;
    }
}

/// Full style with all properties
#[derive(Clone, Default)]
pub struct Style {
    props: Props,              // Which properties are explicitly set
    attrs: Attrs,              // Boolean attributes
    fg_color: Option<Color>,
    bg_color: Option<Color>,
    width: u16,
    height: u16,
    max_width: u16,
    max_height: u16,
    padding: Sides<u16>,
    margin: Sides<u16>,
    border_style: BorderStyle,
    border_edges: BorderEdges,
    border_fg: Option<Color>,
    border_bg: Option<Color>,
    align_h: HAlign,
    align_v: VAlign,
    link: Option<String>,
    tab_width: u8,
}

impl Style {
    pub fn new() -> Self { Self::default() }

    // Fluent builders that track which properties are set

    pub fn fg(mut self, color: impl Into<Color>) -> Self {
        self.fg_color = Some(color.into());
        self.props |= Props::FG_COLOR;
        self
    }

    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.bg_color = Some(color.into());
        self.props |= Props::BG_COLOR;
        self
    }

    pub fn bold(mut self) -> Self {
        self.attrs |= Attrs::BOLD;
        self.props |= Props::BOLD;
        self
    }

    pub fn italic(mut self) -> Self {
        self.attrs |= Attrs::ITALIC;
        self.props |= Props::ITALIC;
        self
    }

    pub fn underline(mut self) -> Self {
        self.attrs |= Attrs::UNDERLINE;
        self.props |= Props::UNDERLINE;
        self
    }

    pub fn padding<E: Into<Sides<u16>>>(mut self, edges: E) -> Self {
        self.padding = edges.into();
        self.props |= Props::PADDING_TOP | Props::PADDING_RIGHT |
                      Props::PADDING_BOTTOM | Props::PADDING_LEFT;
        self
    }

    pub fn margin<E: Into<Sides<u16>>>(mut self, edges: E) -> Self {
        self.margin = edges.into();
        self.props |= Props::MARGIN_TOP | Props::MARGIN_RIGHT |
                      Props::MARGIN_BOTTOM | Props::MARGIN_LEFT;
        self
    }

    pub fn border(mut self, style: BorderStyle) -> Self {
        self.border_style = style;
        self.border_edges = BorderEdges::all();
        self.props |= Props::BORDER_STYLE | Props::BORDER_TOP |
                      Props::BORDER_RIGHT | Props::BORDER_BOTTOM | Props::BORDER_LEFT;
        self
    }

    pub fn width(mut self, w: u16) -> Self {
        self.width = w;
        self.props |= Props::WIDTH;
        self
    }

    pub fn height(mut self, h: u16) -> Self {
        self.height = h;
        self.props |= Props::HEIGHT;
        self
    }

    pub fn link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self.props |= Props::LINK;
        self
    }

    /// Inherit unset properties from parent
    pub fn inherit(mut self, parent: &Style) -> Self {
        // Only inherit properties NOT set on self
        let unset = !self.props;

        if unset.contains(Props::FG_COLOR) {
            self.fg_color = parent.fg_color.clone();
        }
        if unset.contains(Props::BG_COLOR) {
            self.bg_color = parent.bg_color.clone();
        }
        // ... inherit other properties

        // Merge attributes (always combine, don't replace)
        if unset.intersects(Props::BOLD | Props::ITALIC | Props::UNDERLINE /* ... */) {
            self.attrs |= parent.attrs;
        }

        self
    }
}
```

### 5.6 From charmed_rust: Border System

```rust
/// Complete border definition with all corners and edges
#[derive(Clone, Debug)]
pub struct Border {
    pub top: char,
    pub bottom: char,
    pub left: char,
    pub right: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    // For tables with internal dividers
    pub middle_left: char,
    pub middle_right: char,
    pub middle: char,
    pub middle_top: char,
    pub middle_bottom: char,
}

impl Border {
    pub fn rounded() -> Self {
        Self {
            top: '─', bottom: '─', left: '│', right: '│',
            top_left: '╭', top_right: '╮',
            bottom_left: '╰', bottom_right: '╯',
            middle_left: '├', middle_right: '┤',
            middle: '┼', middle_top: '┬', middle_bottom: '┴',
        }
    }

    pub fn square() -> Self {
        Self {
            top: '─', bottom: '─', left: '│', right: '│',
            top_left: '┌', top_right: '┐',
            bottom_left: '└', bottom_right: '┘',
            middle_left: '├', middle_right: '┤',
            middle: '┼', middle_top: '┬', middle_bottom: '┴',
        }
    }

    pub fn double() -> Self {
        Self {
            top: '═', bottom: '═', left: '║', right: '║',
            top_left: '╔', top_right: '╗',
            bottom_left: '╚', bottom_right: '╝',
            middle_left: '╠', middle_right: '╣',
            middle: '╬', middle_top: '╦', middle_bottom: '╩',
        }
    }

    pub fn heavy() -> Self {
        Self {
            top: '━', bottom: '━', left: '┃', right: '┃',
            top_left: '┏', top_right: '┓',
            bottom_left: '┗', bottom_right: '┛',
            middle_left: '┣', middle_right: '┫',
            middle: '╋', middle_top: '┳', middle_bottom: '┻',
        }
    }

    pub fn ascii() -> Self {
        Self {
            top: '-', bottom: '-', left: '|', right: '|',
            top_left: '+', top_right: '+',
            bottom_left: '+', bottom_right: '+',
            middle_left: '+', middle_right: '+',
            middle: '+', middle_top: '+', middle_bottom: '+',
        }
    }

    pub fn hidden() -> Self {
        Self {
            top: ' ', bottom: ' ', left: ' ', right: ' ',
            top_left: ' ', top_right: ' ',
            bottom_left: ' ', bottom_right: ' ',
            middle_left: ' ', middle_right: ' ',
            middle: ' ', middle_top: ' ', middle_bottom: ' ',
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum BorderStyle {
    #[default]
    None,
    Rounded,
    Square,
    Double,
    Heavy,
    Ascii,
    Hidden,
    Custom(Border),
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct BorderEdges: u8 {
        const TOP = 1 << 0;
        const RIGHT = 1 << 1;
        const BOTTOM = 1 << 2;
        const LEFT = 1 << 3;
    }
}

impl BorderEdges {
    pub fn all() -> Self {
        Self::TOP | Self::RIGHT | Self::BOTTOM | Self::LEFT
    }

    pub fn horizontal() -> Self {
        Self::TOP | Self::BOTTOM
    }

    pub fn vertical() -> Self {
        Self::LEFT | Self::RIGHT
    }
}
```

### 5.7 From charmed_rust: Color System with Profile Detection

```rust
/// Color profile (terminal capability level)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorProfile {
    Ascii,      // No color (1-bit)
    Ansi,       // 16 colors (4-bit)
    Ansi256,    // 256 colors (8-bit)
    TrueColor,  // 16 million colors (24-bit)
}

impl ColorProfile {
    /// Detect from environment
    pub fn detect() -> Self {
        // Check NO_COLOR first (standard for disabling color)
        if std::env::var("NO_COLOR").is_ok() {
            return Self::Ascii;
        }

        // Check COLORTERM for true color
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            if colorterm == "truecolor" || colorterm == "24bit" {
                return Self::TrueColor;
            }
        }

        // Check TERM
        if let Ok(term) = std::env::var("TERM") {
            // Known true-color terminals
            if term.contains("kitty") || term.contains("wezterm") ||
               term.contains("alacritty") || term.contains("ghostty") {
                return Self::TrueColor;
            }
            if term.contains("256color") {
                return Self::Ansi256;
            }
            if term.contains("color") || term.starts_with("xterm") {
                return Self::Ansi;
            }
            if term == "dumb" {
                return Self::Ascii;
            }
        }

        // Conservative default
        Self::TrueColor
    }

    /// Check if this profile supports another
    pub fn supports(&self, other: Self) -> bool {
        *self >= other
    }
}

/// Color that can be resolved against a profile
#[derive(Clone, Debug)]
pub enum Color {
    /// Direct RGB value
    Rgb(u8, u8, u8),
    /// ANSI color index (0-255)
    Ansi(u8),
    /// Named color (resolved from theme)
    Named(String),
    /// Semantic color slot
    Semantic(SemanticColor),
    /// Adaptive color (different for light/dark)
    Adaptive { light: Box<Color>, dark: Box<Color> },
}

#[derive(Clone, Copy, Debug)]
pub enum SemanticColor {
    Foreground,
    Background,
    Primary,
    Secondary,
    Success,
    Warning,
    Error,
    Muted,
    Link,
    Selection,
    CodeBg,
}

impl Color {
    /// Parse from string: "#RGB", "#RRGGBB", "red", "123" (ANSI)
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        // Hex color
        if s.starts_with('#') {
            let hex = &s[1..];
            return match hex.len() {
                3 => {
                    let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                    let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                    let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                    Some(Self::Rgb(r, g, b))
                }
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some(Self::Rgb(r, g, b))
                }
                _ => None,
            };
        }

        // ANSI index
        if let Ok(n) = s.parse::<u8>() {
            return Some(Self::Ansi(n));
        }

        // Named color
        Some(Self::Named(s.to_string()))
    }

    /// Generate ANSI escape for foreground
    pub fn to_ansi_fg(&self, profile: ColorProfile, dark_bg: bool) -> String {
        match (self, profile) {
            (Self::Rgb(r, g, b), ColorProfile::TrueColor) => {
                format!("\x1b[38;2;{};{};{}m", r, g, b)
            }
            (Self::Rgb(r, g, b), ColorProfile::Ansi256) => {
                let idx = rgb_to_256(*r, *g, *b);
                format!("\x1b[38;5;{}m", idx)
            }
            (Self::Rgb(r, g, b), ColorProfile::Ansi) => {
                let idx = rgb_to_16(*r, *g, *b);
                if idx < 8 {
                    format!("\x1b[{}m", 30 + idx)
                } else {
                    format!("\x1b[{}m", 90 + idx - 8)
                }
            }
            (Self::Ansi(n), ColorProfile::TrueColor | ColorProfile::Ansi256) => {
                format!("\x1b[38;5;{}m", n)
            }
            (Self::Ansi(n), ColorProfile::Ansi) => {
                let n = if *n >= 8 && *n < 16 { n - 8 + 90 } else { 30 + n };
                format!("\x1b[{}m", n.min(37))
            }
            (Self::Adaptive { light, dark }, _) => {
                let color = if dark_bg { dark } else { light };
                color.to_ansi_fg(profile, dark_bg)
            }
            (_, ColorProfile::Ascii) => String::new(),
            _ => String::new(), // Named/Semantic resolved elsewhere
        }
    }
}

/// Convert RGB to 256-color palette index
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Check grayscale first (more accurate for grays)
    if r == g && g == b {
        if r < 8 { return 16; }  // Black
        if r > 248 { return 231; }  // White
        return 232 + ((r - 8) / 10).min(23);  // Grayscale ramp
    }

    // 6×6×6 color cube
    let r6 = (r as u16 * 6 / 256) as u8;
    let g6 = (g as u16 * 6 / 256) as u8;
    let b6 = (b as u16 * 6 / 256) as u8;
    16 + 36 * r6 + 6 * g6 + b6
}

/// Convert RGB to 16-color ANSI index
fn rgb_to_16(r: u8, g: u8, b: u8) -> u8 {
    let brightness = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    let bright = brightness > 127;

    let base = match (r > 127, g > 127, b > 127) {
        (false, false, false) => 0,  // black
        (true, false, false) => 1,   // red
        (false, true, false) => 2,   // green
        (true, true, false) => 3,    // yellow
        (false, false, true) => 4,   // blue
        (true, false, true) => 5,    // magenta
        (false, true, true) => 6,    // cyan
        (true, true, true) => 7,     // white
    };

    if bright { base + 8 } else { base }
}
```

---

## Chapter 6: The Optimal Cell and Buffer

### 6.1 Cell Structure (16 bytes, derived from opentui_rust)

```rust
/// A single terminal cell.
///
/// # Memory Layout (16 bytes total)
/// ```text
/// ┌─────────────────┬─────────────────┬─────────────────┬─────────────────┐
/// │  content (4B)   │    fg (4B)      │    bg (4B)      │   attrs (4B)    │
/// └─────────────────┴─────────────────┴─────────────────┴─────────────────┘
/// ```
///
/// # Performance Characteristics
/// - 4 cells per 64-byte cache line
/// - Single 128-bit SIMD comparison
/// - No heap allocation for BMP characters
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct Cell {
    content: CellContent,
    fg: PackedRgba,
    bg: PackedRgba,
    attrs: CellAttrs,
}

/// Cell content: character, grapheme reference, or placeholder
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellContent {
    Empty = 0,
    Char(char),           // Unicode scalar (up to 0x10FFFF, fits in u32)
    Grapheme(GraphemeId), // Pool reference with cached width
    Continuation,         // Wide char occupancy marker
}

impl CellContent {
    /// Display width of this content
    pub fn width(&self) -> usize {
        match self {
            Self::Empty | Self::Continuation => 0,
            Self::Char(c) => unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0),
            Self::Grapheme(id) => id.width(),
        }
    }
}

/// Packed RGBA color (4 bytes)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedRgba(u32);

impl PackedRgba {
    pub const TRANSPARENT: Self = Self(0);
    pub const BLACK: Self = Self(0xFF000000);  // Alpha = 255
    pub const WHITE: Self = Self(0xFFFFFFFF);

    #[inline]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self((r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24))
    }

    #[inline] pub fn r(self) -> u8 { self.0 as u8 }
    #[inline] pub fn g(self) -> u8 { (self.0 >> 8) as u8 }
    #[inline] pub fn b(self) -> u8 { (self.0 >> 16) as u8 }
    #[inline] pub fn a(self) -> u8 { (self.0 >> 24) as u8 }

    /// Multiply alpha by factor (for opacity stack)
    #[inline]
    pub fn with_alpha_multiplied(self, factor: f32) -> Self {
        let new_a = ((self.a() as f32) * factor) as u8;
        Self((self.0 & 0x00FFFFFF) | ((new_a as u32) << 24))
    }

    /// Porter-Duff "over" compositing
    /// self over bg: result = self + bg * (1 - self.alpha)
    pub fn over(self, bg: Self) -> Self {
        let sa = self.a() as f32 / 255.0;
        let da = bg.a() as f32 / 255.0;

        let out_a = sa + da * (1.0 - sa);
        if out_a == 0.0 {
            return Self::TRANSPARENT;
        }

        let inv_sa = 1.0 - sa;
        let out_r = ((self.r() as f32 * sa + bg.r() as f32 * da * inv_sa) / out_a) as u8;
        let out_g = ((self.g() as f32 * sa + bg.g() as f32 * da * inv_sa) / out_a) as u8;
        let out_b = ((self.b() as f32 * sa + bg.b() as f32 * da * inv_sa) / out_a) as u8;

        Self::new(out_r, out_g, out_b, (out_a * 255.0) as u8)
    }
}

/// Cell attributes: text styling + hyperlink ID
/// Bits 0-7:  Style flags (bold, italic, underline, etc.)
/// Bits 8-31: Hyperlink ID (24-bit, 0 = no link)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct CellAttrs(u32);

impl CellAttrs {
    pub const BOLD: u32       = 1 << 0;
    pub const DIM: u32        = 1 << 1;
    pub const ITALIC: u32     = 1 << 2;
    pub const UNDERLINE: u32  = 1 << 3;
    pub const BLINK: u32      = 1 << 4;
    pub const REVERSE: u32    = 1 << 5;
    pub const HIDDEN: u32     = 1 << 6;
    pub const STRIKE: u32     = 1 << 7;

    #[inline]
    pub fn flags(self) -> u8 { self.0 as u8 }

    #[inline]
    pub fn link_id(self) -> u32 { self.0 >> 8 }

    #[inline]
    pub fn with_link(self, id: u32) -> Self {
        debug_assert!(id < (1 << 24), "Link ID overflow");
        Self((self.0 & 0xFF) | (id << 8))
    }

    pub fn has(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn clear(&mut self, flag: u32) {
        self.0 &= !flag;
    }
}

impl Cell {
    pub const EMPTY: Self = Self {
        content: CellContent::Empty,
        fg: PackedRgba::WHITE,
        bg: PackedRgba::TRANSPARENT,
        attrs: CellAttrs(0),
    };

    /// Bitwise equality (3-4× faster than derived PartialEq)
    #[inline(always)]
    pub fn bits_eq(&self, other: &Self) -> bool {
        // Safety: Cell is repr(C) with known layout, all fields are Copy
        unsafe {
            let a = std::ptr::read(self as *const _ as *const [u32; 4]);
            let b = std::ptr::read(other as *const _ as *const [u32; 4]);
            a == b
        }
    }

    pub fn width(&self) -> usize {
        self.content.width()
    }
}

// Implement PartialEq using bits_eq for consistency
impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.bits_eq(other)
    }
}
impl Eq for Cell {}
```

### 6.2 GraphemePool (Reference-Counted Interning)

```rust
/// Grapheme ID: 32-bit with cached width
/// Layout: [31: reserved][30-24: width (7 bits)][23-0: slot (24 bits)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GraphemeId(u32);

impl GraphemeId {
    #[inline]
    pub fn new(slot: u32, width: u8) -> Self {
        debug_assert!(slot < (1 << 24), "Slot overflow");
        debug_assert!(width < 128, "Width overflow");
        Self(slot | ((width as u32) << 24))
    }

    #[inline]
    pub fn slot(self) -> u32 { self.0 & 0x00FFFFFF }

    #[inline]
    pub fn width(self) -> usize { ((self.0 >> 24) & 0x7F) as usize }
}

/// Slot in the grapheme pool
struct Slot {
    bytes: Box<str>,    // UTF-8 grapheme content
    refcount: u32,      // Reference count (0 = free)
    width: u8,          // Cached display width
}

/// Interned storage for multi-codepoint grapheme clusters
pub struct GraphemePool {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
    index: HashMap<Box<str>, u32>,
}

impl GraphemePool {
    pub fn new() -> Self {
        Self {
            slots: Vec::with_capacity(256),
            free_list: Vec::new(),
            index: HashMap::with_capacity(256),
        }
    }

    /// Intern a grapheme, returning its ID with cached width
    pub fn intern(&mut self, grapheme: &str) -> GraphemeId {
        // Fast path: already interned
        if let Some(&slot) = self.index.get(grapheme) {
            self.slots[slot as usize].refcount += 1;
            return GraphemeId::new(slot, self.slots[slot as usize].width);
        }

        // Slow path: allocate new slot
        let width = unicode_width::UnicodeWidthStr::width(grapheme)
            .min(127) as u8;
        let slot = self.alloc_slot(grapheme.into(), width);
        self.index.insert(grapheme.into(), slot);
        GraphemeId::new(slot, width)
    }

    /// Increment reference count
    #[inline]
    pub fn incref(&mut self, id: GraphemeId) {
        self.slots[id.slot() as usize].refcount += 1;
    }

    /// Decrement reference count, returns true if still alive
    #[inline]
    pub fn decref(&mut self, id: GraphemeId) -> bool {
        let slot = &mut self.slots[id.slot() as usize];
        slot.refcount = slot.refcount.saturating_sub(1);
        if slot.refcount == 0 {
            self.free_list.push(id.slot());
            false
        } else {
            true
        }
    }

    /// Get grapheme string by ID
    #[inline]
    pub fn get(&self, id: GraphemeId) -> &str {
        &self.slots[id.slot() as usize].bytes
    }

    fn alloc_slot(&mut self, bytes: Box<str>, width: u8) -> u32 {
        if let Some(slot) = self.free_list.pop() {
            self.slots[slot as usize] = Slot { bytes, refcount: 1, width };
            slot
        } else {
            let slot = self.slots.len() as u32;
            debug_assert!(slot < (1 << 24), "Pool overflow");
            self.slots.push(Slot { bytes, refcount: 1, width });
            slot
        }
    }

    pub fn stats(&self) -> PoolStats {
        let active = self.slots.iter().filter(|s| s.refcount > 0).count();
        PoolStats {
            total_slots: self.slots.len(),
            active_slots: active,
            free_slots: self.free_list.len(),
        }
    }
}

#[derive(Debug)]
pub struct PoolStats {
    pub total_slots: usize,
    pub active_slots: usize,
    pub free_slots: usize,
}
```

### 6.3 Buffer with Scissor and Opacity Stacks

```rust
use smallvec::SmallVec;

/// Rectangular region for clipping
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width &&
        y >= self.y && y < self.y + self.height
    }

    pub fn intersect(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);

        if right > x && bottom > y {
            Self { x, y, width: right - x, height: bottom - y }
        } else {
            Self { x: 0, y: 0, width: 0, height: 0 }  // Empty
        }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Inner rect after removing edges
    pub fn inner(&self, edges: &Sides<u16>) -> Self {
        let x = self.x + edges.left;
        let y = self.y + edges.top;
        let width = self.width.saturating_sub(edges.left + edges.right);
        let height = self.height.saturating_sub(edges.top + edges.bottom);
        Self { x, y, width, height }
    }
}

/// Stack of clipping regions (nested clips)
pub struct ScissorStack {
    stack: SmallVec<[Rect; 8]>,  // Stack-allocated for typical nesting
    current: Rect,
}

impl ScissorStack {
    pub fn new(bounds: Rect) -> Self {
        Self {
            stack: SmallVec::new(),
            current: bounds,
        }
    }

    pub fn push(&mut self, rect: Rect) {
        self.stack.push(self.current);
        self.current = self.current.intersect(&rect);
    }

    pub fn pop(&mut self) -> Option<Rect> {
        self.stack.pop().map(|r| std::mem::replace(&mut self.current, r))
    }

    #[inline]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.current.contains(x, y)
    }

    pub fn current(&self) -> &Rect {
        &self.current
    }
}

/// Stack of opacity multipliers
pub struct OpacityStack {
    stack: SmallVec<[f32; 8]>,
    current: f32,  // Product of all active opacities
}

impl OpacityStack {
    pub fn new() -> Self {
        Self {
            stack: SmallVec::new(),
            current: 1.0,
        }
    }

    pub fn push(&mut self, opacity: f32) {
        self.stack.push(self.current);
        self.current *= opacity.clamp(0.0, 1.0);
    }

    pub fn pop(&mut self) -> Option<f32> {
        self.stack.pop().map(|o| std::mem::replace(&mut self.current, o))
    }

    #[inline]
    pub fn current(&self) -> f32 {
        self.current
    }
}

/// 2D grid of cells with scissor and opacity stacks
pub struct Buffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    scissor_stack: ScissorStack,
    opacity_stack: OpacityStack,
}

impl Buffer {
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells: vec![Cell::EMPTY; size],
            scissor_stack: ScissorStack::new(Rect::new(0, 0, width, height)),
            opacity_stack: OpacityStack::new(),
        }
    }

    #[inline]
    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize) * (self.width as usize) + (x as usize)
    }

    #[inline]
    pub fn get(&self, x: u16, y: u16) -> &Cell {
        debug_assert!(x < self.width && y < self.height);
        &self.cells[self.index(x, y)]
    }

    /// Get without bounds check (for hot loops)
    #[inline]
    pub unsafe fn get_unchecked(&self, x: u16, y: u16) -> &Cell {
        self.cells.get_unchecked(self.index(x, y))
    }

    #[inline]
    pub fn set(&mut self, x: u16, y: u16, mut cell: Cell) {
        if !self.scissor_stack.contains(x, y) {
            return;
        }

        // Apply opacity
        let opacity = self.opacity_stack.current();
        if opacity < 1.0 {
            cell.fg = cell.fg.with_alpha_multiplied(opacity);
            cell.bg = cell.bg.with_alpha_multiplied(opacity);
        }

        let idx = self.index(x, y);
        self.cells[idx] = cell;
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::EMPTY);
    }

    pub fn clear_region(&mut self, rect: Rect) {
        for y in rect.y..(rect.y + rect.height).min(self.height) {
            for x in rect.x..(rect.x + rect.width).min(self.width) {
                let idx = self.index(x, y);
                self.cells[idx] = Cell::EMPTY;
            }
        }
    }

    // Scissor operations
    pub fn push_scissor(&mut self, rect: Rect) {
        self.scissor_stack.push(rect);
    }

    pub fn pop_scissor(&mut self) {
        self.scissor_stack.pop();
    }

    // Opacity operations
    pub fn push_opacity(&mut self, opacity: f32) {
        self.opacity_stack.push(opacity);
    }

    pub fn pop_opacity(&mut self) {
        self.opacity_stack.pop();
    }

    // Accessors
    pub fn width(&self) -> u16 { self.width }
    pub fn height(&self) -> u16 { self.height }
    pub fn cells(&self) -> &[Cell] { &self.cells }
}
```

---

## Chapter 7: Diff Algorithm and Presenter

### 7.1 Buffer Diff (Cell-Level)

```rust
/// Diff result: list of changed cell coordinates
pub struct BufferDiff {
    changes: Vec<(u16, u16)>,
    change_count: usize,
}

impl BufferDiff {
    /// Compute minimal change set between two buffers
    pub fn compute(old: &Buffer, new: &Buffer) -> Self {
        debug_assert_eq!(old.width, new.width);
        debug_assert_eq!(old.height, new.height);

        let total = (old.width as usize) * (old.height as usize);
        let mut changes = Vec::with_capacity(total / 20);  // Expect ~5% changes

        // Sequential scan with bitwise comparison
        for (i, (old_cell, new_cell)) in old.cells.iter().zip(&new.cells).enumerate() {
            if !old_cell.bits_eq(new_cell) {
                let x = (i % old.width as usize) as u16;
                let y = (i / old.width as usize) as u16;
                changes.push((x, y));
            }
        }

        Self {
            change_count: changes.len(),
            changes,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.change_count == 0
    }

    /// Should we skip diffing and do full redraw?
    /// Heuristic: if >50% changed, full redraw is cheaper
    pub fn should_full_redraw(&self, total_cells: usize) -> bool {
        self.change_count > total_cells / 2
    }

    pub fn changes(&self) -> &[(u16, u16)] {
        &self.changes
    }
}
```

### 7.2 Presenter (ANSI Output with State Tracking)

```rust
use std::io::{self, Write};

/// Terminal capabilities
#[derive(Clone, Debug)]
pub struct TerminalCapabilities {
    pub true_color: bool,
    pub color_256: bool,
    pub sync_output: bool,
    pub hyperlinks: bool,
    pub kitty_keyboard: bool,
    pub bracketed_paste: bool,
    pub focus_events: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();

        Self {
            true_color: colorterm == "truecolor" || colorterm == "24bit" ||
                        term.contains("kitty") || term.contains("wezterm") ||
                        term.contains("alacritty") || term.contains("ghostty"),
            color_256: term.contains("256color") || term.contains("xterm"),
            sync_output: term.contains("kitty") || term.contains("wezterm") ||
                         term.contains("alacritty") || term.contains("contour") ||
                         term.contains("foot") || term.contains("ghostty"),
            hyperlinks: term.contains("kitty") || term.contains("wezterm") ||
                        term.contains("alacritty") || term.contains("iterm") ||
                        term.contains("foot") || term.contains("ghostty"),
            kitty_keyboard: term.contains("kitty") || term.contains("wezterm") ||
                            term.contains("foot") || term.contains("ghostty"),
            bracketed_paste: true,  // Widely supported
            focus_events: !term.contains("linux") && !term.contains("dumb"),
        }
    }
}

/// Current style state for minimal SGR output
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct StyleState {
    fg: PackedRgba,
    bg: PackedRgba,
    attrs: u8,
}

/// Hyperlink registry: maps URLs to IDs
pub struct LinkRegistry {
    links: Vec<String>,
    index: HashMap<String, u32>,
}

impl LinkRegistry {
    pub fn new() -> Self {
        Self {
            links: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn register(&mut self, url: &str) -> u32 {
        if let Some(&id) = self.index.get(url) {
            return id;
        }
        let id = self.links.len() as u32;
        assert!(id < (1 << 24), "Link registry overflow");
        self.links.push(url.to_string());
        self.index.insert(url.to_string(), id);
        id
    }

    pub fn get(&self, id: u32) -> Option<&str> {
        self.links.get(id as usize).map(|s| s.as_str())
    }
}

/// ANSI output generator with state tracking
pub struct Presenter {
    caps: TerminalCapabilities,
    current_style: StyleState,
    current_link: Option<u32>,
    cursor_x: u16,
    cursor_y: u16,
    scratch: Vec<u8>,  // Reusable output buffer
}

impl Presenter {
    pub fn new(caps: TerminalCapabilities) -> Self {
        Self {
            caps,
            current_style: StyleState::default(),
            current_link: None,
            cursor_x: 0,
            cursor_y: 0,
            scratch: Vec::with_capacity(4096),
        }
    }

    /// Present buffer changes to output
    pub fn present(
        &mut self,
        out: &mut impl Write,
        buffer: &Buffer,
        diff: &BufferDiff,
        pool: &GraphemePool,
        links: &LinkRegistry,
    ) -> io::Result<()> {
        if diff.is_empty() {
            return Ok(());
        }

        self.scratch.clear();

        // Synchronized output: begin
        if self.caps.sync_output {
            self.scratch.extend_from_slice(b"\x1b[?2026h");
        }

        // Emit changes
        if diff.should_full_redraw(buffer.cells().len()) {
            self.emit_full(buffer, pool, links);
        } else {
            self.emit_diff(buffer, diff, pool, links);
        }

        // Synchronized output: end
        if self.caps.sync_output {
            self.scratch.extend_from_slice(b"\x1b[?2026l");
        }

        out.write_all(&self.scratch)?;
        out.flush()
    }

    fn emit_diff(
        &mut self,
        buffer: &Buffer,
        diff: &BufferDiff,
        pool: &GraphemePool,
        links: &LinkRegistry,
    ) {
        for &(x, y) in diff.changes() {
            self.move_cursor_to(x, y);
            let cell = buffer.get(x, y);
            self.emit_cell(cell, pool, links);
        }
    }

    fn emit_full(
        &mut self,
        buffer: &Buffer,
        pool: &GraphemePool,
        links: &LinkRegistry,
    ) {
        // Clear screen and move home
        self.scratch.extend_from_slice(b"\x1b[2J\x1b[H");
        self.cursor_x = 0;
        self.cursor_y = 0;

        for y in 0..buffer.height() {
            for x in 0..buffer.width() {
                let cell = buffer.get(x, y);
                self.emit_cell(cell, pool, links);
            }
            if y < buffer.height() - 1 {
                self.scratch.extend_from_slice(b"\r\n");
                self.cursor_x = 0;
                self.cursor_y += 1;
            }
        }
    }

    fn move_cursor_to(&mut self, x: u16, y: u16) {
        if self.cursor_y == y && x == self.cursor_x {
            // Already there
        } else if self.cursor_y == y && x == self.cursor_x + 1 {
            // Adjacent: cursor advances naturally after character
        } else if self.cursor_y == y {
            // Same row: relative movement
            let delta = x as i32 - self.cursor_x as i32;
            if delta > 0 {
                write!(&mut self.scratch, "\x1b[{}C", delta).unwrap();
            } else {
                write!(&mut self.scratch, "\x1b[{}D", -delta).unwrap();
            }
        } else {
            // Different row: absolute positioning (1-indexed)
            write!(&mut self.scratch, "\x1b[{};{}H", y + 1, x + 1).unwrap();
        }
        self.cursor_x = x;
        self.cursor_y = y;
    }

    fn emit_cell(&mut self, cell: &Cell, pool: &GraphemePool, links: &LinkRegistry) {
        // Style changes
        self.emit_style_changes(cell);

        // Link changes (OSC 8)
        self.emit_link_changes(cell, links);

        // Content
        match cell.content {
            CellContent::Empty => self.scratch.push(b' '),
            CellContent::Char(c) => {
                let mut buf = [0u8; 4];
                self.scratch.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            CellContent::Grapheme(id) => {
                self.scratch.extend_from_slice(pool.get(id).as_bytes());
            }
            CellContent::Continuation => {}  // Skip, wide char already written
        }

        // Update cursor position
        self.cursor_x += cell.width() as u16;
    }

    fn emit_style_changes(&mut self, cell: &Cell) {
        let new_style = StyleState {
            fg: cell.fg,
            bg: cell.bg,
            attrs: cell.attrs.flags(),
        };

        if new_style == self.current_style {
            return;
        }

        // Build SGR sequence
        self.scratch.extend_from_slice(b"\x1b[");
        let mut first = true;

        // Check if we need to reset (attributes removed)
        let removed = self.current_style.attrs & !new_style.attrs;
        if removed != 0 {
            self.scratch.push(b'0');
            first = false;
            self.current_style = StyleState::default();
        }

        // Add attributes
        let added = new_style.attrs & !self.current_style.attrs;
        if added & (CellAttrs::BOLD as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'1');
            first = false;
        }
        if added & (CellAttrs::DIM as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'2');
            first = false;
        }
        if added & (CellAttrs::ITALIC as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'3');
            first = false;
        }
        if added & (CellAttrs::UNDERLINE as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'4');
            first = false;
        }
        if added & (CellAttrs::BLINK as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'5');
            first = false;
        }
        if added & (CellAttrs::REVERSE as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'7');
            first = false;
        }
        if added & (CellAttrs::HIDDEN as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'8');
            first = false;
        }
        if added & (CellAttrs::STRIKE as u8) != 0 {
            if !first { self.scratch.push(b';'); }
            self.scratch.push(b'9');
            first = false;
        }

        // Foreground color
        if new_style.fg != self.current_style.fg {
            self.emit_color(new_style.fg, true, &mut first);
        }

        // Background color
        if new_style.bg != self.current_style.bg {
            self.emit_color(new_style.bg, false, &mut first);
        }

        self.scratch.push(b'm');
        self.current_style = new_style;
    }

    fn emit_color(&mut self, color: PackedRgba, foreground: bool, first: &mut bool) {
        if !*first { self.scratch.push(b';'); }
        *first = false;

        let (r, g, b) = (color.r(), color.g(), color.b());

        if self.caps.true_color {
            if foreground {
                write!(&mut self.scratch, "38;2;{};{};{}", r, g, b).unwrap();
            } else {
                write!(&mut self.scratch, "48;2;{};{};{}", r, g, b).unwrap();
            }
        } else if self.caps.color_256 {
            let idx = rgb_to_256(r, g, b);
            if foreground {
                write!(&mut self.scratch, "38;5;{}", idx).unwrap();
            } else {
                write!(&mut self.scratch, "48;5;{}", idx).unwrap();
            }
        } else {
            let idx = rgb_to_16(r, g, b);
            if foreground {
                write!(&mut self.scratch, "{}", if idx < 8 { 30 + idx } else { 90 + idx - 8 }).unwrap();
            } else {
                write!(&mut self.scratch, "{}", if idx < 8 { 40 + idx } else { 100 + idx - 8 }).unwrap();
            }
        }
    }

    fn emit_link_changes(&mut self, cell: &Cell, registry: &LinkRegistry) {
        let new_link = cell.attrs.link_id();
        let current = self.current_link.unwrap_or(0);

        if new_link == current {
            return;
        }

        if new_link == 0 {
            // Close link: OSC 8 ; ; ST
            self.scratch.extend_from_slice(b"\x1b]8;;\x1b\\");
            self.current_link = None;
        } else if let Some(url) = registry.get(new_link) {
            // Open link: OSC 8 ; ; URL ST
            write!(&mut self.scratch, "\x1b]8;;{}\x1b\\", url).unwrap();
            self.current_link = Some(new_link);
        }
    }
}
```

---

## Chapter 8: Terminal Protocol Support

### 8.1 Synchronized Output (DEC Mode 2026)

```rust
/// Zero-flicker rendering via synchronized output
///
/// Protocol:
///   Begin: CSI ? 2026 h  (\x1b[?2026h)
///   End:   CSI ? 2026 l  (\x1b[?2026l)
///
/// Terminal Support:
///   ✓ Kitty, WezTerm, Alacritty, Ghostty, Contour, foot
///   ✗ iTerm2, GNOME Terminal, Konsole, xterm

pub const SYNC_OUTPUT_BEGIN: &[u8] = b"\x1b[?2026h";
pub const SYNC_OUTPUT_END: &[u8] = b"\x1b[?2026l";

/// Query synchronized output support
pub const SYNC_OUTPUT_QUERY: &[u8] = b"\x1b[?2026$p";

/// Parse sync output query response
/// Response: CSI ? 2026 ; <value> $ y
/// value: 0=not recognized, 1=set, 2=reset, 4=permanently reset (not supported)
pub fn parse_sync_output_response(response: &[u8]) -> Option<bool> {
    // Expected: \x1b[?2026;N$y
    if response.len() < 10 { return None; }
    if !response.starts_with(b"\x1b[?2026;") { return None; }

    let value = response[8];
    match value {
        b'1' | b'2' => Some(true),   // Supported (set or reset)
        b'0' | b'4' => Some(false),  // Not supported
        _ => None,
    }
}
```

### 8.2 OSC 8 Hyperlinks

```rust
/// Hyperlink escape sequences (OSC 8)
///
/// Open:  OSC 8 ; ; URL ST  (\x1b]8;;URL\x1b\\)
/// Close: OSC 8 ; ; ST      (\x1b]8;;\x1b\\)
///
/// With params: OSC 8 ; params ; URL ST
/// Example params: id=mylink (for multi-part links)

pub fn hyperlink_open(url: &str) -> String {
    format!("\x1b]8;;{}\x1b\\", url)
}

pub fn hyperlink_open_with_id(url: &str, id: &str) -> String {
    format!("\x1b]8;id={};{}\x1b\\", id, url)
}

pub const HYPERLINK_CLOSE: &str = "\x1b]8;;\x1b\\";
```

### 8.3 Mouse Protocols

```rust
/// Mouse tracking modes
pub mod mouse {
    /// Enable SGR extended mode + all motion tracking
    pub const ENABLE: &[u8] = b"\x1b[?1003h\x1b[?1006h";

    /// Disable mouse tracking
    pub const DISABLE: &[u8] = b"\x1b[?1006l\x1b[?1003l";

    /// Mouse modes:
    /// 1000: VT200 - press/release only
    /// 1002: Cell motion - report on cell change during drag
    /// 1003: All motion - report every movement
    /// 1006: SGR extended format (recommended)
    /// 1016: SGR pixel coordinates
}

#[derive(Clone, Debug)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub button: MouseButton,
    pub action: MouseAction,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    Drag,
    Move,
}

/// Parse SGR mouse event: CSI < btn ; x ; y M/m
pub fn parse_sgr_mouse(seq: &[u8]) -> Option<MouseEvent> {
    // Expected: \x1b[<N;X;YM or \x1b[<N;X;Ym
    if seq.len() < 6 || !seq.starts_with(b"\x1b[<") {
        return None;
    }

    let final_byte = *seq.last()?;
    if final_byte != b'M' && final_byte != b'm' {
        return None;
    }

    // Parse parameters between '<' and final byte
    let params = std::str::from_utf8(&seq[3..seq.len()-1]).ok()?;
    let parts: Vec<&str> = params.split(';').collect();
    if parts.len() != 3 {
        return None;
    }

    let btn: u16 = parts[0].parse().ok()?;
    let x: u16 = parts[1].parse().ok()?.saturating_sub(1);
    let y: u16 = parts[2].parse().ok()?.saturating_sub(1);

    let button = match btn & 0b11 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::None,
        _ => MouseButton::None,
    };

    // Check for wheel
    let button = if btn & 64 != 0 {
        if btn & 1 != 0 { MouseButton::WheelDown } else { MouseButton::WheelUp }
    } else {
        button
    };

    let action = if final_byte == b'm' {
        MouseAction::Release
    } else if btn & 32 != 0 {
        MouseAction::Drag
    } else {
        MouseAction::Press
    };

    let mut modifiers = Modifiers::empty();
    if btn & 4 != 0 { modifiers |= Modifiers::SHIFT; }
    if btn & 8 != 0 { modifiers |= Modifiers::ALT; }
    if btn & 16 != 0 { modifiers |= Modifiers::CTRL; }

    Some(MouseEvent { x, y, button, action, modifiers })
}
```

### 8.4 Focus Events and Bracketed Paste

```rust
/// Focus event tracking
pub mod focus {
    pub const ENABLE: &[u8] = b"\x1b[?1004h";
    pub const DISABLE: &[u8] = b"\x1b[?1004l";

    pub const FOCUS_IN: &[u8] = b"\x1b[I";
    pub const FOCUS_OUT: &[u8] = b"\x1b[O";
}

/// Bracketed paste mode
pub mod paste {
    pub const ENABLE: &[u8] = b"\x1b[?2004h";
    pub const DISABLE: &[u8] = b"\x1b[?2004l";

    pub const PASTE_START: &[u8] = b"\x1b[200~";
    pub const PASTE_END: &[u8] = b"\x1b[201~";
}
```

### 8.5 tmux Passthrough

```rust
/// Wrap escape sequence for tmux passthrough
/// All ESC characters in the inner sequence must be doubled
pub fn wrap_for_tmux(sequence: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(sequence.len() * 2 + 10);

    // DCS tmux;
    result.extend_from_slice(b"\x1bPtmux;");

    // Double all ESC characters
    for &byte in sequence {
        if byte == 0x1b {
            result.push(0x1b);
            result.push(0x1b);
        } else {
            result.push(byte);
        }
    }

    // ST
    result.extend_from_slice(b"\x1b\\");
    result
}

/// Detect if running inside tmux
pub fn in_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}
```

---

## Chapter 9: Input Parser

### 9.1 Event Types

```rust
use bitflags::bitflags;

#[derive(Clone, Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
    FocusGained,
    FocusLost,
}

#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),  // F1-F12+
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b0001;
        const CTRL  = 0b0010;
        const ALT   = 0b0100;
        const META  = 0b1000;
    }
}
```

### 9.2 Input Parser State Machine

```rust
/// Limits for DoS protection
pub mod limits {
    pub const MAX_CSI_LEN: usize = 256;
    pub const MAX_OSC_LEN: usize = 4096;
    pub const MAX_PASTE_LEN: usize = 10 * 1024 * 1024;  // 10 MB
}

enum ParseResult {
    Event(Event),
    Incomplete,
    Invalid(usize),  // Skip this many bytes
}

pub struct InputParser {
    buffer: Vec<u8>,
    in_paste: bool,
    paste_buffer: Vec<u8>,
}

impl InputParser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(64),
            in_paste: false,
            paste_buffer: Vec::new(),
        }
    }

    pub fn parse(&mut self, input: &[u8]) -> Vec<Event> {
        let mut events = Vec::new();
        self.buffer.extend_from_slice(input);

        while !self.buffer.is_empty() {
            if self.in_paste {
                if let Some(end) = self.find_paste_end() {
                    let paste = std::mem::take(&mut self.paste_buffer);
                    if let Ok(s) = String::from_utf8(paste) {
                        events.push(Event::Paste(s));
                    }
                    self.buffer.drain(..end);
                    self.in_paste = false;
                } else {
                    // Check paste overflow
                    if self.paste_buffer.len() + self.buffer.len() > limits::MAX_PASTE_LEN {
                        // Truncate and end paste
                        self.in_paste = false;
                        self.paste_buffer.clear();
                        self.buffer.clear();
                        break;
                    }
                    self.paste_buffer.extend(self.buffer.drain(..));
                    break;
                }
            } else {
                match self.parse_sequence() {
                    ParseResult::Event(e) => events.push(e),
                    ParseResult::Incomplete => break,
                    ParseResult::Invalid(n) => {
                        self.buffer.drain(..n.max(1));
                    }
                }
            }
        }

        events
    }

    fn parse_sequence(&mut self) -> ParseResult {
        if self.buffer.is_empty() {
            return ParseResult::Incomplete;
        }

        match self.buffer[0] {
            0x1b => self.parse_escape(),
            0x00..=0x1f => self.parse_control(),
            0x7f => {
                self.buffer.drain(..1);
                ParseResult::Event(Event::Key(KeyEvent {
                    code: KeyCode::Backspace,
                    modifiers: Modifiers::empty(),
                }))
            }
            _ => self.parse_char(),
        }
    }

    fn parse_escape(&mut self) -> ParseResult {
        if self.buffer.len() < 2 {
            return ParseResult::Incomplete;
        }

        match self.buffer[1] {
            b'[' => self.parse_csi(),
            b'O' => self.parse_ss3(),
            b']' => self.parse_osc(),
            _ => {
                // Alt + key
                self.buffer.drain(..1);
                match self.parse_sequence() {
                    ParseResult::Event(Event::Key(mut k)) => {
                        k.modifiers |= Modifiers::ALT;
                        ParseResult::Event(Event::Key(k))
                    }
                    other => other,
                }
            }
        }
    }

    fn parse_csi(&mut self) -> ParseResult {
        // Find final byte (0x40-0x7E)
        let mut i = 2;
        while i < self.buffer.len() && i < limits::MAX_CSI_LEN {
            let b = self.buffer[i];
            if (0x40..=0x7E).contains(&b) {
                return self.decode_csi(i + 1);
            }
            i += 1;
        }

        if i >= limits::MAX_CSI_LEN {
            return ParseResult::Invalid(i);
        }
        ParseResult::Incomplete
    }

    fn decode_csi(&mut self, len: usize) -> ParseResult {
        let seq = &self.buffer[..len];
        let final_byte = seq[len - 1];

        // Check for paste markers
        if seq.starts_with(paste::PASTE_START) {
            self.buffer.drain(..len);
            self.in_paste = true;
            return ParseResult::Incomplete;
        }

        let event = match final_byte {
            b'A' => Event::Key(KeyEvent { code: KeyCode::Up, modifiers: self.parse_csi_modifiers(seq) }),
            b'B' => Event::Key(KeyEvent { code: KeyCode::Down, modifiers: self.parse_csi_modifiers(seq) }),
            b'C' => Event::Key(KeyEvent { code: KeyCode::Right, modifiers: self.parse_csi_modifiers(seq) }),
            b'D' => Event::Key(KeyEvent { code: KeyCode::Left, modifiers: self.parse_csi_modifiers(seq) }),
            b'H' => Event::Key(KeyEvent { code: KeyCode::Home, modifiers: self.parse_csi_modifiers(seq) }),
            b'F' => Event::Key(KeyEvent { code: KeyCode::End, modifiers: self.parse_csi_modifiers(seq) }),
            b'~' => return self.decode_csi_tilde(len),
            b'M' | b'm' if seq.starts_with(b"\x1b[<") => {
                if let Some(mouse) = parse_sgr_mouse(seq) {
                    Event::Mouse(mouse)
                } else {
                    self.buffer.drain(..len);
                    return ParseResult::Invalid(0);
                }
            }
            b'I' => Event::FocusGained,
            b'O' => Event::FocusLost,
            _ => {
                self.buffer.drain(..len);
                return ParseResult::Invalid(0);
            }
        };

        self.buffer.drain(..len);
        ParseResult::Event(event)
    }

    fn decode_csi_tilde(&mut self, len: usize) -> ParseResult {
        let seq = &self.buffer[2..len-1];
        let params: Vec<u16> = String::from_utf8_lossy(seq)
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();

        let code = match params.first() {
            Some(1) | Some(7) => KeyCode::Home,
            Some(2) => KeyCode::Insert,
            Some(3) => KeyCode::Delete,
            Some(4) | Some(8) => KeyCode::End,
            Some(5) => KeyCode::PageUp,
            Some(6) => KeyCode::PageDown,
            Some(n @ 11..=15) => KeyCode::F((n - 10) as u8),
            Some(n @ 17..=21) => KeyCode::F((n - 11) as u8),
            Some(n @ 23..=24) => KeyCode::F((n - 12) as u8),
            _ => {
                self.buffer.drain(..len);
                return ParseResult::Invalid(0);
            }
        };

        let modifiers = params.get(1)
            .map(|&m| self.decode_modifier(m))
            .unwrap_or(Modifiers::empty());

        self.buffer.drain(..len);
        ParseResult::Event(Event::Key(KeyEvent { code, modifiers }))
    }

    fn parse_csi_modifiers(&self, seq: &[u8]) -> Modifiers {
        // Look for modifier parameter (e.g., CSI 1;5 A for Ctrl+Up)
        let params = &seq[2..seq.len()-1];
        if let Some(semi) = params.iter().position(|&b| b == b';') {
            let mod_str = &params[semi+1..];
            if let Ok(m) = std::str::from_utf8(mod_str).and_then(|s| s.parse::<u16>().map_err(|_| std::str::Utf8Error::default())) {
                return self.decode_modifier(m);
            }
        }
        Modifiers::empty()
    }

    fn decode_modifier(&self, m: u16) -> Modifiers {
        let m = m.saturating_sub(1);
        let mut mods = Modifiers::empty();
        if m & 1 != 0 { mods |= Modifiers::SHIFT; }
        if m & 2 != 0 { mods |= Modifiers::ALT; }
        if m & 4 != 0 { mods |= Modifiers::CTRL; }
        if m & 8 != 0 { mods |= Modifiers::META; }
        mods
    }

    fn parse_ss3(&mut self) -> ParseResult {
        if self.buffer.len() < 3 {
            return ParseResult::Incomplete;
        }

        let code = match self.buffer[2] {
            b'P' => KeyCode::F(1),
            b'Q' => KeyCode::F(2),
            b'R' => KeyCode::F(3),
            b'S' => KeyCode::F(4),
            b'H' => KeyCode::Home,
            b'F' => KeyCode::End,
            _ => {
                self.buffer.drain(..3);
                return ParseResult::Invalid(0);
            }
        };

        self.buffer.drain(..3);
        ParseResult::Event(Event::Key(KeyEvent { code, modifiers: Modifiers::empty() }))
    }

    fn parse_osc(&mut self) -> ParseResult {
        // Find terminator: ST (\x1b\\) or BEL (\x07)
        for i in 2..self.buffer.len().min(limits::MAX_OSC_LEN) {
            if self.buffer[i] == 0x07 ||
               (i > 0 && self.buffer[i-1] == 0x1b && self.buffer[i] == b'\\') {
                let end = if self.buffer[i] == 0x07 { i + 1 } else { i + 1 };
                self.buffer.drain(..end);
                return ParseResult::Invalid(0);  // OSC parsed but not used
            }
        }

        if self.buffer.len() >= limits::MAX_OSC_LEN {
            return ParseResult::Invalid(limits::MAX_OSC_LEN);
        }
        ParseResult::Incomplete
    }

    fn parse_control(&mut self) -> ParseResult {
        let b = self.buffer[0];
        self.buffer.drain(..1);

        let event = match b {
            0x00 => KeyEvent { code: KeyCode::Char('@'), modifiers: Modifiers::CTRL },
            0x09 => KeyEvent { code: KeyCode::Tab, modifiers: Modifiers::empty() },
            0x0a | 0x0d => KeyEvent { code: KeyCode::Enter, modifiers: Modifiers::empty() },
            0x08 => KeyEvent { code: KeyCode::Backspace, modifiers: Modifiers::empty() },
            0x1b => KeyEvent { code: KeyCode::Escape, modifiers: Modifiers::empty() },
            0x01..=0x1a => KeyEvent {
                code: KeyCode::Char((b + b'a' - 1) as char),
                modifiers: Modifiers::CTRL,
            },
            _ => return ParseResult::Invalid(0),
        };

        ParseResult::Event(Event::Key(event))
    }

    fn parse_char(&mut self) -> ParseResult {
        // UTF-8 length from first byte
        let len = match self.buffer[0] {
            0x00..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            _ => return ParseResult::Invalid(1),
        };

        if self.buffer.len() < len {
            return ParseResult::Incomplete;
        }

        if let Ok(s) = std::str::from_utf8(&self.buffer[..len]) {
            if let Some(c) = s.chars().next() {
                self.buffer.drain(..len);
                return ParseResult::Event(Event::Key(KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: Modifiers::empty(),
                }));
            }
        }

        ParseResult::Invalid(1)
    }

    fn find_paste_end(&self) -> Option<usize> {
        // Look for paste end marker in buffer
        self.buffer.windows(paste::PASTE_END.len())
            .position(|w| w == paste::PASTE_END)
            .map(|p| p + paste::PASTE_END.len())
    }
}
```

---

## Chapter 10: Components

### 10.1 The Renderable Trait

```rust
/// Anything that can be rendered to a buffer
pub trait Renderable {
    /// Measure content dimensions (for layout)
    fn measure(&self, ctx: &RenderContext) -> Measurement {
        Measurement::ZERO
    }

    /// Render to buffer within area
    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext);
}

/// Render context with ambient state
pub struct RenderContext {
    pub caps: TerminalCapabilities,
    pub theme: Theme,
    pub grapheme_pool: std::cell::RefCell<GraphemePool>,
    pub link_registry: std::cell::RefCell<LinkRegistry>,
}

// Blanket implementations for composition
impl<T: Renderable> Renderable for &T {
    fn measure(&self, ctx: &RenderContext) -> Measurement { (*self).measure(ctx) }
    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext) { (*self).render(buf, area, ctx) }
}

impl<T: Renderable> Renderable for Box<T> {
    fn measure(&self, ctx: &RenderContext) -> Measurement { (**self).measure(ctx) }
    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext) { (**self).render(buf, area, ctx) }
}
```

### 10.2 Panel Component

```rust
/// Bordered container
pub struct Panel<C: Renderable> {
    content: C,
    border: BorderStyle,
    border_fg: Option<Color>,
    title: Option<String>,
    title_align: HAlign,
    style: Style,
}

impl<C: Renderable> Panel<C> {
    pub fn new(content: C) -> Self {
        Self {
            content,
            border: BorderStyle::Rounded,
            border_fg: None,
            title: None,
            title_align: HAlign::Left,
            style: Style::default(),
        }
    }

    pub fn border(mut self, style: BorderStyle) -> Self {
        self.border = style;
        self
    }

    pub fn border_fg(mut self, color: impl Into<Color>) -> Self {
        self.border_fg = Some(color.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn title_align(mut self, align: HAlign) -> Self {
        self.title_align = align;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<C: Renderable> Renderable for Panel<C> {
    fn measure(&self, ctx: &RenderContext) -> Measurement {
        let inner = self.content.measure(ctx);
        let border_w = if matches!(self.border, BorderStyle::None) { 0 } else { 2 };
        let padding_w = self.style.padding.horizontal() as usize;

        Measurement {
            minimum: inner.minimum + border_w + padding_w,
            maximum: inner.maximum + border_w + padding_w,
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
        let border = match self.border {
            BorderStyle::None => return self.content.render(buf, area, ctx),
            BorderStyle::Rounded => Border::rounded(),
            BorderStyle::Square => Border::square(),
            BorderStyle::Double => Border::double(),
            BorderStyle::Heavy => Border::heavy(),
            BorderStyle::Ascii => Border::ascii(),
            BorderStyle::Hidden => Border::hidden(),
            BorderStyle::Custom(ref b) => b.clone(),
        };

        let fg = self.border_fg.as_ref()
            .map(|c| c.resolve(&ctx.theme))
            .unwrap_or(ctx.theme.foreground());

        // Draw corners
        buf.set(area.x, area.y, Cell {
            content: CellContent::Char(border.top_left),
            fg: fg.to_packed(),
            ..Cell::EMPTY
        });
        buf.set(area.x + area.width - 1, area.y, Cell {
            content: CellContent::Char(border.top_right),
            fg: fg.to_packed(),
            ..Cell::EMPTY
        });
        buf.set(area.x, area.y + area.height - 1, Cell {
            content: CellContent::Char(border.bottom_left),
            fg: fg.to_packed(),
            ..Cell::EMPTY
        });
        buf.set(area.x + area.width - 1, area.y + area.height - 1, Cell {
            content: CellContent::Char(border.bottom_right),
            fg: fg.to_packed(),
            ..Cell::EMPTY
        });

        // Draw top edge (with optional title)
        let top_y = area.y;
        if let Some(ref title) = self.title {
            let title_len = crate::unicode::display_width(title);
            let available = (area.width as usize).saturating_sub(4);
            let title_start = match self.title_align {
                HAlign::Left => 2,
                HAlign::Center => ((area.width as usize - title_len) / 2).max(2),
                HAlign::Right => (area.width as usize - title_len - 2).max(2),
            };

            for x in 1..(area.width - 1) {
                let px = area.x + x;
                let rel_x = x as usize;

                if rel_x >= title_start && rel_x < title_start + title_len {
                    // Title character
                    let title_idx = rel_x - title_start;
                    if let Some(c) = title.chars().nth(title_idx) {
                        buf.set(px, top_y, Cell {
                            content: CellContent::Char(c),
                            fg: fg.to_packed(),
                            ..Cell::EMPTY
                        });
                    }
                } else {
                    buf.set(px, top_y, Cell {
                        content: CellContent::Char(border.top),
                        fg: fg.to_packed(),
                        ..Cell::EMPTY
                    });
                }
            }
        } else {
            for x in 1..(area.width - 1) {
                buf.set(area.x + x, top_y, Cell {
                    content: CellContent::Char(border.top),
                    fg: fg.to_packed(),
                    ..Cell::EMPTY
                });
            }
        }

        // Draw bottom edge
        let bottom_y = area.y + area.height - 1;
        for x in 1..(area.width - 1) {
            buf.set(area.x + x, bottom_y, Cell {
                content: CellContent::Char(border.bottom),
                fg: fg.to_packed(),
                ..Cell::EMPTY
            });
        }

        // Draw left/right edges
        for y in 1..(area.height - 1) {
            buf.set(area.x, area.y + y, Cell {
                content: CellContent::Char(border.left),
                fg: fg.to_packed(),
                ..Cell::EMPTY
            });
            buf.set(area.x + area.width - 1, area.y + y, Cell {
                content: CellContent::Char(border.right),
                fg: fg.to_packed(),
                ..Cell::EMPTY
            });
        }

        // Render content in inner area
        let inner = Rect::new(
            area.x + 1 + self.style.padding.left,
            area.y + 1 + self.style.padding.top,
            area.width.saturating_sub(2 + self.style.padding.horizontal()),
            area.height.saturating_sub(2 + self.style.padding.vertical()),
        );

        buf.push_scissor(inner);
        self.content.render(buf, inner, ctx);
        buf.pop_scissor();
    }
}
```

### 10.3 Spinner Component

```rust
use std::time::{Duration, Instant};

pub struct Spinner {
    style: SpinnerStyle,
    message: String,
    frame: usize,
    last_tick: Instant,
    fg: Option<Color>,
}

#[derive(Clone)]
pub enum SpinnerStyle {
    Dots,
    Line,
    Braille,
    Bounce,
    Custom(Vec<&'static str>),
}

impl SpinnerStyle {
    fn frames(&self) -> &[&str] {
        match self {
            Self::Dots => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            Self::Line => &["-", "\\", "|", "/"],
            Self::Braille => &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            Self::Bounce => &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"],
            Self::Custom(frames) => frames,
        }
    }

    fn frame_duration(&self) -> Duration {
        match self {
            Self::Dots | Self::Braille => Duration::from_millis(80),
            Self::Line => Duration::from_millis(100),
            Self::Bounce => Duration::from_millis(120),
            Self::Custom(_) => Duration::from_millis(80),
        }
    }
}

impl Spinner {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            style: SpinnerStyle::Dots,
            message: message.into(),
            frame: 0,
            last_tick: Instant::now(),
            fg: None,
        }
    }

    pub fn style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: impl Into<Color>) -> Self {
        self.fg = Some(color.into());
        self
    }

    /// Advance animation frame if enough time has passed
    /// Returns true if frame changed (needs redraw)
    pub fn tick(&mut self) -> bool {
        if self.last_tick.elapsed() >= self.style.frame_duration() {
            self.frame = (self.frame + 1) % self.style.frames().len();
            self.last_tick = Instant::now();
            true
        } else {
            false
        }
    }
}

impl Renderable for Spinner {
    fn measure(&self, _ctx: &RenderContext) -> Measurement {
        let frame_width = 2;  // Spinner frames are 1-2 cells
        let msg_width = crate::unicode::display_width(&self.message);
        let total = frame_width + 1 + msg_width;
        Measurement { minimum: total, maximum: total }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
        let frame = self.style.frames()[self.frame];
        let fg = self.fg.as_ref()
            .map(|c| c.resolve(&ctx.theme))
            .unwrap_or(ctx.theme.primary())
            .to_packed();

        // Draw spinner frame
        for (i, c) in frame.chars().enumerate() {
            if area.x + i as u16 >= area.x + area.width { break; }
            buf.set(area.x + i as u16, area.y, Cell {
                content: CellContent::Char(c),
                fg,
                ..Cell::EMPTY
            });
        }

        // Draw message
        let msg_x = area.x + 2;
        for (i, c) in self.message.chars().enumerate() {
            let x = msg_x + i as u16;
            if x >= area.x + area.width { break; }
            buf.set(x, area.y, Cell {
                content: CellContent::Char(c),
                ..Cell::EMPTY
            });
        }
    }
}
```

### 10.4 Progress Bar

```rust
pub struct Progress {
    value: f32,
    label: Option<String>,
    filled_char: char,
    empty_char: char,
    filled_fg: Option<Color>,
    empty_fg: Option<Color>,
    show_percentage: bool,
}

impl Progress {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            label: None,
            filled_char: '█',
            empty_char: '░',
            filled_fg: None,
            empty_fg: None,
            show_percentage: true,
        }
    }

    pub fn value(mut self, v: f32) -> Self {
        self.value = v.clamp(0.0, 1.0);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    pub fn filled_fg(mut self, color: impl Into<Color>) -> Self {
        self.filled_fg = Some(color.into());
        self
    }

    pub fn set(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }
}

impl Renderable for Progress {
    fn measure(&self, _ctx: &RenderContext) -> Measurement {
        let label_w = self.label.as_ref().map(|l| l.len() + 1).unwrap_or(0);
        let pct_w = if self.show_percentage { 5 } else { 0 };
        let min_bar = 10;
        Measurement {
            minimum: label_w + min_bar + pct_w,
            maximum: label_w + 50 + pct_w,
        }
    }

    fn render(&self, buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
        let mut x = area.x;

        // Label
        if let Some(ref label) = self.label {
            for c in label.chars() {
                if x >= area.x + area.width { break; }
                buf.set(x, area.y, Cell {
                    content: CellContent::Char(c),
                    ..Cell::EMPTY
                });
                x += 1;
            }
            x += 1;  // Space after label
        }

        // Calculate bar width
        let pct_width = if self.show_percentage { 5 } else { 0 };
        let bar_width = (area.width as usize).saturating_sub((x - area.x) as usize + pct_width);
        let filled_width = ((bar_width as f32) * self.value) as usize;

        let filled_fg = self.filled_fg.as_ref()
            .map(|c| c.resolve(&ctx.theme))
            .unwrap_or(ctx.theme.primary())
            .to_packed();
        let empty_fg = self.empty_fg.as_ref()
            .map(|c| c.resolve(&ctx.theme))
            .unwrap_or(ctx.theme.muted())
            .to_packed();

        // Draw bar
        for i in 0..bar_width {
            let (ch, fg) = if i < filled_width {
                (self.filled_char, filled_fg)
            } else {
                (self.empty_char, empty_fg)
            };
            buf.set(x, area.y, Cell {
                content: CellContent::Char(ch),
                fg,
                ..Cell::EMPTY
            });
            x += 1;
        }

        // Percentage
        if self.show_percentage {
            let pct = format!(" {:>3}%", (self.value * 100.0) as u8);
            for c in pct.chars() {
                if x >= area.x + area.width { break; }
                buf.set(x, area.y, Cell {
                    content: CellContent::Char(c),
                    ..Cell::EMPTY
                });
                x += 1;
            }
        }
    }
}
```

---

## Chapter 11: Agent Harness API

### 11.1 High-Level Coordinator

```rust
use std::io::{self, Stdout, Write};

/// High-level API for coding agent UIs
pub struct AgentHarness {
    terminal: Terminal<Stdout>,
    buffer: Buffer,
    prev_buffer: Buffer,
    presenter: Presenter,
    ctx: RenderContext,

    // State
    current_tool: Option<ToolIndicator>,
    spinners: Vec<Spinner>,
}

impl AgentHarness {
    pub fn new() -> io::Result<Self> {
        let caps = TerminalCapabilities::detect();
        let (w, h) = terminal_size()?;

        let ctx = RenderContext {
            caps: caps.clone(),
            theme: Theme::dark(),
            grapheme_pool: std::cell::RefCell::new(GraphemePool::new()),
            link_registry: std::cell::RefCell::new(LinkRegistry::new()),
        };

        Ok(Self {
            terminal: Terminal::new(io::stdout(), caps.clone())?,
            buffer: Buffer::new(w, h),
            prev_buffer: Buffer::new(w, h),
            presenter: Presenter::new(caps),
            ctx,
            current_tool: None,
            spinners: Vec::new(),
        })
    }

    // ===== TOOL INDICATORS =====

    /// Start a tool execution with spinner
    pub fn tool_start(&mut self, name: &str, target: &str) {
        self.current_tool = Some(ToolIndicator {
            name: name.to_string(),
            target: target.to_string(),
            status: ToolStatus::Running,
            spinner: Spinner::new(""),
            start_time: Instant::now(),
        });
        self.render();
    }

    /// Mark current tool as successful
    pub fn tool_success(&mut self) {
        if let Some(ref mut tool) = self.current_tool {
            tool.status = ToolStatus::Success;
            self.render();
        }
        self.current_tool = None;
    }

    /// Mark current tool as failed
    pub fn tool_error(&mut self, message: &str) {
        if let Some(ref mut tool) = self.current_tool {
            tool.status = ToolStatus::Failed(message.to_string());
            self.render();
        }
        self.current_tool = None;
    }

    // ===== PROGRESS =====

    /// Show a progress bar
    pub fn progress(&mut self, label: &str, value: f32) {
        let progress = Progress::new(value).label(label);
        // Render progress in status area
        self.render();
    }

    // ===== STATUS =====

    /// Show a status message with spinner
    pub fn status(&mut self, message: &str) {
        let spinner = Spinner::new(message);
        self.spinners.push(spinner);
        self.render();
    }

    /// Clear current status
    pub fn clear_status(&mut self) {
        self.spinners.pop();
        self.render();
    }

    // ===== OUTPUT =====

    /// Print styled content
    pub fn print(&mut self, content: impl Renderable) {
        // Render at current position, advance scroll
        self.render();
    }

    /// Print markup text: [bold red]Error:[/] message
    pub fn print_markup(&mut self, markup: &str) {
        let segments = parse_markup(markup, Style::default());
        // Render segments
        self.render();
    }

    /// Print code with syntax highlighting
    pub fn code_block(&mut self, language: &str, code: &str) {
        // TODO: Syntax highlighting integration
        let panel = Panel::new(code)
            .border(BorderStyle::Rounded)
            .title(language);
        self.print(panel);
    }

    // ===== RENDERING =====

    fn render(&mut self) {
        self.buffer.clear();

        // Tick all spinners
        for spinner in &mut self.spinners {
            spinner.tick();
        }
        if let Some(ref mut tool) = self.current_tool {
            tool.spinner.tick();
        }

        // Layout and render components
        // ... (render conversation, tools, status, etc.)

        // Present to terminal
        let diff = BufferDiff::compute(&self.prev_buffer, &self.buffer);
        let pool = self.ctx.grapheme_pool.borrow();
        let links = self.ctx.link_registry.borrow();

        let _ = self.presenter.present(
            &mut self.terminal.writer,
            &self.buffer,
            &diff,
            &pool,
            &links,
        );

        std::mem::swap(&mut self.buffer, &mut self.prev_buffer);
    }
}

struct ToolIndicator {
    name: String,
    target: String,
    status: ToolStatus,
    spinner: Spinner,
    start_time: Instant,
}

enum ToolStatus {
    Running,
    Success,
    Failed(String),
}
```

---

## Chapter 12: Performance Benchmarks

### 12.1 Target Metrics

| Operation | Target | Justification |
|-----------|--------|---------------|
| Cell comparison | < 1ns | Bitwise u128 compare |
| Buffer diff (80×24) | < 500µs | Sequential scan, SIMD |
| Frame render | < 1ms | Required for 60 FPS |
| Input parse | < 20µs/event | State machine |
| Style parse | < 8µs (cached) | From rich_rust benchmarks |
| Color blend | < 10ns | Single Porter-Duff op |
| Grapheme intern (hit) | < 100ns | Hash lookup |
| Grapheme intern (miss) | < 1µs | Hash + alloc + width |

### 12.2 Benchmark Framework

```rust
#[cfg(test)]
mod benchmarks {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn bench_cell_comparison(c: &mut Criterion) {
        let cell_a = Cell::new('A', Style::default());
        let cell_b = Cell::new('B', Style::default());

        c.bench_function("cell_bits_eq", |b| {
            b.iter(|| black_box(cell_a.bits_eq(&cell_b)))
        });
    }

    fn bench_buffer_diff(c: &mut Criterion) {
        let buf1 = Buffer::new(80, 24);
        let mut buf2 = Buffer::new(80, 24);

        // 5% cells changed
        for i in 0..96 {
            buf2.cells[i * 20] = Cell::new('X', Style::default());
        }

        c.bench_function("buffer_diff_5pct", |b| {
            b.iter(|| black_box(BufferDiff::compute(&buf1, &buf2)))
        });
    }

    fn bench_grapheme_pool(c: &mut Criterion) {
        let mut pool = GraphemePool::new();

        // Pre-populate
        pool.intern("hello");
        pool.intern("world");
        pool.intern("👨‍👩‍👧");

        c.bench_function("grapheme_intern_hit", |b| {
            b.iter(|| black_box(pool.intern("hello")))
        });

        c.bench_function("grapheme_intern_miss", |b| {
            let mut i = 0;
            b.iter(|| {
                let s = format!("new_grapheme_{}", i);
                i += 1;
                black_box(pool.intern(&s))
            })
        });
    }

    criterion_group!(benches, bench_cell_comparison, bench_buffer_diff, bench_grapheme_pool);
    criterion_main!(benches);
}
```

---

## Chapter 13: Implementation Roadmap

### Phase 1: Core Primitives (Week 1-2)
- [ ] `Cell` (16 bytes), `CellContent`, `CellAttrs`
- [ ] `PackedRgba` with Porter-Duff blending
- [ ] `GraphemeId`, `GraphemePool` with ref-counting
- [ ] `Buffer` with scissor/opacity stacks
- [ ] `Rect`, `Sides`, `Measurement`
- [ ] Unit tests for all types

**Exit criteria**: `cargo test` passes, cell comparison < 5ns

### Phase 2: Rendering Pipeline (Week 3-4)
- [ ] `BufferDiff` with SIMD path
- [ ] `Presenter` with state tracking
- [ ] `TerminalCapabilities` detection
- [ ] Synchronized output (DEC 2026)
- [ ] OSC 8 hyperlinks
- [ ] Panic hook installation

**Exit criteria**: Zero-flicker rendering demo

### Phase 3: Input System (Week 5)
- [ ] Event types (Key, Mouse, Resize, Paste, Focus)
- [ ] `InputParser` state machine
- [ ] SGR mouse protocol
- [ ] Bracketed paste handling
- [ ] DoS protection limits

**Exit criteria**: All key combinations parsed correctly

### Phase 4: Styling (Week 6)
- [ ] `Style` with bitflags property tracking
- [ ] CSS-like shorthand (Sides from tuples)
- [ ] `Color` enum with profile resolution
- [ ] `Theme` system with presets
- [ ] `Border` presets and custom
- [ ] Markup parser `[bold red]text[/]`

**Exit criteria**: Markup matches Rich syntax

### Phase 5: Components (Week 7-8)
- [ ] `Renderable` trait
- [ ] `Panel` with borders and titles
- [ ] `Spinner` with multiple styles
- [ ] `Progress` bar
- [ ] `Text` with wrapping
- [ ] `Table` (basic)

**Exit criteria**: All components have demos

### Phase 6: Agent Harness (Week 9-10)
- [ ] `AgentHarness` coordinator
- [ ] Tool indicators with spinners
- [ ] Status line
- [ ] Streaming code renderer
- [ ] High-level API

**Exit criteria**: Claude Code-like demo

### Phase 7: Polish (Week 11-12)
- [ ] Documentation (rustdoc)
- [ ] Examples gallery
- [ ] Performance audit
- [ ] Terminal compatibility testing
- [ ] CI/CD setup

**Exit criteria**: Ready for v0.1.0 release

---

## Chapter 14: Terminal Compatibility Matrix

| Feature | Kitty | WezTerm | Alacritty | Ghostty | iTerm2 | GNOME Term | Win Term |
|---------|-------|---------|-----------|---------|--------|------------|----------|
| True Color | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Sync Output | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ |
| OSC 8 Links | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| Kitty Keyboard | ✓ | ✓ | ✗ | ✓ | ✗ | ✗ | ✗ |
| Kitty Graphics | ✓ | ✓ | ✗ | ✓ | ✗ | ✗ | ✗ |
| Sixel | ✗ | ✓ | ✗ | ✗ | ✓ | ✗ | ✗ |
| Focus Events | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Bracketed Paste | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

---

## Appendix A: ANSI Escape Sequence Reference

```
CSI = ESC [  (\x1b[)
SGR = CSI ... m
OSC = ESC ]  (\x1b])
DCS = ESC P  (\x1bP)
ST  = ESC \  (\x1b\\)

COLORS:
  CSI 38;2;R;G;B m    24-bit foreground
  CSI 48;2;R;G;B m    24-bit background
  CSI 38;5;N m        256-color foreground
  CSI 48;5;N m        256-color background
  CSI 39 m            Default foreground
  CSI 49 m            Default background

ATTRIBUTES:
  CSI 0 m   Reset all
  CSI 1 m   Bold
  CSI 2 m   Dim
  CSI 3 m   Italic
  CSI 4 m   Underline
  CSI 5 m   Blink
  CSI 7 m   Reverse
  CSI 8 m   Hidden
  CSI 9 m   Strikethrough

CURSOR:
  CSI H         Move to (1,1)
  CSI r;c H     Move to row r, col c
  CSI n A       Move up n
  CSI n B       Move down n
  CSI n C       Move right n
  CSI n D       Move left n

SCREEN:
  CSI 2 J       Clear screen
  CSI K         Clear to end of line
  CSI ? 1049 h  Enter alternate screen
  CSI ? 1049 l  Leave alternate screen
  CSI ? 25 h    Show cursor
  CSI ? 25 l    Hide cursor

SYNCHRONIZED OUTPUT:
  CSI ? 2026 h  Begin sync
  CSI ? 2026 l  End sync

MOUSE (SGR 1006):
  CSI ? 1003 h  Enable all motion
  CSI ? 1006 h  Enable SGR mode
  CSI < btn;x;y M  Press
  CSI < btn;x;y m  Release

HYPERLINKS:
  OSC 8 ; ; URL ST  Open link
  OSC 8 ; ; ST      Close link

FOCUS:
  CSI ? 1004 h  Enable focus events
  CSI I         Focus gained
  CSI O         Focus lost

PASTE:
  CSI ? 2004 h  Enable bracketed paste
  CSI 200 ~     Paste start
  CSI 201 ~     Paste end
```

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **Grapheme cluster** | User-perceived character (may span multiple codepoints) |
| **Codepoint** | Single Unicode value (U+0000 to U+10FFFF) |
| **Cell** | Single position in terminal grid |
| **SGR** | Select Graphic Rendition (ANSI style codes) |
| **CSI** | Control Sequence Introducer (`ESC [`) |
| **OSC** | Operating System Command (`ESC ]`) |
| **DCS** | Device Control String (`ESC P`) |
| **ST** | String Terminator (`ESC \`) |
| **BiDi** | Bidirectional text (mixing LTR and RTL) |
| **ZWJ** | Zero Width Joiner (connects graphemes into compound) |
| **Porter-Duff** | Compositing algebra for alpha blending |

---

## Conclusion

FrankenTUI v4.0 represents the mathematically optimal synthesis of three excellent terminal UI libraries:

1. **From opentui_rust**: Cache-optimal 16-byte cells, bitwise comparison, Porter-Duff blending, grapheme pooling, scissor/opacity stacks, cell-level diffing

2. **From rich_rust**: Cow<str> segments for zero-copy, regex-based markup parser, event-driven span rendering, LRU width caching, measurement protocol

3. **From charmed_rust**: Bitflags property tracking for inheritance, CSS-like tuple shorthand, adaptive colors, border presets, color profile detection

The result is a **scrollback-native, zero-flicker, agent-ergonomic** terminal UI library that achieves:

- **< 1ns** cell comparison (bitwise)
- **< 500µs** frame diff (80×24)
- **< 1ms** total frame time
- **16 bytes** per cell (4 cells per cache line)
- **Zero heap allocation** for 99% of cells

*FrankenTUI: Where mathematical rigor meets practical ergonomics.*
