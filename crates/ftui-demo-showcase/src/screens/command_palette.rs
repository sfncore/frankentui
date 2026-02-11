#![forbid(unsafe_code)]

//! Command Palette screen — Ctrl+P action launcher for Gas Town TUI.
//!
//! Demonstrates `CommandPalette` widget with fuzzy search via `BayesianScorer`.
//! Actions: some execute immediately (refresh, list), others open a status
//! message showing args would be needed (nudge, mail).

use std::cell::Cell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::text::{Line, Span};
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::command_palette::{ActionItem, CommandPalette, PaletteAction};
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::theme;

/// Build the Gas Town action items for the command palette.
fn gas_town_actions() -> Vec<ActionItem> {
    vec![
        ActionItem::new("gt-sling", "Sling Work")
            .with_description("Assign work to a polecat (gt sling)")
            .with_tags(&["work", "assign", "dispatch"])
            .with_category("Work"),
        ActionItem::new("gt-nudge", "Nudge Agent")
            .with_description("Send a nudge to a stuck agent (gt nudge)")
            .with_tags(&["agent", "nudge", "unstick"])
            .with_category("Agent"),
        ActionItem::new("gt-mail-send", "Send Mail")
            .with_description("Send mail to an agent or role (gt mail send)")
            .with_tags(&["mail", "send", "message"])
            .with_category("Mail"),
        ActionItem::new("gt-convoy-create", "Create Convoy")
            .with_description("Create a new convoy (gt convoy create)")
            .with_tags(&["convoy", "create", "group"])
            .with_category("Convoy"),
        ActionItem::new("gt-status", "Refresh Status")
            .with_description("Refresh rig status (gt status)")
            .with_tags(&["refresh", "status", "update"])
            .with_category("System"),
        ActionItem::new("gt-polecat-list", "Polecat List")
            .with_description("List all polecats in the rig")
            .with_tags(&["polecat", "list", "workers"])
            .with_category("System"),
        ActionItem::new("gt-rig-list", "Rig List")
            .with_description("List all rigs in Gas Town")
            .with_tags(&["rig", "list", "town"])
            .with_category("System"),
        ActionItem::new("bd-ready", "Beads Ready")
            .with_description("Show ready work with no blockers (bd ready)")
            .with_tags(&["beads", "ready", "work"])
            .with_category("Beads"),
        ActionItem::new("bd-stats", "Beads Stats")
            .with_description("Show beads statistics and counts (bd stats)")
            .with_tags(&["beads", "stats", "metrics"])
            .with_category("Beads"),
    ]
}

/// Whether an action executes immediately or needs arguments.
fn action_needs_args(id: &str) -> bool {
    matches!(id, "gt-nudge" | "gt-mail-send" | "gt-sling" | "gt-convoy-create")
}

/// Simulated result message for an executed action.
fn action_result_message(id: &str) -> &'static str {
    match id {
        "gt-status" => "Refreshing rig status...",
        "gt-polecat-list" => "Listing polecats: obsidian (active), jade (idle)",
        "gt-rig-list" => "Rigs: frankentui (3 polecats), longeye (1 polecat)",
        "bd-ready" => "Ready: 4 issues with no blockers",
        "bd-stats" => "Beads: 12 open, 8 closed, 3 in-progress",
        "gt-sling" => "Needs args: target polecat and issue ID",
        "gt-nudge" => "Needs args: agent name and message",
        "gt-mail-send" => "Needs args: recipient and message body",
        "gt-convoy-create" => "Needs args: convoy name and members",
        _ => "Unknown action",
    }
}

/// Command palette action launcher screen.
pub struct CommandPaletteScreen {
    /// The command palette widget.
    palette: CommandPalette,
    /// Last executed action result (displayed in status panel).
    last_result: Option<(String, String)>,
    /// Total actions executed this session.
    exec_count: u32,
    /// Cached layout areas for mouse hit-testing.
    layout_palette: Cell<Rect>,
    layout_status: Cell<Rect>,
}

