#![forbid(unsafe_code)]

//! Convoy Panel screen — Table widget with progress columns.
//!
//! Displays convoy data in a Table with columns: ID (fixed 10), Title (fill),
//! Progress (fixed 12), Status (fixed 10, with icon), Age (fixed 8).
//! Uses TableTheme for alternating row styling and HitId for row click.

use std::cell::RefCell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::{Frame, HitId};
use ftui_runtime::Cmd;
use ftui_style::{Style, TablePresetId, TableTheme};
use ftui_text::{Line, Span, Text};
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::mouse::MouseResult;
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::progress::ProgressBar;
use ftui_widgets::table::{Row, Table, TableState};
use ftui_widgets::{StatefulWidget, Widget};

use super::{HelpEntry, Screen};
use crate::theme;

/// HitId for the convoy table.
const CONVOY_TABLE_HIT: HitId = HitId::new(9001);

/// A single convoy entry for display.
#[derive(Debug, Clone)]
struct ConvoyEntry {
    id: &'static str,
    title: &'static str,
    done: u32,
    total: u32,
    status: ConvoyStatus,
    age: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConvoyStatus {
    Running,
    Complete,
    Blocked,
    Queued,
    Failed,
}

impl ConvoyStatus {
    fn icon(self) -> &'static str {
        match self {
            Self::Running => "\u{25b6}",
            Self::Complete => "\u{2713}",
            Self::Blocked => "\u{2298}",
            Self::Queued => "\u{25cc}",
            Self::Failed => "\u{2717}",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Complete => "Done",
            Self::Blocked => "Blocked",
            Self::Queued => "Queued",
            Self::Failed => "Failed",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Running => Style::new().fg(theme::accent::INFO),
            Self::Complete => Style::new().fg(theme::accent::SUCCESS),
            Self::Blocked => Style::new().fg(theme::accent::WARNING),
            Self::Queued => Style::new().fg(theme::fg::MUTED),
            Self::Failed => Style::new().fg(theme::accent::ERROR),
        }
    }
}

const SAMPLE_ENTRIES: &[ConvoyEntry] = &[
    ConvoyEntry { id: "cv-a1b2", title: "Widget upgrade epic", done: 7, total: 10, status: ConvoyStatus::Running, age: "2h 15m" },
    ConvoyEntry { id: "cv-c3d4", title: "Test coverage sweep", done: 12, total: 12, status: ConvoyStatus::Complete, age: "5h 30m" },
    ConvoyEntry { id: "cv-e5f6", title: "Refinery merge backlog", done: 3, total: 8, status: ConvoyStatus::Blocked, age: "1h 45m" },
    ConvoyEntry { id: "cv-g7h8", title: "Documentation sprint", done: 0, total: 5, status: ConvoyStatus::Queued, age: "10m" },
    ConvoyEntry { id: "cv-i9j0", title: "CI pipeline fix", done: 1, total: 3, status: ConvoyStatus::Failed, age: "45m" },
    ConvoyEntry { id: "cv-k1l2", title: "Style theme overhaul", done: 4, total: 6, status: ConvoyStatus::Running, age: "3h 20m" },
    ConvoyEntry { id: "cv-m3n4", title: "Accessibility audit", done: 2, total: 9, status: ConvoyStatus::Running, age: "55m" },
    ConvoyEntry { id: "cv-o5p6", title: "Performance benchmarks", done: 6, total: 6, status: ConvoyStatus::Complete, age: "8h 10m" },
    ConvoyEntry { id: "cv-q7r8", title: "Mermaid diagram renderer", done: 5, total: 7, status: ConvoyStatus::Running, age: "1h 30m" },
    ConvoyEntry { id: "cv-s9t0", title: "Agent handoff protocol", done: 0, total: 4, status: ConvoyStatus::Queued, age: "5m" },
];

/// Convoy Panel screen state.
pub struct ConvoyPanel {
    table_state: RefCell<TableState>,
    entries: Vec<ConvoyEntry>,
    tick_count: u64,
    selected_detail: Option<usize>,
}

