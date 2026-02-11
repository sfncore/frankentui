#![forbid(unsafe_code)]

//! Snapshot tests for Gas Town dashboard screens in the demo showcase.
//!
//! Each screen is rendered at 80x24 (standard) and 120x40 (wide) with
//! static sample data. Run `BLESS=1 cargo test` to create/update snapshots.

use ftui_core::geometry::Rect;
use ftui_demo_showcase::screens::Screen;
use ftui_demo_showcase::theme::{ScopedThemeLock, ThemeId};
use ftui_harness::assert_snapshot;
use ftui_render::frame::Frame;
use ftui_render::grapheme_pool::GraphemePool;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn snapshot_screen<S: Screen>(screen: &S, width: u16, height: u16, name: &str) {
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(width, height, &mut pool);
    let area = Rect::new(0, 0, width, height);
    screen.view(&mut frame, area);
    assert_snapshot!(name, &frame.buffer);
}

// ---------------------------------------------------------------------------
// Agent Tree
// ---------------------------------------------------------------------------

#[test]
fn agent_tree_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::agent_tree::AgentTree::default();
    snapshot_screen(&screen, 80, 24, "gt_agent_tree_80x24");
}

#[test]
fn agent_tree_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::agent_tree::AgentTree::default();
    snapshot_screen(&screen, 120, 40, "gt_agent_tree_120x40");
}

// ---------------------------------------------------------------------------
// Convoy Panel
// ---------------------------------------------------------------------------

#[test]
fn convoy_panel_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::convoy_panel::ConvoyPanel::default();
    snapshot_screen(&screen, 80, 24, "gt_convoy_panel_80x24");
}

#[test]
fn convoy_panel_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::convoy_panel::ConvoyPanel::default();
    snapshot_screen(&screen, 120, 40, "gt_convoy_panel_120x40");
}

// ---------------------------------------------------------------------------
// Event Feed
// ---------------------------------------------------------------------------

#[test]
fn event_feed_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::event_feed::EventFeed::default();
    snapshot_screen(&screen, 80, 24, "gt_event_feed_80x24");
}

#[test]
fn event_feed_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::event_feed::EventFeed::default();
    snapshot_screen(&screen, 120, 40, "gt_event_feed_120x40");
}

// ---------------------------------------------------------------------------
// Agent Detail
// ---------------------------------------------------------------------------

#[test]
fn agent_detail_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::agent_detail::AgentDetail::default();
    snapshot_screen(&screen, 80, 24, "gt_agent_detail_80x24");
}

#[test]
fn agent_detail_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::agent_detail::AgentDetail::default();
    snapshot_screen(&screen, 120, 40, "gt_agent_detail_120x40");
}

// ---------------------------------------------------------------------------
// Mail Inbox
// ---------------------------------------------------------------------------

#[test]
fn mail_inbox_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::mail_inbox::MailInbox::default();
    snapshot_screen(&screen, 80, 24, "gt_mail_inbox_80x24");
}

#[test]
fn mail_inbox_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::mail_inbox::MailInbox::default();
    snapshot_screen(&screen, 120, 40, "gt_mail_inbox_120x40");
}

// ---------------------------------------------------------------------------
// Command Palette
// ---------------------------------------------------------------------------

#[test]
fn command_palette_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen =
        ftui_demo_showcase::screens::command_palette::CommandPaletteScreen::default();
    snapshot_screen(&screen, 80, 24, "gt_command_palette_80x24");
}

#[test]
fn command_palette_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen =
        ftui_demo_showcase::screens::command_palette::CommandPaletteScreen::default();
    snapshot_screen(&screen, 120, 40, "gt_command_palette_120x40");
}

// ---------------------------------------------------------------------------
// Toast Events
// ---------------------------------------------------------------------------

#[test]
fn toast_events_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::toast_events::ToastEventsScreen::default();
    snapshot_screen(&screen, 80, 24, "gt_toast_events_80x24");
}

#[test]
fn toast_events_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::toast_events::ToastEventsScreen::default();
    snapshot_screen(&screen, 120, 40, "gt_toast_events_120x40");
}

// ---------------------------------------------------------------------------
// Beads Overview
// ---------------------------------------------------------------------------

#[test]
fn beads_overview_80x24() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::beads_overview::BeadsOverview::default();
    snapshot_screen(&screen, 80, 24, "gt_beads_overview_80x24");
}

#[test]
fn beads_overview_120x40() {
    let _guard = ScopedThemeLock::new(ThemeId::CyberpunkAurora);
    let screen = ftui_demo_showcase::screens::beads_overview::BeadsOverview::default();
    snapshot_screen(&screen, 120, 40, "gt_beads_overview_120x40");
}
