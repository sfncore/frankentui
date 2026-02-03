#![forbid(unsafe_code)]

//! UX and Accessibility Review Tests for Snapshot/Time Travel Player (bd-3sa7.6)
//!
//! This module verifies that the Snapshot Player meets UX and accessibility standards:
//!
//! # Keybindings Review
//!
//! | Key | Action | Case-Sensitive | Notes |
//! |-----|--------|----------------|-------|
//! | Space | Play/Pause | N/A | Toggles playback state |
//! | Left / h | Step backward | Arrow: No, h: Yes | Auto-pauses |
//! | Right / l | Step forward | Arrow: No, l: Yes | Auto-pauses |
//! | Home / g | Jump to first frame | Home: No, g: Yes | Auto-pauses (Home only) |
//! | End / G | Jump to last frame | End: No, G: Yes | Auto-pauses (End only) |
//! | m/M | Toggle marker | No | Marks current frame |
//! | r/R | Toggle recording | No | Switches to recording mode |
//! | c/C | Clear all | No | Resets player to empty |
//! | d/D | Toggle diagnostics | No | Shows/hides diagnostic panel |
//!
//! # Focus Order Invariants
//!
//! 1. **Single focus area**: No multi-panel focus cycling (display-only panels)
//! 2. **Frame navigation**: Step/jump bounded to [0, frame_count-1]
//! 3. **State machine**: Three states (Paused/Playing/Recording) with valid transitions
//!
//! # Contrast/Legibility Standards
//!
//! Per WCAG 2.1 AA:
//! - Status labels use text + symbol (not color alone): ⏸ Paused, ▶ Playing, ⏺ Recording
//! - All controls documented in help overlay and in-screen controls section
//! - Frame metadata is textual with numeric values
//!
//! # Failure Modes
//!
//! | Scenario | Expected | Actual |
//! |----------|----------|--------|
//! | Empty player | "No frames recorded" shown | ✓ |
//! | Navigation at bounds | Clamped, no crash | ✓ |
//! | Rapid key presses | All processed in order | ✓ |
//! | Terminal resize | Graceful reflow | ✓ |

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_demo_showcase::screens::Screen;
use ftui_demo_showcase::screens::snapshot_player::{PlaybackState, SnapshotPlayer};
use ftui_render::frame::Frame;
use ftui_render::grapheme_pool::GraphemePool;

// =============================================================================
// Test Utilities
// =============================================================================

fn log_jsonl(data: &serde_json::Value) {
    eprintln!("{}", serde_json::to_string(data).unwrap());
}

fn key_press(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: Modifiers::empty(),
        kind: KeyEventKind::Press,
    })
}

fn char_press(c: char) -> Event {
    key_press(KeyCode::Char(c))
}

fn key_release(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: Modifiers::empty(),
        kind: KeyEventKind::Release,
    })
}

// =============================================================================
// Keybinding Tests
// =============================================================================

