//! Mail Inbox screen — message list with reading pane.

use std::cell::Cell;

use ftui_core::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::Cell as RenderCell;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::{Style, StyleFlags};
use ftui_text::{grapheme_width, graphemes};
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ftui_widgets::{StatefulWidget, Widget};

use crate::msg::Msg;

// ---------------------------------------------------------------------------
// Mail data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Priority {
    fn icon(self) -> &'static str {
        match self {
            Priority::Low => " ",
            Priority::Normal => " ",
            Priority::High => "!",
            Priority::Critical => "!!",
        }
    }
}

#[derive(Debug, Clone)]
struct MailMessage {
    from: String,
    subject: String,
    body: String,
    priority: Priority,
    timestamp: String,
    read: bool,
}

fn det_hash(seed: u64) -> u64 {
    let mut h = seed;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^= h >> 33;
    h
}

fn generate_messages() -> Vec<MailMessage> {
    let senders = [
        "mayor", "witness", "refinery", "polecat/amber",
        "polecat/jade", "polecat/onyx", "hq/dispatch", "hq/scheduler",
    ];
    let subjects = [
        "Work assignment ready",
        "Merge queue status update",
        "Build failure on main",
        "Context recovery needed",
        "New molecule dispatched",
        "Escalation: blocked task",
        "Performance metrics report",
        "Rig health check passed",
        "Dependency conflict detected",
        "Handoff notes attached",
        "Test coverage report",
        "Branch rebase required",
        "Pipeline stalled - action needed",
        "Sprint planning summary",
        "Code review requested",
    ];
    let bodies = [
        "Your work has been placed on the hook. Execute immediately per GUPP.\n\nMolecule: bd-wisp-tflm\nSteps remaining: 8\n\nRemember: Hook \u{2192} bd ready \u{2192} Execute \u{2192} gt done.",
        "The merge queue has processed 3 branches successfully.\n\n  \u{2713} polecat/amber/bd-123 \u{2192} merged\n  \u{2713} polecat/jade/bd-456 \u{2192} merged\n  \u{2713} polecat/onyx/bd-789 \u{2192} merged\n\nNo conflicts detected. Pipeline green.",
        "Build failed on commit e3c1baac.\n\nError: test_snapshot_determinism FAILED\n  Expected checksum: 0xABCD1234\n  Actual checksum:   0xDEADBEEF\n\nThis is a pre-existing failure on main. Filed as bd-xyz.",
        "Your session context is approaching limits.\n\nRecommendation: Run gt handoff to cycle to a fresh session.\nCurrent usage: 87% of context window.\n\nAll progress has been committed.",
        "New molecule bd-wisp-abc has been dispatched.\n\nFormula: mol-polecat-work (10 steps)\nAssigned to: obsidian\nPriority: P2\n\nBegin with: bd ready",
        "Task bd-429p is blocked.\n\nBlocker: Missing API endpoint for mail fetch.\nSeverity: HIGH\n\nWitness has been notified. Awaiting resolution.",
        "Performance metrics for the last 24 hours:\n\n  Render budget: 98.2% on target\n  Frame drops: 0\n  Memory usage: 42MB (stable)\n  GC pauses: none (Rust)\n\nAll systems nominal.",
        "Rig health check completed successfully.\n\n  Polecats: 3/3 healthy\n  Refinery: operational\n  Witness: monitoring\n  Beads DB: synced\n\nNo action required.",
        "Dependency conflict in Cargo.lock.\n\n  ftui-widgets 0.1.1 requires ftui-core >=0.1.0\n  ftui-extras 0.1.1 requires ftui-core >=0.1.1\n\nResolution: cargo update -p ftui-core",
        "Handoff from previous session:\n\nIssue: bd-429p (Mail Inbox panel)\nStatus: Implementation in progress\nBranch: polecat/obsidian/bd-429p\n\nNext: Continue with mail_inbox.rs creation.",
        "Test coverage summary:\n\n  ftui-core: 94.2%\n  ftui-widgets: 87.1%\n  ftui-extras: 76.8%\n  ftui-demo-showcase: 62.4%\n\nTarget: 80% across all crates.",
        "Branch polecat/obsidian/bd-429p needs rebase.\n\nMain has advanced 5 commits since branch creation.\nConflicts likely in: app.rs, mod.rs\n\nRun: git fetch origin && git rebase origin/main",
        "Pipeline stalled for 12 minutes.\n\nCause: polecat/jade session idle (Idle Polecat heresy)\nAction: Witness sending nudge.\n\nIf you are polecat/jade, run gt done NOW.",
        "Sprint planning for next cycle:\n\n  1. Focus management refactor (bd-280t)\n  2. Command palette (bd-s8py)\n  3. Toast notifications (bd-1smd)\n  4. Snapshot tests (bd-nmg1)\n\nPrioritize in order.",
        "Code review requested for PR #42.\n\nAuthor: polecat/amber\nBranch: polecat/amber/bd-s8py\nFiles changed: 4\nInsertions: +312\nDeletions: -8\n\nPlease review at your earliest convenience.",
    ];

    let mut messages = Vec::with_capacity(20);
    for i in 0..20 {
        let h = det_hash(i as u64 + 42);
        let sender_idx = (h % senders.len() as u64) as usize;
        let subject_idx = ((h >> 8) % subjects.len() as u64) as usize;
        let body_idx = ((h >> 16) % bodies.len() as u64) as usize;
        let priority = match (h >> 24) % 10 {
            0 => Priority::Critical,
            1..=2 => Priority::High,
            3..=5 => Priority::Normal,
            _ => Priority::Low,
        };
        let hour = ((h >> 32) % 24) as u8;
        let minute = ((h >> 40) % 60) as u8;
        let read = (h >> 48) % 3 == 0;

        messages.push(MailMessage {
            from: senders[sender_idx].to_string(),
            subject: subjects[subject_idx].to_string(),
            body: bodies[body_idx].to_string(),
            priority,
            timestamp: format!("{hour:02}:{minute:02}"),
            read,
        });
    }
    messages
}