impl Default for CommandPaletteScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteScreen {
    pub fn new() -> Self {
        let mut palette = CommandPalette::new().with_max_visible(9);
        palette.replace_actions(gas_town_actions());
        palette.open();

        Self {
            palette,
            last_result: None,
            exec_count: 0,
            layout_palette: Cell::new(Rect::default()),
            layout_status: Cell::new(Rect::default()),
        }
    }

    /// Handle action execution when user selects from palette.
    fn execute_action(&mut self, action_id: &str) {
        let msg = action_result_message(action_id);
        let label = if action_needs_args(action_id) {
            format!("[needs args] {msg}")
        } else {
            format!("[executed] {msg}")
        };
        self.last_result = Some((action_id.to_string(), label));
        self.exec_count += 1;
        // Re-open palette after execution for continued use
        self.palette.open();
    }

    /// Handle mouse events on the palette area.
    fn handle_mouse(&mut self, event: &Event) {
        if let Event::Mouse(mouse) = event {
            let palette_area = self.layout_palette.get();
            if !palette_area.contains(mouse.x, mouse.y) {
                return;
            }
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        modifiers: ftui_core::event::Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    if let Some(PaletteAction::Execute(id)) = self.palette.handle_event(&synth) {
                        self.execute_action(&id);
                    }
                }
                MouseEventKind::ScrollUp => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Up,
                        modifiers: ftui_core::event::Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    for _ in 0..3 {
                        let _ = self.palette.handle_event(&synth);
                    }
                }
                MouseEventKind::ScrollDown => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Down,
                        modifiers: ftui_core::event::Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    for _ in 0..3 {
                        let _ = self.palette.handle_event(&synth);
                    }
                }
                _ => {}
            }
        }
    }

    /// Render the instructions & status panel on the left.
    fn render_status_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Command Palette")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let accent = Style::new().fg(theme::screen_accent::ADVANCED).bold();
        let muted = theme::muted();
        let info = Style::new().fg(theme::accent::INFO);

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from_spans([
            Span::styled("Ctrl+P", accent),
            Span::styled(" or ", muted),
            Span::styled(":", accent),
            Span::styled(" to open palette", muted),
        ]));
        lines.push(Line::raw(""));
        lines.push(Line::from_spans([
            Span::styled("Actions:", Style::new().fg(theme::fg::PRIMARY).bold()),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  ", muted),
            Span::styled("Immediate:", info),
            Span::styled(" refresh, list, stats", muted),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  ", muted),
            Span::styled("With args:", info),
            Span::styled(" sling, nudge, mail", muted),
        ]));
        lines.push(Line::raw(""));
        lines.push(Line::from_spans([
            Span::styled("Navigation:", Style::new().fg(theme::fg::PRIMARY).bold()),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  Up/Down ", accent),
            Span::styled("move selection", muted),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  Enter   ", accent),
            Span::styled("execute action", muted),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  Esc     ", accent),
            Span::styled("dismiss palette", muted),
        ]));
        lines.push(Line::from_spans([
            Span::styled("  Type    ", accent),
            Span::styled("fuzzy filter", muted),
        ]));

        lines.push(Line::raw(""));
        lines.push(Line::from_spans([
            Span::styled(
                format!("Executions: {}", self.exec_count),
                Style::new().fg(theme::accent::SUCCESS),
            ),
        ]));

        if let Some((ref id, ref msg)) = self.last_result {
            lines.push(Line::raw(""));
            lines.push(Line::from_spans([
                Span::styled("Last: ", muted),
                Span::styled(id.as_str(), accent),
            ]));
            lines.push(Line::from_spans([
                Span::styled(msg.as_str(), info),
            ]));
        }

        for (i, line) in lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let row = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
            Paragraph::new(line.clone()).render(row, frame);
        }
    }
}

impl Screen for CommandPaletteScreen {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        // Handle mouse events
        if matches!(event, Event::Mouse(_)) {
            self.handle_mouse(event);
            return Cmd::None;
        }