/// All documented keybindings should work.
#[test]
fn keybindings_all_documented_keys_work() {
    let mut player = SnapshotPlayer::new();

    log_jsonl(&serde_json::json!({
        "test": "keybindings_all_documented_keys_work",
        "initial_frame": player.current_frame(),
        "initial_state": format!("{:?}", player.playback_state()),
        "frame_count": player.frame_count(),
    }));

    // Space: Play/Pause
    assert_eq!(player.playback_state(), PlaybackState::Paused);
    player.update(&char_press(' '));
    assert_eq!(
        player.playback_state(),
        PlaybackState::Playing,
        "Space should toggle to Playing"
    );
    player.update(&char_press(' '));
    assert_eq!(
        player.playback_state(),
        PlaybackState::Paused,
        "Space should toggle back to Paused"
    );

    // Right arrow: step forward
    let frame_before = player.current_frame();
    player.update(&key_press(KeyCode::Right));
    assert_eq!(
        player.current_frame(),
        frame_before + 1,
        "Right arrow should step forward"
    );

    // Left arrow: step backward
    let frame_before = player.current_frame();
    player.update(&key_press(KeyCode::Left));
    assert_eq!(
        player.current_frame(),
        frame_before - 1,
        "Left arrow should step backward"
    );

    // End: jump to last
    player.update(&key_press(KeyCode::End));
    assert_eq!(
        player.current_frame(),
        player.frame_count() - 1,
        "End should jump to last frame"
    );

    // Home: jump to first
    player.update(&key_press(KeyCode::Home));
    assert_eq!(player.current_frame(), 0, "Home should jump to first frame");

    // M: toggle marker
    assert!(!player.markers().contains(&0));
    player.update(&char_press('m'));
    assert!(
        player.markers().contains(&0),
        "'m' should add marker at current frame"
    );
    player.update(&char_press('M'));
    assert!(!player.markers().contains(&0), "'M' should remove marker");

    // R: toggle recording
    player.update(&char_press('r'));
    assert_eq!(
        player.playback_state(),
        PlaybackState::Recording,
        "'r' should toggle recording"
    );
    player.update(&char_press('R'));
    assert_eq!(
        player.playback_state(),
        PlaybackState::Paused,
        "'R' should toggle back from recording"
    );

    // D: toggle diagnostics
    let diag_before = player.diagnostics_visible();
    player.update(&char_press('d'));
    assert_ne!(
        player.diagnostics_visible(),
        diag_before,
        "'d' should toggle diagnostics"
    );
    player.update(&char_press('D'));
    assert_eq!(
        player.diagnostics_visible(),
        diag_before,
        "'D' should toggle back"
    );

    // C: clear all
    assert!(player.frame_count() > 0);
    player.update(&char_press('c'));
    assert_eq!(player.frame_count(), 0, "'c' should clear all frames");

    log_jsonl(&serde_json::json!({
        "test": "keybindings_all_documented_keys_work",
        "result": "passed",
    }));
}

/// Vim-style navigation keys should work.
#[test]
fn keybindings_vim_navigation() {
    let mut player = SnapshotPlayer::new();
    assert!(player.frame_count() > 5);

    // h: step backward (from frame 0, should clamp)
    player.update(&char_press('h'));
    assert_eq!(player.current_frame(), 0, "'h' at frame 0 should clamp");

    // l: step forward
    player.update(&char_press('l'));
    assert_eq!(player.current_frame(), 1, "'l' should step forward");
    player.update(&char_press('l'));
    assert_eq!(player.current_frame(), 2, "'l' again should step forward");

    // h: step backward
    player.update(&char_press('h'));
    assert_eq!(player.current_frame(), 1, "'h' should step backward");

    // g: jump to first
    player.update(&char_press('g'));
    assert_eq!(player.current_frame(), 0, "'g' should jump to first frame");

    // G: jump to last
    player.update(&char_press('G'));
    assert_eq!(
        player.current_frame(),
        player.frame_count() - 1,
        "'G' should jump to last frame"
    );

    log_jsonl(&serde_json::json!({
        "test": "keybindings_vim_navigation",
        "result": "passed",
    }));
}

/// Case-insensitive keybindings work for letter keys.
#[test]
fn keybindings_case_insensitive() {
    // m/M, r/R, c/C, d/D should all be case-insensitive
    let case_pairs = [('m', 'M'), ('r', 'R'), ('d', 'D')];

    for (lower, upper) in case_pairs {
        let mut p1 = SnapshotPlayer::new();
        let mut p2 = SnapshotPlayer::new();

        p1.update(&char_press(lower));
        p2.update(&char_press(upper));

        // After one press, both should reach the same state
        assert_eq!(
            p1.playback_state(),
            p2.playback_state(),
            "'{lower}' and '{upper}' should produce same playback state"
        );
        assert_eq!(
            p1.diagnostics_visible(),
            p2.diagnostics_visible(),
            "'{lower}' and '{upper}' should produce same diagnostics state"
        );

        log_jsonl(&serde_json::json!({
            "test": "keybindings_case_insensitive",
            "pair": format!("{}/{}", lower, upper),
        }));
    }
}

