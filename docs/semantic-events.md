# Semantic Events: Widget Migration Guide

High-level input events derived from raw terminal input. This guide explains when
to use semantic events vs. raw events, how to configure gesture recognition, and
how to migrate existing widgets.

---

## Overview

FrankenTUI provides two ways to handle user input:

1. **Raw Events** (`Event`) — Low-level terminal events: key presses, mouse
   button states, cursor positions.

2. **Semantic Events** (`SemanticEvent`) — High-level user *intentions*:
   double-click, drag, long-press, key chords.

The `GestureRecognizer` converts raw events into semantic events. Your model
receives **both** streams — you choose which to consume.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Terminal Input                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Event (ftui-core)                           │
│         Key, Mouse, Resize, Focus, Paste, Tick               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              GestureRecognizer (ftui-core)                   │
│    Click detection, drag tracking, chord sequences           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              SemanticEvent (ftui-core)                       │
│    Click, DoubleClick, DragStart, DragEnd, Chord, etc.       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Model.update()                            │
│         Receives both Event and SemanticEvent                │
└─────────────────────────────────────────────────────────────┘
```

---

## Semantic vs. Raw Events

### Raw Events (`Event`)

```rust
pub enum Event {
    Key(KeyEvent),      // Key press/release/repeat
    Mouse(MouseEvent),  // Down/Up/Drag/Move/Scroll
    Resize { width: u16, height: u16 },
    Paste(PasteEvent),
    Focus(bool),
    Clipboard(ClipboardEvent),
    Tick,
}
```

**Use raw events when:**

- You need precise control over input timing
- You're building custom gesture detection
- You want to handle key repeats explicitly
- You need mouse scroll events
- You're processing paste or clipboard data

### Semantic Events (`SemanticEvent`)

```rust
pub enum SemanticEvent {
    // Mouse gestures
    Click { pos: Position, button: MouseButton },
    DoubleClick { pos: Position, button: MouseButton },
    TripleClick { pos: Position, button: MouseButton },
    LongPress { pos: Position, duration: Duration },

    // Drag gestures
    DragStart { pos: Position, button: MouseButton },
    DragMove { start: Position, current: Position, delta: (i16, i16) },
    DragEnd { start: Position, end: Position },
    DragCancel,

    // Keyboard gestures
    Chord { sequence: Vec<ChordKey> },

    // Touch-like gestures
    Swipe { direction: SwipeDirection, distance: u16, velocity: f32 },
}
```

**Use semantic events when:**

- You want double-click to select words
- You're implementing drag-and-drop
- You need key chord support (Ctrl+K, Ctrl+C)
- You want long-press context menus
- You prefer higher-level, portable input handling

---

## When to Use Which

| Scenario | Use Raw | Use Semantic |
|----------|---------|--------------|
| Text input field | ✓ | |
| Button click | | ✓ |
| Double-click to edit | | ✓ |
| Drag to reorder items | | ✓ |
| Scroll wheel handling | ✓ | |
| Key repeat for movement | ✓ | |
| Long-press context menu | | ✓ |
| Vim-like key sequences | | ✓ |
| Custom gesture detection | ✓ | |
| Game-like input | ✓ | |

---

## Configuration

Configure gesture recognition with `GestureConfig`:

```rust
use ftui_core::gesture::{GestureConfig, GestureRecognizer};
use std::time::Duration;

let config = GestureConfig {
    // Time window for double/triple click (default: 300ms)
    multi_click_timeout: Duration::from_millis(300),

    // Duration before long press fires (default: 500ms)
    long_press_threshold: Duration::from_millis(500),

    // Cells of movement before drag starts (default: 3)
    drag_threshold: 3,

    // Time window for chord completion (default: 1000ms)
    chord_timeout: Duration::from_millis(1000),

    // Velocity for swipe detection (default: 50.0 cells/sec)
    swipe_velocity_threshold: 50.0,

    // Position tolerance for multi-click (default: 1 cell)
    click_tolerance: 1,
};

let recognizer = GestureRecognizer::new(config);
```

### Configuration Guidelines

| Parameter | Too Low | Too High |
|-----------|---------|----------|
| `multi_click_timeout` | Missed double-clicks | False double-clicks from separate clicks |
| `long_press_threshold` | Accidental triggers | Feels unresponsive |
| `drag_threshold` | Accidental drags | Drags feel "sticky" |
| `chord_timeout` | Missed chords | Stale keys combine |
| `click_tolerance` | Position must be exact | Separate clicks merge |

---

## Migration Guide

### Before: Raw Events Only

```rust
enum Msg {
    Event(Event),
}

