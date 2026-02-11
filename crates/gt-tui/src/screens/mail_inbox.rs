//! Mail Inbox screen — message list with reading pane.
//!
//! Fetches real mail data from `gt mail inbox --json` via MailPoller.

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

use crate::data;
use crate::msg::Msg;

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
    messages: Vec<data::MailMessage>,
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
            messages: Vec::new(),
            selected: 0,
            list_scroll: 0,
            body_scroll: 0,
            focus: Panel::MessageList,
            tick_count: 0,
            layout_list: Cell::new(Rect::default()),
            layout_reading: Cell::new(Rect::default()),
        }
    }

    /// Replace messages with fresh data from MailPoller.
    pub fn set_messages(&mut self, messages: Vec<data::MailMessage>) {
        let prev_id = self.messages.get(self.selected).map(|m| m.id.clone());
        self.messages = messages;
        // Try to preserve selection by ID
        if let Some(id) = prev_id {
            if let Some(idx) = self.messages.iter().position(|m| m.id == id) {
                self.selected = idx;
                return;
            }
        }
        // Clamp selection
        if !self.messages.is_empty() && self.selected >= self.messages.len() {
            self.selected = self.messages.len() - 1;
        }
    }

    fn unread_count(&self) -> usize {
        self.messages.iter().filter(|m| !m.read).count()
    }

    fn mark_selected_read(&mut self) -> Cmd<Msg> {
        let msg = match self.messages.get_mut(self.selected) {
            Some(m) if !m.read => m,
            _ => return Cmd::None,
        };
        msg.read = true;
        let id = msg.id.clone();
        // Fire async gt mail mark-read
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                let output = data::run_cli_command(&format!("gt mail mark-read {}", id));
                Msg::CommandOutput(format!("gt mail mark-read {}", id), output)
            }),
        )
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

    fn priority_icon(priority: &str) -> &'static str {
        match priority {
            "critical" => "!!",
            "high" => "!",
            "low" => " ",
            _ => " ",
        }
    }

    fn priority_label(priority: &str) -> &'static str {
        match priority {
            "critical" => "CRITICAL",
            "high" => "HIGH",
            "low" => "Low",
            _ => "Normal",
        }
    }

    // -- Rendering helpers --

    fn render_message_list(&self, frame: &mut Frame, area: Rect) {
        let inbox_title = format!("Inbox ({} unread / {} total)", self.unread_count(), self.messages.len());
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

        if self.messages.is_empty() {
            Paragraph::new("No mail messages. gt mail inbox is empty.")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
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

            let priority_icon = Self::priority_icon(&msg.priority);
            let from_display = truncate_to_width(&msg.from, 16);
            // Show just HH:MM from ISO timestamp
            let time_display = if msg.timestamp.len() >= 16 {
                &msg.timestamp[11..16]
            } else {
                &msg.timestamp
            };
            let line = format!(
                " {:<2} {:<16} {:<5} {}",
                priority_icon, from_display, time_display, msg.subject
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

        let from_line = format!("From: {}  To: {}", msg.from, msg.to);
        Paragraph::new(truncate_to_width(&from_line, header_rows[0].width))
            .style(
                Style::new()
                    .fg(theme::fg::PRIMARY)
                    .attrs(StyleFlags::BOLD),
            )
            .render(header_rows[0], frame);

        let meta_line = format!(
            "Priority: {} | {} | ID: {} | {}",
            Self::priority_label(&msg.priority),
            msg.timestamp,
            msg.id,
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
            " Message {}/{} | Unread: {} | Focus: {:?} | Enter=mark-read  Tab=switch  j/k=navigate",
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
                return self.mark_selected_read();
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