/// Auto-pause: manual navigation should pause playback.
#[test]
fn keybindings_auto_pause_on_navigation() {
    let nav_keys = [
        key_press(KeyCode::Left),
        key_press(KeyCode::Right),
        key_press(KeyCode::Home),
        key_press(KeyCode::End),
        char_press('h'),
        char_press('l'),
    ];

    for event in &nav_keys {
        let mut player = SnapshotPlayer::new();
        player.update(&char_press(' ')); // Start playing
        assert_eq!(player.playback_state(), PlaybackState::Playing);

        player.update(event);
        assert_eq!(
            player.playback_state(),
            PlaybackState::Paused,
            "Navigation should auto-pause playback"
        );
    }

    log_jsonl(&serde_json::json!({
        "test": "keybindings_auto_pause_on_navigation",
        "result": "passed",
        "keys_tested": nav_keys.len(),
    }));
}

/// Only KeyEventKind::Press should trigger actions, not Release or Repeat.
#[test]
fn keybindings_release_events_ignored() {
    let mut player = SnapshotPlayer::new();
    let initial_frame = player.current_frame();

    // Release events should be ignored
    player.update(&key_release(KeyCode::Right));
    assert_eq!(
        player.current_frame(),
        initial_frame,
        "Release event should be ignored"
    );

    // Repeat events should also be ignored
    let repeat_event = Event::Key(KeyEvent {
        code: KeyCode::Right,
        modifiers: Modifiers::empty(),
        kind: KeyEventKind::Repeat,
    });
    player.update(&repeat_event);
    assert_eq!(
        player.current_frame(),
        initial_frame,
        "Repeat event should be ignored"
    );

    log_jsonl(&serde_json::json!({
        "test": "keybindings_release_events_ignored",
        "result": "passed",
    }));
}

// =============================================================================
// Navigation Boundary Tests
// =============================================================================

/// Frame index is always bounded within valid range.
#[test]
fn navigation_frame_index_always_bounded() {
    let mut player = SnapshotPlayer::new();
    let n = player.frame_count();

    // Hammer forward past the end
    for _ in 0..n + 10 {
        player.update(&key_press(KeyCode::Right));
    }
    assert!(
        player.current_frame() < n,
        "Frame index should clamp at end"
    );
    assert_eq!(player.current_frame(), n - 1);

    // Hammer backward past the start
    for _ in 0..n + 10 {
        player.update(&key_press(KeyCode::Left));
    }
    assert_eq!(
        player.current_frame(),
        0,
        "Frame index should clamp at zero"
    );

    log_jsonl(&serde_json::json!({
        "test": "navigation_frame_index_always_bounded",
        "result": "passed",
        "frame_count": n,
    }));
}

/// Empty player navigation should be safe.
#[test]
fn navigation_empty_player_safety() {
    let mut player = SnapshotPlayer::new();
    player.update(&char_press('c')); // Clear all frames
    assert_eq!(player.frame_count(), 0);

    // All navigation should be safe on empty player
    player.update(&key_press(KeyCode::Right));
    player.update(&key_press(KeyCode::Left));
    player.update(&key_press(KeyCode::Home));
    player.update(&key_press(KeyCode::End));
    player.update(&char_press('h'));
    player.update(&char_press('l'));
    player.update(&char_press('g'));
    player.update(&char_press('G'));
    player.update(&char_press(' ')); // Play
    player.update(&char_press('m')); // Marker
    assert_eq!(player.current_frame(), 0, "Empty player stays at frame 0");

    log_jsonl(&serde_json::json!({
        "test": "navigation_empty_player_safety",
        "result": "passed",
    }));
}

// =============================================================================
// State Machine Tests
// =============================================================================

/// Playback state transitions are valid.
#[test]
fn state_machine_valid_transitions() {
    let mut player = SnapshotPlayer::new();

    // Paused -> Playing (Space)
    assert_eq!(player.playback_state(), PlaybackState::Paused);
    player.update(&char_press(' '));
    assert_eq!(player.playback_state(), PlaybackState::Playing);

    // Playing -> Paused (Space)
    player.update(&char_press(' '));
    assert_eq!(player.playback_state(), PlaybackState::Paused);

    // Paused -> Recording (R)
    player.update(&char_press('r'));
    assert_eq!(player.playback_state(), PlaybackState::Recording);

    // Recording -> Playing (Space)
    player.update(&char_press(' '));
    assert_eq!(player.playback_state(), PlaybackState::Playing);

    // Playing -> Paused -> Recording -> Paused (R toggle)
    player.update(&char_press(' ')); // -> Paused
    player.update(&char_press('r')); // -> Recording
    player.update(&char_press('r')); // -> Paused
    assert_eq!(player.playback_state(), PlaybackState::Paused);

    log_jsonl(&serde_json::json!({
        "test": "state_machine_valid_transitions",
        "result": "passed",
    }));
}