impl Model for MyApp {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Event(Event::Mouse(mouse)) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Start potential drag or click
                        self.mouse_down_pos = Some((mouse.x, mouse.y));
                        self.mouse_down_time = Some(Instant::now());
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        if let Some(down_pos) = self.mouse_down_pos.take() {
                            // Was it a click or drag? Check distance...
                            let dx = (mouse.x as i32 - down_pos.0 as i32).abs();
                            let dy = (mouse.y as i32 - down_pos.1 as i32).abs();
                            if dx + dy < 3 {
                                // It's a click! But was it a double-click?
                                if let Some(last) = self.last_click {
                                    let elapsed = self.mouse_down_time
                                        .unwrap()
                                        .duration_since(last);
                                    if elapsed < Duration::from_millis(300) {
                                        self.on_double_click(mouse.x, mouse.y);
                                    } else {
                                        self.on_click(mouse.x, mouse.y);
                                    }
                                }
                                self.last_click = self.mouse_down_time;
                            } else {
                                // It was a drag
                                self.on_drag_end(down_pos, (mouse.x, mouse.y));
                            }
                        }
                    }
                    MouseEventKind::Drag(_) => {
                        // Manual drag tracking...
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Cmd::None
    }
}
```

### After: Semantic Events

```rust
use ftui_core::semantic_event::{SemanticEvent, Position};

enum Msg {
    Event(Event),
    Semantic(SemanticEvent),
}

impl Model for MyApp {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            // Handle semantic events for gestures
            Msg::Semantic(semantic) => match semantic {
                SemanticEvent::Click { pos, button } => {
                    self.on_click(pos.x, pos.y);
                }
                SemanticEvent::DoubleClick { pos, button } => {
                    self.on_double_click(pos.x, pos.y);
                }
                SemanticEvent::DragStart { pos, button } => {
                    self.start_drag(pos);
                }
                SemanticEvent::DragMove { start, current, delta } => {
                    self.update_drag(current, delta);
                }
                SemanticEvent::DragEnd { start, end } => {
                    self.finish_drag(start, end);
                }
                SemanticEvent::DragCancel => {
                    self.cancel_drag();
                }
                SemanticEvent::LongPress { pos, duration } => {
                    self.show_context_menu(pos);
                }
                _ => {}
            },

            // Still handle raw events for text input, scroll, etc.
            Msg::Event(Event::Key(key)) => {
                self.handle_key_input(key);
            }
            Msg::Event(Event::Mouse(mouse)) => {
                if let MouseEventKind::ScrollDown | MouseEventKind::ScrollUp = mouse.kind {
                    self.handle_scroll(mouse);
                }
            }
            _ => {}
        }
        Cmd::None
    }
}
```

---

## Examples

### Example 1: List with Double-Click to Edit

```rust
use ftui_core::semantic_event::{SemanticEvent, Position};

struct ListModel {
    items: Vec<String>,
    selected: Option<usize>,
    editing: Option<usize>,
}

enum Msg {
    Semantic(SemanticEvent),
    Event(Event),
    // ... other messages
}

impl Model for ListModel {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Semantic(SemanticEvent::Click { pos, .. }) => {
                // Single click selects
                self.selected = self.item_at_position(pos);
                self.editing = None;
            }
            Msg::Semantic(SemanticEvent::DoubleClick { pos, .. }) => {
                // Double click edits
                if let Some(idx) = self.item_at_position(pos) {
                    self.editing = Some(idx);
                }
            }
            Msg::Semantic(SemanticEvent::TripleClick { pos, .. }) => {
                // Triple click selects all text in item
                if let Some(idx) = self.item_at_position(pos) {
                    self.select_all_in_item(idx);
                }
            }
            _ => {}
        }
        Cmd::None
    }

    fn item_at_position(&self, pos: Position) -> Option<usize> {
        // Convert position to item index
        let row = pos.y as usize;
        if row < self.items.len() {
            Some(row)
        } else {
            None
        }
    }
}
```

### Example 2: Drag-and-Drop Reordering

```rust
use ftui_core::semantic_event::{SemanticEvent, Position};

struct ReorderableList {
    items: Vec<String>,
    drag_source: Option<usize>,
    drag_target: Option<usize>,
}

enum Msg {
    Semantic(SemanticEvent),
    Event(Event),
}

impl Model for ReorderableList {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Semantic(SemanticEvent::DragStart { pos, .. }) => {
                self.drag_source = self.item_at(pos.y);
                self.drag_target = self.drag_source;
            }
            Msg::Semantic(SemanticEvent::DragMove { current, .. }) => {
                self.drag_target = self.item_at(current.y);
            }
            Msg::Semantic(SemanticEvent::DragEnd { start, end }) => {
                if let (Some(from), Some(to)) = (self.drag_source, self.drag_target) {
                    if from != to {
                        let item = self.items.remove(from);
                        let insert_at = if to > from { to - 1 } else { to };
                        self.items.insert(insert_at, item);
                    }
                }
                self.drag_source = None;
                self.drag_target = None;
            }
            Msg::Semantic(SemanticEvent::DragCancel) => {
                self.drag_source = None;
                self.drag_target = None;
            }
            _ => {}
        }
        Cmd::None
    }

    fn item_at(&self, y: u16) -> Option<usize> {
        let idx = y as usize;
        if idx < self.items.len() { Some(idx) } else { None }
    }
}
```

### Example 3: Key Chords (Vim-Style)

```rust
use ftui_core::semantic_event::{SemanticEvent, ChordKey};
use ftui_core::event::{KeyCode, Modifiers};

