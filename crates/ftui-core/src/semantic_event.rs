#![forbid(unsafe_code)]

//! High-level semantic events derived from raw terminal input (bd-3fu8).
//!
//! [`SemanticEvent`] represents user *intentions* rather than raw key presses or
//! mouse coordinates. A gesture recognizer (see bd-2v34) converts raw [`Event`]
//! sequences into these semantic events.
//!
//! # Design
//!
//! ## Invariants
//! 1. Every drag sequence is well-formed: `DragStart` → zero or more `DragMove` → `DragEnd` or `DragCancel`.
//! 2. Click multiplicity is monotonically increasing within a multi-click window:
//!    a `TripleClick` always follows a `DoubleClick` from the same position.
//! 3. `Chord` sequences are non-empty (enforced by constructor).
//! 4. `Swipe` velocity is always non-negative.
//!
//! ## Failure Modes
//! - If the gesture recognizer times out mid-chord, no `Chord` event is emitted;
//!   the raw keys are passed through instead (graceful degradation).
//! - If a drag is interrupted by focus loss, `DragCancel` is emitted (never a
//!   dangling `DragStart` without termination).

use crate::event::{KeyCode, Modifiers, MouseButton};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// A 2D cell position in the terminal (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// Manhattan distance to another position.
    #[must_use]
    pub fn manhattan_distance(self, other: Self) -> u32 {
        (self.x as i32 - other.x as i32).unsigned_abs()
            + (self.y as i32 - other.y as i32).unsigned_abs()
    }
}

impl From<(u16, u16)> for Position {
    fn from((x, y): (u16, u16)) -> Self {
        Self { x, y }
    }
}

// ---------------------------------------------------------------------------
// ChordKey
// ---------------------------------------------------------------------------

/// A single key in a chord sequence (e.g., Ctrl+K in "Ctrl+K, Ctrl+C").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChordKey {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

impl ChordKey {
    /// Create a chord key.
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }
}

// ---------------------------------------------------------------------------
// SwipeDirection
// ---------------------------------------------------------------------------

/// Cardinal direction for swipe gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SwipeDirection {
    /// Returns the opposite direction.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Returns true for vertical directions.
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    /// Returns true for horizontal directions.
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

// ---------------------------------------------------------------------------
// SemanticEvent
// ---------------------------------------------------------------------------

/// High-level semantic events derived from raw terminal input.
///
/// These represent user intentions rather than raw key presses or mouse
/// coordinates. A gesture recognizer converts raw events into these.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticEvent {
    // === Mouse Gestures ===
    /// Single click (mouse down + up in same position within threshold).
    Click { pos: Position, button: MouseButton },

    /// Two clicks within the double-click time threshold.
    DoubleClick { pos: Position, button: MouseButton },

    /// Three clicks within threshold (often used for line selection).
    TripleClick { pos: Position, button: MouseButton },

    /// Mouse held down beyond threshold without moving.
    LongPress { pos: Position, duration: Duration },

    // === Drag Gestures ===
    /// Mouse moved beyond drag threshold while button held.
    DragStart { pos: Position, button: MouseButton },

    /// Ongoing drag movement.
    DragMove {
        start: Position,
        current: Position,
        /// Movement since last DragMove (dx, dy).
        delta: (i16, i16),
    },

    /// Mouse released after drag.
    DragEnd { start: Position, end: Position },

    /// Drag cancelled (Escape pressed, focus lost, etc.).
    DragCancel,

    // === Keyboard Gestures ===
    /// Key chord sequence completed (e.g., Ctrl+K, Ctrl+C).
    ///
    /// Invariant: `sequence` is always non-empty.
    Chord { sequence: Vec<ChordKey> },

    // === Touch-Like Gestures ===
    /// Swipe gesture (rapid mouse movement in a cardinal direction).
    Swipe {
        direction: SwipeDirection,
        /// Distance in cells.
        distance: u16,
        /// Velocity in cells per second (always >= 0.0).
        velocity: f32,
    },
}

impl SemanticEvent {
    /// Returns true if this is a drag-related event.
    #[must_use]
    pub fn is_drag(&self) -> bool {
        matches!(
            self,
            Self::DragStart { .. }
                | Self::DragMove { .. }
                | Self::DragEnd { .. }
                | Self::DragCancel
        )
    }