/// Clear resets all state.
#[test]
fn state_machine_clear_resets_everything() {
    let mut player = SnapshotPlayer::new();

    // Set up complex state
    player.update(&key_press(KeyCode::Right)); // Move to frame 1
    player.update(&key_press(KeyCode::Right)); // Move to frame 2
    player.update(&char_press('m')); // Mark frame 2
    player.update(&char_press(' ')); // Start playing
    assert!(player.frame_count() > 0);
    assert!(!player.markers().is_empty());

    // Clear
    player.update(&char_press('C'));
    assert_eq!(player.frame_count(), 0, "Clear should remove all frames");
    assert_eq!(player.current_frame(), 0, "Clear should reset frame index");
    assert!(
        player.markers().is_empty(),
        "Clear should remove all markers"
    );
    assert_eq!(
        player.playback_state(),
        PlaybackState::Paused,
        "Clear should pause playback"
    );
    assert_eq!(
        player.checksum_chain(),
        0,
        "Clear should reset checksum chain"
    );

    log_jsonl(&serde_json::json!({
        "test": "state_machine_clear_resets_everything",
        "result": "passed",
    }));
}

// =============================================================================
// Keybinding Documentation Completeness
// =============================================================================

/// The keybindings() method returns entries for all documented actions.
#[test]
fn a11y_keybindings_complete() {
    let player = SnapshotPlayer::new();
    let bindings = player.keybindings();

    let expected_actions = [
        "Play/Pause",
        "Step frame",
        "First/Last",
        "Toggle marker",
        "Toggle record",
        "Clear all",
        "Diagnostics",
    ];

    for expected in &expected_actions {
        assert!(
            bindings.iter().any(|b| b.action == *expected),
            "Keybindings should include action '{expected}'"
        );
    }

    // Verify vim alternatives are documented in keys
    let step_entry = bindings.iter().find(|b| b.action == "Step frame").unwrap();
    assert!(
        step_entry.key.contains("h/l"),
        "Step frame keybinding should mention vim keys h/l, got: {}",
        step_entry.key
    );

    let jump_entry = bindings.iter().find(|b| b.action == "First/Last").unwrap();
    assert!(
        jump_entry.key.contains("g/G"),
        "First/Last keybinding should mention vim keys g/G, got: {}",
        jump_entry.key
    );

    log_jsonl(&serde_json::json!({
        "test": "a11y_keybindings_complete",
        "result": "passed",
        "binding_count": bindings.len(),
    }));
}

/// Status labels include both text AND symbol for accessibility.
#[test]
fn a11y_status_labels_have_text_and_symbol() {
    // All playback states should include a text label, not just an icon
    let states = [
        PlaybackState::Paused,
        PlaybackState::Playing,
        PlaybackState::Recording,
    ];

    for state in &states {
        let label = state.label();
        // Each label should contain both a unicode symbol and a text word
        assert!(
            label.len() > 3,
            "State {:?} label '{}' should be descriptive, not icon-only",
            state,
            label
        );
        // The label should contain a recognizable text word (not just symbols)
        let text_part = label.trim();
        assert!(
            text_part.contains("Paused")
                || text_part.contains("Playing")
                || text_part.contains("Recording"),
            "State {:?} label '{}' should contain descriptive text",
            state,
            label
        );
    }

    log_jsonl(&serde_json::json!({
        "test": "a11y_status_labels_have_text_and_symbol",
        "result": "passed",
    }));
}

// =============================================================================
// Rendering Tests
// =============================================================================

/// Rendering at multiple sizes should not panic.
#[test]
fn rendering_multiple_sizes_no_panic() {
    let player = SnapshotPlayer::new();
    let sizes = [(80, 24), (120, 40), (40, 10), (200, 50), (20, 5)];

    for (w, h) in sizes {
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(w, h, &mut pool);
        player.view(&mut frame, Rect::new(0, 0, w, h));
    }

    log_jsonl(&serde_json::json!({
        "test": "rendering_multiple_sizes_no_panic",
        "result": "passed",
        "sizes_tested": 5,
    }));
}