struct Editor {
    content: String,
    // ...
}

enum Msg {
    Semantic(SemanticEvent),
    Event(Event),
}

impl Model for Editor {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Semantic(SemanticEvent::Chord { sequence }) => {
                self.handle_chord(&sequence);
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_chord(&mut self, sequence: &[ChordKey]) {
        // Match common chord sequences
        match sequence.as_slice() {
            // Ctrl+K, Ctrl+C → Comment line
            [ChordKey { code: KeyCode::Char('k'), modifiers: m1 },
             ChordKey { code: KeyCode::Char('c'), modifiers: m2 }]
            if m1.contains(Modifiers::CTRL) && m2.contains(Modifiers::CTRL) => {
                self.comment_line();
            }

            // Ctrl+K, Ctrl+U → Uncomment line
            [ChordKey { code: KeyCode::Char('k'), modifiers: m1 },
             ChordKey { code: KeyCode::Char('u'), modifiers: m2 }]
            if m1.contains(Modifiers::CTRL) && m2.contains(Modifiers::CTRL) => {
                self.uncomment_line();
            }

            // Ctrl+K, Ctrl+D → Delete line
            [ChordKey { code: KeyCode::Char('k'), modifiers: m1 },
             ChordKey { code: KeyCode::Char('d'), modifiers: m2 }]
            if m1.contains(Modifiers::CTRL) && m2.contains(Modifiers::CTRL) => {
                self.delete_line();
            }

            _ => {}
        }
    }
}
```

### Example 4: Long-Press Context Menu

```rust
use ftui_core::semantic_event::{SemanticEvent, Position};
use std::time::Duration;

struct FileList {
    files: Vec<PathBuf>,
    context_menu: Option<ContextMenu>,
}

struct ContextMenu {
    position: Position,
    file_index: usize,
}

enum Msg {
    Semantic(SemanticEvent),
    Event(Event),
    MenuAction(MenuAction),
}

impl Model for FileList {
    type Message = Msg;

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Semantic(SemanticEvent::Click { pos, .. }) => {
                // Close context menu on click
                self.context_menu = None;
            }
            Msg::Semantic(SemanticEvent::LongPress { pos, duration }) => {
                // Show context menu on long press
                if let Some(idx) = self.file_at(pos) {
                    self.context_menu = Some(ContextMenu {
                        position: pos,
                        file_index: idx,
                    });
                }
            }
            _ => {}
        }
        Cmd::None
    }
}
```

---

## Invariants

The gesture recognizer guarantees:

1. **Drag sequences are well-formed**: Every `DragStart` is followed by
   zero or more `DragMove` events, ending with exactly one `DragEnd` or
   `DragCancel`.

2. **Click and Drag are mutually exclusive**: A mouse-down/mouse-up
   sequence produces either click events OR drag events, never both.

3. **Click multiplicity is monotonic**: Within a multi-click window,
   you see `Click` → `DoubleClick` → `TripleClick` in order.

4. **Chords are non-empty**: A `Chord` event always contains at least
   two keys in its sequence.

5. **Focus loss cancels drags**: Losing window focus emits `DragCancel`
   if a drag was in progress.

---

## Failure Modes

| Failure | Cause | Result |
|---------|-------|--------|
| Chord timeout | Keys pressed too slowly | No chord emitted; raw keys pass through |
| Focus loss mid-drag | Window deactivated | `DragCancel` emitted |
| Escape mid-drag | User pressed Escape | `DragCancel` emitted |
| Click outside threshold | Movement beyond `click_tolerance` | Resets to single click |

---

## Testing

### Unit Test Helpers

```rust
use ftui_core::event::{Event, MouseEvent, MouseEventKind, MouseButton, Modifiers};
use ftui_core::gesture::{GestureRecognizer, GestureConfig};
use std::time::Instant;

fn mouse_down(x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        x,
        y,
        modifiers: Modifiers::NONE,
    })
}

fn mouse_up(x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        x,
        y,
        modifiers: Modifiers::NONE,
    })
}

#[test]
fn test_double_click() {
    let mut gr = GestureRecognizer::new(GestureConfig::default());
    let t = Instant::now();
    let dt = Duration::from_millis(50);

    // First click
    gr.process(&mouse_down(5, 5), t);
    gr.process(&mouse_up(5, 5), t + dt);

    // Second click within timeout
    gr.process(&mouse_down(5, 5), t + dt * 2);
    let events = gr.process(&mouse_up(5, 5), t + dt * 3);

    assert!(matches!(events[0], SemanticEvent::DoubleClick { .. }));
}
```

---

## Related Documentation

- [Semantic Events Specification](spec/semantic-events.md) — Formal spec
- [Drag-and-Drop Protocol](../crates/ftui-widgets/src/drag.rs) — Widget integration
- [Gesture Recognizer Source](../crates/ftui-core/src/gesture.rs) — Implementation
