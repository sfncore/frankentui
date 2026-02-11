#![forbid(unsafe_code)]

//! Beads Overview screen — Ready/blocked/in-progress dashboard.
//!
//! Three-section beads dashboard with Tables for ready, in-progress, and blocked
//! issues. Bottom sparkline shows close rate over time. Simulated static data
//! matching bd CLI output format.

use std::cell::RefCell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::{Style, TablePresetId, TableTheme};
use ftui_text::{Line, Span, Text};
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::sparkline::Sparkline;
use ftui_widgets::table::{Row, Table, TableState};
use ftui_widgets::{StatefulWidget, Widget};

use super::{HelpEntry, Screen};
use crate::theme;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct BeadItem {
    id: &'static str,
    title: &'static str,
    priority: u8,
    bead_type: &'static str,
    blockers: &'static str,
}

impl BeadItem {
    fn priority_label(&self) -> &'static str {
        match self.priority {
            0 => "P0",
            1 => "P1",
            2 => "P2",
            3 => "P3",
            4 => "P4",
            _ => "P?",
        }
    }

    fn priority_style(&self) -> Style {
        match self.priority {
            0 => Style::new().fg(theme::accent::ERROR).bold(),
            1 => Style::new().fg(theme::accent::WARNING).bold(),
            2 => Style::new().fg(theme::accent::INFO),
            3 => Style::new().fg(theme::fg::SECONDARY),
            _ => Style::new().fg(theme::fg::MUTED),
        }
    }
}

// ---------------------------------------------------------------------------
// Sample data
// ---------------------------------------------------------------------------

const READY_ITEMS: &[BeadItem] = &[
    BeadItem { id: "bd-nmg1", title: "Snapshot tests for all panels", priority: 2, bead_type: "task", blockers: "" },
    BeadItem { id: "bd-280t", title: "Focus management: FocusGraph", priority: 3, bead_type: "task", blockers: "" },
    BeadItem { id: "bd-4fhg", title: "Beads overview dashboard", priority: 3, bead_type: "task", blockers: "" },
    BeadItem { id: "bd-x7k2", title: "Agent log viewer integration", priority: 2, bead_type: "feature", blockers: "" },
    BeadItem { id: "bd-r3m9", title: "Keyboard shortcut reference", priority: 4, bead_type: "task", blockers: "" },
];

const IN_PROGRESS_ITEMS: &[BeadItem] = &[
    BeadItem { id: "bd-9f1f", title: "gt-tui v2: Rich widget upgrade epic", priority: 1, bead_type: "epic", blockers: "" },
    BeadItem { id: "bd-a2c4", title: "Mermaid renderer performance", priority: 2, bead_type: "task", blockers: "" },
    BeadItem { id: "bd-f8g1", title: "Theme hot-reload support", priority: 3, bead_type: "feature", blockers: "" },
];

const BLOCKED_ITEMS: &[BeadItem] = &[
    BeadItem { id: "bd-j5k7", title: "Deploy to production", priority: 1, bead_type: "task", blockers: "bd-9f1f" },
    BeadItem { id: "bd-m2n4", title: "Integration test suite", priority: 2, bead_type: "task", blockers: "bd-nmg1" },
    BeadItem { id: "bd-p8q1", title: "User docs update", priority: 3, bead_type: "task", blockers: "bd-4fhg" },
    BeadItem { id: "bd-t5u7", title: "Release changelog", priority: 3, bead_type: "task", blockers: "bd-9f1f, bd-j5k7" },
];

/// Simulated close-rate sparkline data (issues closed per day, last 14 days).
const CLOSE_RATE: &[f64] = &[2.0, 1.0, 3.0, 5.0, 4.0, 7.0, 6.0, 3.0, 8.0, 5.0, 4.0, 6.0, 9.0, 7.0];

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

/// Active section for keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Ready,
    InProgress,
    Blocked,
}

impl Section {
    fn next(self) -> Self {
        match self {
            Self::Ready => Self::InProgress,
            Self::InProgress => Self::Blocked,
            Self::Blocked => Self::Ready,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Ready => Self::Blocked,
            Self::InProgress => Self::Ready,
            Self::Blocked => Self::InProgress,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::InProgress => "In Progress",
            Self::Blocked => "Blocked",
        }
    }

    fn items(self) -> &'static [BeadItem] {
        match self {
            Self::Ready => READY_ITEMS,
            Self::InProgress => IN_PROGRESS_ITEMS,
            Self::Blocked => BLOCKED_ITEMS,
        }
    }
}

/// Beads overview screen state.
pub struct BeadsOverview {
    active_section: Section,
    ready_state: RefCell<TableState>,
    in_progress_state: RefCell<TableState>,
    blocked_state: RefCell<TableState>,
    tick_count: u64,
}

impl Default for BeadsOverview {
    fn default() -> Self {
        Self {
            active_section: Section::Ready,
            ready_state: RefCell::new(TableState::default()),
            in_progress_state: RefCell::new(TableState::default()),
            blocked_state: RefCell::new(TableState::default()),
            tick_count: 0,
        }
    }
}