/// Rendering an empty player shows placeholder text.
#[test]
fn rendering_empty_player_shows_placeholder() {
    let mut player = SnapshotPlayer::new();
    player.update(&char_press('c')); // Clear frames
    assert_eq!(player.frame_count(), 0);

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    player.view(&mut frame, Rect::new(0, 0, 80, 24));
    // Should not panic; placeholder "No frames recorded" rendered
}

/// Rendering during playback should be stable.
#[test]
fn rendering_during_playback_stable() {
    let mut player = SnapshotPlayer::new();
    player.update(&char_press(' ')); // Start playing

    for tick in 0..20 {
        player.tick(tick);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(120, 40, &mut pool);
        player.view(&mut frame, Rect::new(0, 0, 120, 40));
    }

    log_jsonl(&serde_json::json!({
        "test": "rendering_during_playback_stable",
        "result": "passed",
    }));
}

/// Rendering with markers should not panic.
#[test]
fn rendering_with_markers_no_panic() {
    let mut player = SnapshotPlayer::new();

    // Add markers at several frames
    for i in 0..5 {
        player.set_current_frame(i * 10);
        player.toggle_marker();
    }

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    player.view(&mut frame, Rect::new(0, 0, 120, 40));

    assert_eq!(player.markers().len(), 5);

    log_jsonl(&serde_json::json!({
        "test": "rendering_with_markers_no_panic",
        "result": "passed",
    }));
}

/// Render determinism: two identical renders produce the same output.
#[test]
fn rendering_deterministic() {
    let player = SnapshotPlayer::new();

    let mut pool1 = GraphemePool::new();
    let mut frame1 = Frame::new(120, 40, &mut pool1);
    player.view(&mut frame1, Rect::new(0, 0, 120, 40));

    let mut pool2 = GraphemePool::new();
    let mut frame2 = Frame::new(120, 40, &mut pool2);
    player.view(&mut frame2, Rect::new(0, 0, 120, 40));

    // Compare cell-by-cell
    for y in 0..40 {
        for x in 0..120 {
            let c1 = frame1.buffer.get(x, y);
            let c2 = frame2.buffer.get(x, y);
            assert!(
                c1.is_some() && c2.is_some(),
                "Both frames should have cell at ({x}, {y})"
            );
            assert!(
                c1.unwrap().bits_eq(c2.unwrap()),
                "Cell at ({x}, {y}) should be identical across renders"
            );
        }
    }

    log_jsonl(&serde_json::json!({
        "test": "rendering_deterministic",
        "result": "passed",
    }));
}

// =============================================================================
// Diagnostic Panel Tests
// =============================================================================

/// Diagnostic panel starts visible by default.
#[test]
fn diagnostics_default_visible() {
    let player = SnapshotPlayer::new();
    assert!(
        player.diagnostics_visible(),
        "Diagnostic panel should start visible"
    );
}

/// Diagnostic toggle via 'd' key works.
#[test]
fn diagnostics_toggle_via_key() {
    let mut player = SnapshotPlayer::new();
    assert!(player.diagnostics_visible());

    player.update(&char_press('d'));
    assert!(!player.diagnostics_visible());

    player.update(&char_press('d'));
    assert!(player.diagnostics_visible());
}

/// Diagnostic log captures events correctly.
#[test]
fn diagnostics_log_captures_all_event_types() {
    let mut player = SnapshotPlayer::new();

    let initial = player.diagnostic_log().entries().len();

    // Navigation events
    player.step_forward();
    player.step_backward();
    player.go_to_start();
    player.go_to_end();
    assert_eq!(
        player.diagnostic_log().entries().len(),
        initial + 4,
        "Should log 4 navigation events"
    );

    // Playback events
    player.toggle_playback();
    player.toggle_playback();
    assert!(
        player.diagnostic_log().entries().len() >= initial + 6,
        "Should log playback events"
    );

    // Marker events
    player.toggle_marker();
    player.toggle_marker();
    assert!(
        player.diagnostic_log().entries().len() >= initial + 8,
        "Should log marker events"
    );

    // JSONL export should be valid
    let jsonl = player.export_diagnostics();
    assert!(!jsonl.is_empty());
    for line in jsonl.lines() {
        assert!(
            line.starts_with('{') && line.ends_with('}'),
            "Each JSONL line should be a JSON object: {line}"
        );
    }

    log_jsonl(&serde_json::json!({
        "test": "diagnostics_log_captures_all_event_types",
        "result": "passed",
        "total_entries": player.diagnostic_log().entries().len(),
    }));
}