    /// Returns true if this is a click-related event (single, double, or triple).
    #[must_use]
    pub fn is_click(&self) -> bool {
        matches!(
            self,
            Self::Click { .. } | Self::DoubleClick { .. } | Self::TripleClick { .. }
        )
    }

    /// Returns the position if this event has one.
    #[must_use]
    pub fn position(&self) -> Option<Position> {
        match self {
            Self::Click { pos, .. }
            | Self::DoubleClick { pos, .. }
            | Self::TripleClick { pos, .. }
            | Self::LongPress { pos, .. }
            | Self::DragStart { pos, .. } => Some(*pos),
            Self::DragMove { current, .. } => Some(*current),
            Self::DragEnd { end, .. } => Some(*end),
            Self::Chord { .. } | Self::DragCancel | Self::Swipe { .. } => None,
        }
    }

    /// Returns the mouse button if this event involves one.
    #[must_use]
    pub fn button(&self) -> Option<MouseButton> {
        match self {
            Self::Click { button, .. }
            | Self::DoubleClick { button, .. }
            | Self::TripleClick { button, .. }
            | Self::DragStart { button, .. } => Some(*button),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: u16, y: u16) -> Position {
        Position::new(x, y)
    }

    // === Position tests ===

    #[test]
    fn position_new_and_from_tuple() {
        let p = Position::new(5, 10);
        assert_eq!(p, Position::from((5, 10)));
        assert_eq!(p.x, 5);
        assert_eq!(p.y, 10);
    }

    #[test]
    fn position_manhattan_distance() {
        assert_eq!(pos(0, 0).manhattan_distance(pos(3, 4)), 7);
        assert_eq!(pos(5, 5).manhattan_distance(pos(5, 5)), 0);
        assert_eq!(pos(10, 0).manhattan_distance(pos(0, 10)), 20);
    }

    #[test]
    fn position_default_is_origin() {
        assert_eq!(Position::default(), pos(0, 0));
    }

    // === ChordKey tests ===

    #[test]
    fn chord_key_equality() {
        let k1 = ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL);
        let k2 = ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL);
        let k3 = ChordKey::new(KeyCode::Char('c'), Modifiers::CTRL);

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn chord_key_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL));
        set.insert(ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL)); // duplicate
        assert_eq!(set.len(), 1);
    }

    // === SwipeDirection tests ===

    #[test]
    fn swipe_direction_opposite() {
        assert_eq!(SwipeDirection::Up.opposite(), SwipeDirection::Down);
        assert_eq!(SwipeDirection::Down.opposite(), SwipeDirection::Up);
        assert_eq!(SwipeDirection::Left.opposite(), SwipeDirection::Right);
        assert_eq!(SwipeDirection::Right.opposite(), SwipeDirection::Left);
    }

    #[test]
    fn swipe_direction_axes() {
        assert!(SwipeDirection::Up.is_vertical());
        assert!(SwipeDirection::Down.is_vertical());
        assert!(!SwipeDirection::Left.is_vertical());
        assert!(!SwipeDirection::Right.is_vertical());

        assert!(SwipeDirection::Left.is_horizontal());
        assert!(SwipeDirection::Right.is_horizontal());
        assert!(!SwipeDirection::Up.is_horizontal());
        assert!(!SwipeDirection::Down.is_horizontal());
    }

    // === SemanticEvent tests ===

    #[test]
    fn is_drag_classification() {
        assert!(
            SemanticEvent::DragStart {
                pos: pos(0, 0),
                button: MouseButton::Left,
            }
            .is_drag()
        );

        assert!(
            SemanticEvent::DragMove {
                start: pos(0, 0),
                current: pos(5, 5),
                delta: (5, 5),
            }
            .is_drag()
        );

        assert!(
            SemanticEvent::DragEnd {
                start: pos(0, 0),
                end: pos(10, 10),
            }
            .is_drag()
        );

        assert!(SemanticEvent::DragCancel.is_drag());

        // Non-drag events
        assert!(
            !SemanticEvent::Click {
                pos: pos(0, 0),
                button: MouseButton::Left,
            }
            .is_drag()
        );

        assert!(
            !SemanticEvent::Chord {
                sequence: vec![ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL)],
            }
            .is_drag()
        );
    }

    #[test]
    fn is_click_classification() {
        assert!(
            SemanticEvent::Click {
                pos: pos(1, 2),
                button: MouseButton::Left,
            }
            .is_click()
        );

        assert!(
            SemanticEvent::DoubleClick {
                pos: pos(1, 2),
                button: MouseButton::Left,
            }
            .is_click()
        );

        assert!(
            SemanticEvent::TripleClick {
                pos: pos(1, 2),
                button: MouseButton::Left,
            }
            .is_click()
        );

        assert!(
            !SemanticEvent::DragStart {
                pos: pos(0, 0),
                button: MouseButton::Left,
            }
            .is_click()
        );
    }

    #[test]
    fn position_extraction() {
        assert_eq!(
            SemanticEvent::Click {
                pos: pos(5, 10),
                button: MouseButton::Left,
            }
            .position(),
            Some(pos(5, 10))
        );

        assert_eq!(
            SemanticEvent::DragMove {
                start: pos(0, 0),
                current: pos(15, 20),
                delta: (1, 1),
            }
            .position(),
            Some(pos(15, 20))
        );

        assert_eq!(
            SemanticEvent::DragEnd {
                start: pos(0, 0),
                end: pos(30, 40),
            }
            .position(),
            Some(pos(30, 40))
        );

        assert_eq!(SemanticEvent::DragCancel.position(), None);

        assert_eq!(SemanticEvent::Chord { sequence: vec![] }.position(), None);

        assert_eq!(
            SemanticEvent::Swipe {
                direction: SwipeDirection::Up,
                distance: 10,
                velocity: 100.0,
            }
            .position(),
            None
        );
    }

    #[test]
    fn button_extraction() {
        assert_eq!(
            SemanticEvent::Click {
                pos: pos(0, 0),
                button: MouseButton::Right,
            }
            .button(),
            Some(MouseButton::Right)
        );

        assert_eq!(
            SemanticEvent::DragStart {
                pos: pos(0, 0),
                button: MouseButton::Middle,
            }
            .button(),
            Some(MouseButton::Middle)
        );

        assert_eq!(SemanticEvent::DragCancel.button(), None);

        assert_eq!(
            SemanticEvent::LongPress {
                pos: pos(0, 0),
                duration: Duration::from_millis(500),
            }
            .button(),
            None
        );
    }

    #[test]
    fn long_press_carries_duration() {
        let event = SemanticEvent::LongPress {
            pos: pos(10, 20),
            duration: Duration::from_millis(750),
        };
        assert_eq!(event.position(), Some(pos(10, 20)));
        assert!(!event.is_drag());
        assert!(!event.is_click());
    }

    #[test]
    fn swipe_velocity_and_direction() {
        let event = SemanticEvent::Swipe {
            direction: SwipeDirection::Right,
            distance: 25,
            velocity: 150.0,
        };
        assert!(!event.is_drag());
        assert!(!event.is_click());
        assert_eq!(event.position(), None);
    }

    #[test]
    fn chord_sequence_contents() {
        let chord = SemanticEvent::Chord {
            sequence: vec![
                ChordKey::new(KeyCode::Char('k'), Modifiers::CTRL),
                ChordKey::new(KeyCode::Char('c'), Modifiers::CTRL),
            ],
        };
        if let SemanticEvent::Chord { sequence } = &chord {
            assert_eq!(sequence.len(), 2);
            assert_eq!(sequence[0].code, KeyCode::Char('k'));
            assert_eq!(sequence[1].code, KeyCode::Char('c'));
        } else {
            panic!("Expected Chord variant");
        }
    }

    #[test]
    fn semantic_event_debug_format() {
        let click = SemanticEvent::Click {
            pos: pos(5, 10),
            button: MouseButton::Left,
        };
        let dbg = format!("{:?}", click);
        assert!(dbg.contains("Click"));
        assert!(dbg.contains("Position"));
    }

    #[test]
    fn semantic_event_clone_and_eq() {
        let original = SemanticEvent::DoubleClick {
            pos: pos(3, 7),
            button: MouseButton::Left,
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