// ---------------------------------------------------------------------------
// Focus panels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    MessageList,
    ReadingPane,
}

impl Panel {
    fn next(self) -> Self {
        match self {
            Panel::MessageList => Panel::ReadingPane,
            Panel::ReadingPane => Panel::MessageList,
        }
    }
}

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

pub struct MailInboxScreen {
    messages: Vec<MailMessage>,
    selected: usize,
    list_scroll: usize,
    body_scroll: usize,
    focus: Panel,
    tick_count: u64,
    layout_list: Cell<Rect>,
    layout_reading: Cell<Rect>,
}

impl MailInboxScreen {
    pub fn new() -> Self {
        Self {
            messages: generate_messages(),
            selected: 0,
            list_scroll: 0,
            body_scroll: 0,
            focus: Panel::MessageList,
            tick_count: 0,
            layout_list: Cell::new(Rect::default()),
            layout_reading: Cell::new(Rect::default()),
        }
    }

    fn unread_count(&self) -> usize {
        self.messages.iter().filter(|m| !m.read).count()
    }

    fn mark_selected_read(&mut self) {
        if let Some(msg) = self.messages.get_mut(self.selected) {
            msg.read = true;
        }
    }

    fn toggle_selected_read(&mut self) {
        if let Some(msg) = self.messages.get_mut(self.selected) {
            msg.read = !msg.read;
        }
    }

    fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.body_scroll = 0;
        }
    }

    fn select_down(&mut self) {
        if self.selected + 1 < self.messages.len() {
            self.selected += 1;
            self.body_scroll = 0;
        }
    }

    fn scroll_body_up(&mut self) {
        self.body_scroll = self.body_scroll.saturating_sub(1);
    }

    fn scroll_body_down(&mut self) {
        self.body_scroll = self.body_scroll.saturating_add(1);
    }

    fn focus_from_point(&mut self, x: u16, y: u16) {
        let list = self.layout_list.get();
        let reading = self.layout_reading.get();
        if !list.is_empty() && list.contains(x, y) {
            self.focus = Panel::MessageList;
        } else if !reading.is_empty() && reading.contains(x, y) {
            self.focus = Panel::ReadingPane;
        }
    }

    // -- Rendering helpers --

    fn render_message_list(&self, frame: &mut Frame, area: Rect) {
        let inbox_title = format!("Inbox ({} unread)", self.unread_count());
        let border_style = if self.focus == Panel::MessageList {
            Style::new().fg(theme::accent::PRIMARY)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(&inbox_title)
            .title_alignment(Alignment::Left)
            .style(border_style);
        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height == 0 || inner.width < 10 {
            return;
        }

        // Header row
        let header_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let header = format!(
            " {:<2} {:<16} {:<5} {}",
            "P", "From", "Time", "Subject"
        );
        Paragraph::new(truncate_to_width(&header, inner.width))
            .style(
                Style::new()
                    .fg(theme::fg::PRIMARY)
                    .attrs(StyleFlags::BOLD | StyleFlags::UNDERLINE),
            )
            .render(header_area, frame);

        let list_height = (inner.height as usize).saturating_sub(1);
        if list_height == 0 {
            return;
        }

        let mut scroll = self.list_scroll;
        if self.selected < scroll {
            scroll = self.selected;
        } else if self.selected >= scroll + list_height {
            scroll = self.selected.saturating_sub(list_height.saturating_sub(1));
        }

        let end = (scroll + list_height).min(self.messages.len());
        for (i, msg) in self.messages[scroll..end].iter().enumerate() {
            let row_y = inner.y + 1 + i as u16;
            let row_area = Rect::new(inner.x, row_y, inner.width, 1);
            let is_selected = scroll + i == self.selected;

            let priority_icon = msg.priority.icon();
            let from_display = truncate_to_width(&msg.from, 16);
            let line = format!(
                " {:<2} {:<16} {:<5} {}",
                priority_icon, from_display, msg.timestamp, msg.subject
            );
            let line = truncate_to_width(&line, inner.width);

            if is_selected {
                let bg = if self.focus == Panel::MessageList {
                    theme::alpha::HIGHLIGHT
                } else {
                    theme::alpha::SURFACE
                };
                frame.buffer.fill(
                    row_area,
                    RenderCell::default().with_bg(bg.into()),
                );
                let style = if msg.read {
                    Style::new().fg(theme::fg::PRIMARY).bg(bg)
                } else {
                    Style::new()
                        .fg(theme::fg::PRIMARY)
                        .bg(bg)
                        .attrs(StyleFlags::BOLD)
                };
                Paragraph::new(line).style(style).render(row_area, frame);
            } else if !msg.read {
                Paragraph::new(line)
                    .style(
                        Style::new()
                            .fg(theme::fg::PRIMARY)
                            .attrs(StyleFlags::BOLD),
                    )
                    .render(row_area, frame);
            } else {
                Paragraph::new(line)
                    .style(Style::new().fg(theme::fg::MUTED))
                    .render(row_area, frame);
            }
        }
    }

    fn render_reading_pane(&self, frame: &mut Frame, area: Rect) {
        let msg = self.messages.get(self.selected);
        let title = msg
            .map(|m| m.subject.as_str())
            .unwrap_or("No message selected");
        let title = truncate_to_width(title, area.width.saturating_sub(4));

        let border_style = if self.focus == Panel::ReadingPane {
            Style::new().fg(theme::accent::PRIMARY)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(&title)
            .title_alignment(Alignment::Left)
            .style(border_style);
        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height == 0 || inner.width < 10 {
            return;
        }

        let Some(msg) = msg else {
            Paragraph::new("No messages")
                .style(crate::theme::muted())
                .render(inner, frame);
            return;
        };

        let header_rows = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),
                Constraint::Fixed(1),
                Constraint::Fixed(1),
                Constraint::Min(1),
            ])
            .split(inner);

        let from_line = format!("From: {}", msg.from);
        Paragraph::new(truncate_to_width(&from_line, header_rows[0].width))
            .style(
                Style::new()
                    .fg(theme::fg::PRIMARY)
                    .attrs(StyleFlags::BOLD),
            )
            .render(header_rows[0], frame);

        let meta_line = format!(
            "Priority: {} | Time: {} | Status: {}",
            match msg.priority {
                Priority::Critical => "CRITICAL",
                Priority::High => "HIGH",
                Priority::Normal => "Normal",
                Priority::Low => "Low",
            },
            msg.timestamp,
            if msg.read { "Read" } else { "Unread" }
        );
        Paragraph::new(truncate_to_width(&meta_line, header_rows[1].width))
            .style(Style::new().fg(theme::fg::SECONDARY))
            .render(header_rows[1], frame);

        let sep = "\u{2500}".repeat(header_rows[2].width as usize);
        Paragraph::new(truncate_to_width(&sep, header_rows[2].width))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(header_rows[2], frame);

        let body_area = header_rows[3];
        if body_area.height == 0 {
            return;
        }

        let body_lines: Vec<&str> = msg.body.lines().collect();
        let visible = body_area.height as usize;
        let max_scroll = body_lines.len().saturating_sub(visible);
        let scroll = self.body_scroll.min(max_scroll);

        for (i, line) in body_lines.iter().skip(scroll).take(visible).enumerate() {
            let row_y = body_area.y + i as u16;
            let row_area = Rect::new(body_area.x, row_y, body_area.width, 1);
            Paragraph::new(truncate_to_width(line, body_area.width))
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(row_area, frame);
        }

        if body_lines.len() > visible {
            let sb_area = Rect::new(
                body_area.x + body_area.width.saturating_sub(1),
                body_area.y,
                1,
                body_area.height,
            );
            let mut sb_state = ScrollbarState::new(body_lines.len(), scroll, visible);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::new().fg(theme::accent::PRIMARY))
                .track_style(Style::new().fg(theme::bg::SURFACE));
            StatefulWidget::render(&scrollbar, sb_area, frame, &mut sb_state);
        }
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        frame.buffer.fill(
            area,
            RenderCell::default().with_bg(theme::alpha::SURFACE.into()),
        );

        let status = format!(
            " Message {}/{} | Unread: {} | Focus: {:?} | Enter=read  Tab=switch  r=toggle",
            if self.messages.is_empty() {
                0
            } else {
                self.selected + 1
            },
            self.messages.len(),
            self.unread_count(),
            self.focus,
        );
        Paragraph::new(truncate_to_width(&status, area.width))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(area, frame);
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Tab => {
                self.focus = self.focus.next();
            }
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                Panel::MessageList => self.select_up(),
                Panel::ReadingPane => self.scroll_body_up(),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                Panel::MessageList => self.select_down(),
                Panel::ReadingPane => self.scroll_body_down(),
            },
            KeyCode::Enter => {
                self.mark_selected_read();
            }
            KeyCode::Char('r') => {
                self.toggle_selected_read();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                self.list_scroll = 0;
                self.body_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.messages.is_empty() {
                    self.selected = self.messages.len() - 1;
                    self.body_scroll = 0;
                }
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn handle_mouse(&mut self, mouse: &MouseEvent) -> Cmd<Msg> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus_from_point(mouse.x, mouse.y);
                if self.focus == Panel::MessageList {
                    let list = self.layout_list.get();
                    if list.contains(mouse.x, mouse.y) {
                        let rel_y = mouse.y.saturating_sub(list.y + 2) as usize;
                        let clicked = self.list_scroll + rel_y;
                        if clicked < self.messages.len() {
                            self.selected = clicked;
                            self.body_scroll = 0;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => match self.focus {
                Panel::MessageList => self.select_up(),
                Panel::ReadingPane => self.scroll_body_up(),
            },
            MouseEventKind::ScrollDown => match self.focus {
                Panel::MessageList => self.select_down(),
                Panel::ReadingPane => self.scroll_body_down(),
            },
            _ => {}
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        if area.height < 5 || area.width < 20 {
            Paragraph::new("Terminal too small")
                .style(crate::theme::muted())
                .render(area, frame);
            return;
        }

        let v_chunks = Flex::vertical()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Min(4),
                Constraint::Fixed(1),
            ])
            .split(area);

        self.layout_list.set(v_chunks[0]);
        self.layout_reading.set(v_chunks[1]);

        self.render_message_list(frame, v_chunks[0]);
        self.render_reading_pane(frame, v_chunks[1]);
        self.render_status_bar(frame, v_chunks[2]);
    }
}

fn truncate_to_width(text: &str, max_width: u16) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut width = 0usize;
    let max = max_width as usize;
    for grapheme in graphemes(text) {
        let w = grapheme_width(grapheme);
        if width + w > max {
            break;
        }
        out.push_str(grapheme);
        width += w;
    }
    out
}