impl Default for ConvoyPanel {
    fn default() -> Self {
        Self {
            table_state: RefCell::new(TableState::default()),
            entries: SAMPLE_ENTRIES.to_vec(),
            tick_count: 0,
            selected_detail: None,
        }
    }
}

impl ConvoyPanel {
    fn progress_ratio(entry: &ConvoyEntry) -> f64 {
        if entry.total == 0 {
            0.0
        } else {
            entry.done as f64 / entry.total as f64
        }
    }

    fn build_rows(&self) -> Vec<Row> {
        self.entries
            .iter()
            .map(|entry| {
                let progress_text = format!("{}/{}", entry.done, entry.total);
                let status_text = format!("{} {}", entry.status.icon(), entry.status.label());
                Row::new([
                    Text::raw(entry.id),
                    Text::raw(entry.title),
                    Text::raw(progress_text),
                    Text::from(Line::from_spans([
                        Span::styled(status_text, entry.status.style()),
                    ])),
                    Text::raw(entry.age),
                ])
            })
            .collect()
    }

    fn build_table(&self) -> Table<'_> {
        let widths = [
            Constraint::Fixed(10),  // ID
            Constraint::Fill,       // Title
            Constraint::Fixed(12),  // Progress
            Constraint::Fixed(10),  // Status
            Constraint::Fixed(8),   // Age
        ];

        let header = Row::new([
            Text::raw("ID"),
            Text::raw("Title"),
            Text::raw("Progress"),
            Text::raw("Status"),
            Text::raw("Age"),
        ]).style(Style::new().bold());