        // Handle colon to open palette
        if let Event::Key(KeyEvent {
            code: KeyCode::Char(':'),
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            self.palette.open();
            return Cmd::None;
        }

        // Forward to palette widget
        if let Some(action) = self.palette.handle_event(event) {
            match action {
                PaletteAction::Execute(id) => {
                    self.execute_action(&id);
                }
                PaletteAction::Dismiss => {
                    // Re-open immediately in demo mode so palette stays visible
                    self.palette.open();
                }
            }
        }

        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let chunks = Flex::horizontal()
            .constraints([Constraint::Percentage(35.0), Constraint::Min(1)])
            .split(area);

        self.layout_status.set(chunks[0]);
        self.layout_palette.set(chunks[1]);

        self.render_status_panel(frame, chunks[0]);

        // Render palette widget in the right area
        self.palette.render(chunks[1], frame);
    }

    fn consumes_text_input(&self) -> bool {
        true
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "Ctrl+P",
                action: "Open command palette",
            },
            HelpEntry {
                key: ":",
                action: "Open command palette",
            },
            HelpEntry {
                key: "Up/Down",
                action: "Navigate results",
            },
            HelpEntry {
                key: "Enter",
                action: "Execute selected action",
            },
            HelpEntry {
                key: "Esc",
                action: "Dismiss palette",
            },
            HelpEntry {
                key: "Type",
                action: "Fuzzy search filter",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Command Palette"
    }

    fn tab_label(&self) -> &'static str {
        "CmdPalette"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn default_creates_screen() {
        let screen = CommandPaletteScreen::new();
        assert_eq!(screen.exec_count, 0);
        assert!(screen.last_result.is_none());
    }

    #[test]
    fn gas_town_actions_has_nine_items() {
        let actions = gas_town_actions();
        assert_eq!(actions.len(), 9);
    }

    #[test]
    fn execute_action_increments_count() {
        let mut screen = CommandPaletteScreen::new();
        screen.execute_action("gt-status");
        assert_eq!(screen.exec_count, 1);
        assert!(screen.last_result.is_some());
    }

    #[test]
    fn action_needs_args_correct() {
        assert!(action_needs_args("gt-nudge"));
        assert!(action_needs_args("gt-mail-send"));
        assert!(!action_needs_args("gt-status"));
        assert!(!action_needs_args("bd-ready"));
    }

    #[test]
    fn render_does_not_panic() {
        let screen = CommandPaletteScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        let area = Rect::new(0, 0, 80, 24);
        screen.view(&mut frame, area);
    }

    #[test]
    fn render_zero_area_does_not_panic() {
        let screen = CommandPaletteScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        let area = Rect::new(0, 0, 0, 0);
        screen.view(&mut frame, area);
    }

    #[test]
    fn render_tiny_area_does_not_panic() {
        let screen = CommandPaletteScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(10, 5, &mut pool);
        let area = Rect::new(0, 0, 10, 5);
        screen.view(&mut frame, area);
    }

    #[test]
    fn colon_key_opens_palette() {
        use super::Screen;
        let mut screen = CommandPaletteScreen::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char(':'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        screen.update(&event);
        // Should not panic; palette should stay open
    }

    #[test]
    fn title_and_label() {
        use super::Screen;
        let screen = CommandPaletteScreen::new();
        assert_eq!(screen.title(), "Command Palette");
        assert_eq!(screen.tab_label(), "CmdPalette");
    }

    #[test]
    fn consumes_text_input_true() {
        use super::Screen;
        let screen = CommandPaletteScreen::new();
        assert!(screen.consumes_text_input());
    }

    #[test]
    fn keybindings_returns_entries() {
        use super::Screen;
        let screen = CommandPaletteScreen::new();
        let bindings = screen.keybindings();
        assert_eq!(bindings.len(), 6);
        assert_eq!(bindings[0].key, "Ctrl+P");
    }

    #[test]
    fn render_after_execution_does_not_panic() {
        let mut screen = CommandPaletteScreen::new();
        screen.execute_action("gt-status");
        screen.execute_action("bd-ready");

        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        let area = Rect::new(0, 0, 80, 24);
        screen.view(&mut frame, area);
    }
}