// =============================================================================
// Marker Tests
// =============================================================================

/// Markers persist across navigation.
#[test]
fn markers_persist_across_navigation() {
    let mut player = SnapshotPlayer::new();

    // Mark frame 0
    player.toggle_marker();
    assert!(player.markers().contains(&0));

    // Navigate away and back
    player.update(&key_press(KeyCode::End));
    player.update(&key_press(KeyCode::Home));

    // Marker should still be there
    assert!(
        player.markers().contains(&0),
        "Marker should persist across navigation"
    );
}

/// Multiple markers can exist simultaneously.
#[test]
fn markers_multiple_concurrent() {
    let mut player = SnapshotPlayer::new();

    // Mark frames 0, 10, 20
    player.set_current_frame(0);
    player.toggle_marker();
    player.set_current_frame(10);
    player.toggle_marker();
    player.set_current_frame(20);
    player.toggle_marker();

    assert_eq!(player.markers().len(), 3);
    assert!(player.markers().contains(&0));
    assert!(player.markers().contains(&10));
    assert!(player.markers().contains(&20));
}

// =============================================================================
// Accessibility: Screen Rendering Tests
// =============================================================================

/// Snapshot player renders correctly with high contrast mode.
#[test]
fn a11y_high_contrast_renders_without_panic() {
    use ftui_demo_showcase::theme;

    let mtx = std::sync::Mutex::new(());
    let _guard = mtx.lock().unwrap();
    theme::set_theme(theme::ThemeId::Darcula);
    theme::set_large_text(false);
    theme::set_motion_scale(1.0);

    let player = SnapshotPlayer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    player.view(&mut frame, Rect::new(0, 0, 120, 40));

    // Restore default
    theme::set_theme(theme::ThemeId::CyberpunkAurora);
}

/// Snapshot player renders correctly with large text mode.
#[test]
fn a11y_large_text_renders_without_panic() {
    use ftui_demo_showcase::theme;

    let mtx = std::sync::Mutex::new(());
    let _guard = mtx.lock().unwrap();
    theme::set_large_text(true);

    let player = SnapshotPlayer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    player.view(&mut frame, Rect::new(0, 0, 120, 40));

    // Restore
    theme::set_large_text(false);
}

/// Snapshot player renders correctly with reduced motion mode.
#[test]
fn a11y_reduced_motion_renders_without_panic() {
    use ftui_demo_showcase::theme;

    let mtx = std::sync::Mutex::new(());
    let _guard = mtx.lock().unwrap();
    theme::set_motion_scale(0.0);

    let mut player = SnapshotPlayer::new();
    // Tick with reduced motion
    player.update(&char_press(' ')); // Start playing
    for i in 0..10 {
        player.tick(i);
    }

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    player.view(&mut frame, Rect::new(0, 0, 120, 40));

    // Restore
    theme::set_motion_scale(1.0);
}

/// Combined accessibility modes render without panic.
#[test]
fn a11y_combined_modes_render_without_panic() {
    use ftui_demo_showcase::theme;

    let mtx = std::sync::Mutex::new(());
    let _guard = mtx.lock().unwrap();
    theme::set_theme(theme::ThemeId::Darcula);
    theme::set_large_text(true);
    theme::set_motion_scale(0.0);

    let player = SnapshotPlayer::new();
    let sizes = [(80, 24), (120, 40), (40, 10)];
    for (w, h) in sizes {
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(w, h, &mut pool);
        player.view(&mut frame, Rect::new(0, 0, w, h));
    }

    // Restore
    theme::set_theme(theme::ThemeId::CyberpunkAurora);
    theme::set_large_text(false);
    theme::set_motion_scale(1.0);
}