        let rows = self.build_rows();
        let theme = TableTheme::preset(TablePresetId::Slate);
        let phase = theme::table_theme_phase(self.tick_count);

        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(" Convoys ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(theme::content_border()),
            )
            .highlight_style(Style::new().bg(theme::bg::SURFACE).bold())
            .theme(theme)
            .theme_phase(phase)
            .hit_id(CONVOY_TABLE_HIT)
    }

    fn render_progress_overlay(&self, frame: &mut Frame, area: Rect) {
        let block_inner = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .inner(area);

        if block_inner.height < 2 {
            return;
        }

        // Header row is 1 line, data rows follow
        let data_start_y = block_inner.y + 1;
        let spacing: u16 = 1;

        // Compute progress column x offset:
        // ID(10) + sp + Title(fill) + sp + [Progress here](12) + sp + Status(10) + sp + Age(8)
        let fixed_total: u16 = 10 + 12 + 10 + 8 + spacing * 4;
        let title_width = block_inner.width.saturating_sub(fixed_total);
        let progress_x = block_inner.x + 10 + spacing + title_width + spacing;
        let progress_width: u16 = 12;

        let state = self.table_state.borrow();
        let offset = state.offset;
        let visible_rows = (block_inner.height.saturating_sub(1)) as usize;

        for i in 0..visible_rows {
            let entry_idx = offset + i;
            if entry_idx >= self.entries.len() {
                break;
            }
            let entry = &self.entries[entry_idx];
            let row_y = data_start_y + i as u16;
            if row_y >= area.bottom() {
                break;
            }

            let w = progress_width.min(area.right().saturating_sub(progress_x));
            let bar_area = Rect::new(progress_x, row_y, w, 1);

            let ratio = Self::progress_ratio(entry);
            let gauge_fg = if ratio >= 1.0 {
                theme::accent::SUCCESS
            } else if ratio >= 0.5 {
                theme::accent::INFO
            } else {
                theme::accent::WARNING
            };

            ProgressBar::new()
                .ratio(ratio)
                .gauge_style(Style::new().fg(gauge_fg))
                .style(Style::new().fg(theme::fg::MUTED))
                .render(bar_area, frame);
        }
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let idx = match self.selected_detail {
            Some(i) if i < self.entries.len() => i,
            _ => {
                let block = Block::default()
                    .title(" Detail ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(theme::content_border());
                Paragraph::new(Text::raw("Select a convoy to view details"))
                    .block(block)
                    .style(theme::muted())
                    .render(area, frame);
                return;
            }
        };

        let entry = &self.entries[idx];
        let ratio = Self::progress_ratio(entry);
        let pct = (ratio * 100.0) as u32;

        let lines = [
            Line::from_spans([
                Span::styled("ID:       ", Style::new().bold()),
                Span::raw(entry.id),
            ]),
            Line::from_spans([
                Span::styled("Title:    ", Style::new().bold()),
                Span::raw(entry.title),
            ]),
            Line::from_spans([
                Span::styled("Status:   ", Style::new().bold()),
                Span::styled(
                    format!("{} {}", entry.status.icon(), entry.status.label()),
                    entry.status.style(),
                ),
            ]),
            Line::from_spans([
                Span::styled("Progress: ", Style::new().bold()),
                Span::raw(format!("{}/{} ({}%)", entry.done, entry.total, pct)),
            ]),
            Line::from_spans([
                Span::styled("Age:      ", Style::new().bold()),
                Span::raw(entry.age),
            ]),
        ];

        let detail_title = format!(" {} ", entry.title);
        let block = Block::default()
            .title(detail_title.as_str())
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(theme::content_border());

        Paragraph::new(Text::from_lines(lines))
            .block(block)
            .render(area, frame);
    }

    fn handle_key(&mut self, key: &KeyEvent) {
        let row_count = self.entries.len();
        let mut state = self.table_state.borrow_mut();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(sel) = state.selected {
                    if sel > 0 {
                        state.select(Some(sel - 1));
                    }
                } else if row_count > 0 {
                    state.select(Some(row_count - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(sel) = state.selected {
                    if sel + 1 < row_count {
                        state.select(Some(sel + 1));
                    }
                } else if row_count > 0 {
                    state.select(Some(0));
                }
            }
            KeyCode::Enter => {
                let sel = state.selected;
                drop(state);
                self.selected_detail = sel;
            }
            KeyCode::Escape => {
                state.select(None);
                drop(state);
                self.selected_detail = None;
            }
            KeyCode::Home => {
                if row_count > 0 {
                    state.select(Some(0));
                }
            }
            KeyCode::End => {
                if row_count > 0 {
                    state.select(Some(row_count - 1));
                }
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent) {
        let row_count = self.entries.len();
        let result = self.table_state.borrow_mut().handle_mouse(
            event,
            None,
            CONVOY_TABLE_HIT,
            row_count,
        );
        if let MouseResult::Activated(idx) = result {
            self.selected_detail = Some(idx);
        }
    }
}

impl Screen for ConvoyPanel {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key);
            }
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse);
            }
            _ => {}
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let has_detail = self.selected_detail.is_some();

        let chunks = if has_detail {
            Flex::vertical()
                .constraints([Constraint::Fill, Constraint::Fixed(8)])
                .split(area)
        } else {
            vec![area]
        };

        let table_area = chunks[0];

        // Render table with stateful selection
        let table = self.build_table();
        let mut state = self.table_state.borrow_mut();
        StatefulWidget::render(&table, table_area, frame, &mut state);
        drop(state);

        // Overlay progress bars on the progress column
        self.render_progress_overlay(frame, table_area);

        // Render detail panel if a convoy is selected for detail view
        if has_detail && chunks.len() > 1 {
            self.render_detail(frame, chunks[1]);
        }
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry { key: "j/Down", action: "Next convoy" },
            HelpEntry { key: "k/Up", action: "Previous convoy" },
            HelpEntry { key: "Enter", action: "Show detail" },
            HelpEntry { key: "Esc", action: "Clear selection" },
        ]
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    fn title(&self) -> &'static str {
        "Convoy Panel"
    }

    fn tab_label(&self) -> &'static str {
        "Convoys"
    }
}