impl BeadsOverview {
    fn active_table_state(&self) -> &RefCell<TableState> {
        match self.active_section {
            Section::Ready => &self.ready_state,
            Section::InProgress => &self.in_progress_state,
            Section::Blocked => &self.blocked_state,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) {
        let items = self.active_section.items();
        let row_count = items.len();

        match key.code {
            KeyCode::Tab => {
                self.active_section = self.active_section.next();
            }
            KeyCode::BackTab => {
                self.active_section = self.active_section.prev();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let mut state = self.active_table_state().borrow_mut();
                if let Some(sel) = state.selected {
                    if sel > 0 {
                        state.select(Some(sel - 1));
                    }
                } else if row_count > 0 {
                    state.select(Some(row_count - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let mut state = self.active_table_state().borrow_mut();
                if let Some(sel) = state.selected {
                    if sel + 1 < row_count {
                        state.select(Some(sel + 1));
                    }
                } else if row_count > 0 {
                    state.select(Some(0));
                }
            }
            KeyCode::Home => {
                if row_count > 0 {
                    self.active_table_state().borrow_mut().select(Some(0));
                }
            }
            KeyCode::End => {
                if row_count > 0 {
                    self.active_table_state()
                        .borrow_mut()
                        .select(Some(row_count - 1));
                }
            }
            _ => {}
        }
    }

    fn build_rows(items: &[BeadItem], show_blockers: bool) -> Vec<Row> {
        items
            .iter()
            .map(|item| {
                let mut cells = vec![
                    Text::from(Line::from_spans([Span::styled(
                        item.id,
                        Style::new().fg(theme::accent::INFO),
                    )])),
                    Text::raw(item.title),
                    Text::from(Line::from_spans([Span::styled(
                        item.priority_label(),
                        item.priority_style(),
                    )])),
                    Text::raw(item.bead_type),
                ];
                if show_blockers {
                    cells.push(Text::from(Line::from_spans([Span::styled(
                        item.blockers,
                        Style::new().fg(theme::accent::WARNING),
                    )])));
                }
                Row::new(cells)
            })
            .collect()
    }

    fn build_table<'a>(
        items: &[BeadItem],
        title: &'a str,
        active: bool,
        show_blockers: bool,
        tick_count: u64,
    ) -> Table<'a> {
        let mut widths = vec![
            Constraint::Fixed(10), // ID
            Constraint::Fill,      // Title
            Constraint::Fixed(4),  // Priority
            Constraint::Fixed(8),  // Type
        ];
        let mut header_cells = vec![
            Text::raw("ID"),
            Text::raw("Title"),
            Text::raw("Pri"),
            Text::raw("Type"),
        ];
        if show_blockers {
            widths.push(Constraint::Fixed(20)); // Blockers
            header_cells.push(Text::raw("Blocked By"));
        }

        let header = Row::new(header_cells).style(Style::new().bold());
        let rows = Self::build_rows(items, show_blockers);
        let theme_preset = TableTheme::preset(TablePresetId::Slate);
        let phase = theme::table_theme_phase(tick_count);

        let border_style = if active {
            Style::new()
                .fg(theme::accent::PRIMARY)
                .bg(theme::bg::DEEP)
        } else {
            theme::content_border()
        };

        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(if active {
                        BorderType::Double
                    } else {
                        BorderType::Rounded
                    })
                    .style(border_style),
            )
            .highlight_style(Style::new().bg(theme::bg::SURFACE).bold())
            .theme(theme_preset)
            .theme_phase(phase)
    }

    fn render_stats(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Stats ")
            .title_alignment(Alignment::Left)
            .style(theme::content_border());

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let total_open = READY_ITEMS.len() + IN_PROGRESS_ITEMS.len() + BLOCKED_ITEMS.len();
        let closed = 47; // simulated

        let lines = vec![
            Line::from_spans([
                Span::styled("Open: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{total_open}"),
                    Style::new().fg(theme::accent::INFO).bold(),
                ),
                Span::styled("  Closed: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{closed}"),
                    Style::new().fg(theme::accent::SUCCESS).bold(),
                ),
                Span::styled("  Blocked: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", BLOCKED_ITEMS.len()),
                    Style::new().fg(theme::accent::WARNING).bold(),
                ),
            ]),
            Line::from_spans([
                Span::styled("Ready: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", READY_ITEMS.len()),
                    Style::new().fg(theme::accent::SUCCESS),
                ),
                Span::styled("  In Progress: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", IN_PROGRESS_ITEMS.len()),
                    Style::new().fg(theme::accent::INFO),
                ),
                Span::styled(
                    format!("  Active: {}", self.active_section.label()),
                    Style::new().fg(theme::fg::MUTED),
                ),
            ]),
        ];

        // Render stats text
        let text_height = lines.len() as u16;
        let text_area = Rect::new(inner.x, inner.y, inner.width, text_height.min(inner.height));
        Paragraph::new(Text::from_lines(lines)).render(text_area, frame);

        // Render sparkline for close rate if there's room
        let spark_y = inner.y + text_height + 1;
        if spark_y < inner.bottom() {
            let spark_height = inner.bottom().saturating_sub(spark_y);
            let spark_label_area = Rect::new(inner.x, spark_y, inner.width, 1);
            if spark_height > 1 {
                Paragraph::new("Close rate (14d):")
                    .style(Style::new().fg(theme::fg::MUTED))
                    .render(spark_label_area, frame);

                let spark_area = Rect::new(
                    inner.x,
                    spark_y + 1,
                    inner.width,
                    spark_height.saturating_sub(1),
                );
                Sparkline::new(CLOSE_RATE)
                    .style(Style::new().fg(theme::accent::SUCCESS))
                    .render(spark_area, frame);
            } else {
                // Just sparkline, no label
                let spark_area = Rect::new(inner.x, spark_y, inner.width, spark_height);
                Sparkline::new(CLOSE_RATE)
                    .style(Style::new().fg(theme::accent::SUCCESS))
                    .render(spark_area, frame);
            }
        }
    }
}

impl Screen for BeadsOverview {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                self.handle_key(key);
            }
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        // Layout: three table sections stacked vertically + stats bar at bottom
        let main_chunks = Flex::vertical()
            .constraints([Constraint::Fill, Constraint::Fixed(7)])
            .split(area);

        let table_area = main_chunks[0];
        let stats_area = main_chunks[1];

        // Split tables area into three sections
        let sections = Flex::vertical()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(30.0),
                Constraint::Percentage(30.0),
            ])
            .split(table_area);

        // Ready section
        {
            let table = Self::build_table(
                READY_ITEMS,
                " Ready ",
                self.active_section == Section::Ready,
                false,
                self.tick_count,
            );
            let mut state = self.ready_state.borrow_mut();
            StatefulWidget::render(&table, sections[0], frame, &mut state);
        }

        // In Progress section
        {
            let table = Self::build_table(
                IN_PROGRESS_ITEMS,
                " In Progress ",
                self.active_section == Section::InProgress,
                false,
                self.tick_count,
            );
            let mut state = self.in_progress_state.borrow_mut();
            StatefulWidget::render(&table, sections[1], frame, &mut state);
        }

        // Blocked section
        {
            let table = Self::build_table(
                BLOCKED_ITEMS,
                " Blocked ",
                self.active_section == Section::Blocked,
                true,
                self.tick_count,
            );
            let mut state = self.blocked_state.borrow_mut();
            StatefulWidget::render(&table, sections[2], frame, &mut state);
        }

        // Stats panel at bottom
        self.render_stats(frame, stats_area);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "Tab",
                action: "Next section",
            },
            HelpEntry {
                key: "Shift+Tab",
                action: "Prev section",
            },
            HelpEntry {
                key: "j/Down",
                action: "Next item",
            },
            HelpEntry {
                key: "k/Up",
                action: "Prev item",
            },
            HelpEntry {
                key: "Home/End",
                action: "First/Last item",
            },
        ]
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    fn title(&self) -> &'static str {
        "Beads Overview"
    }

    fn tab_label(&self) -> &'static str {
        "Beads"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn default_state() {
        let screen = BeadsOverview::default();
        assert_eq!(screen.active_section, Section::Ready);
        assert_eq!(screen.tick_count, 0);
    }

    #[test]
    fn tab_cycles_sections() {
        let mut screen = BeadsOverview::default();
        assert_eq!(screen.active_section, Section::Ready);

        let tab = Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        screen.update(&tab);
        assert_eq!(screen.active_section, Section::InProgress);
        screen.update(&tab);
        assert_eq!(screen.active_section, Section::Blocked);
        screen.update(&tab);
        assert_eq!(screen.active_section, Section::Ready);
    }

    #[test]
    fn jk_navigates_rows() {
        let mut screen = BeadsOverview::default();
        let down = Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        screen.update(&down);
        assert_eq!(screen.ready_state.borrow().selected, Some(0));
        screen.update(&down);
        assert_eq!(screen.ready_state.borrow().selected, Some(1));
    }

    #[test]
    fn render_does_not_panic() {
        let screen = BeadsOverview::default();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        screen.view(&mut frame, Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn render_zero_area_does_not_panic() {
        let screen = BeadsOverview::default();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        screen.view(&mut frame, Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn keybindings_returns_entries() {
        let screen = BeadsOverview::default();
        let bindings = screen.keybindings();
        assert_eq!(bindings.len(), 5);
        assert_eq!(bindings[0].key, "Tab");
    }

    #[test]
    fn title_and_label() {
        let screen = BeadsOverview::default();
        assert_eq!(screen.title(), "Beads Overview");
        assert_eq!(screen.tab_label(), "Beads");
    }
}
