//! Beads Overview screen — Ready/blocked/in-progress dashboard with live data.

use std::cell::RefCell;

use ftui_core::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
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

use crate::data::{BeadItem, BeadsSnapshot};
use crate::msg::Msg;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn priority_label(p: u8) -> &'static str {
    match p {
        0 => "P0",
        1 => "P1",
        2 => "P2",
        3 => "P3",
        4 => "P4",
        _ => "P?",
    }
}

fn priority_style(p: u8) -> Style {
    match p {
        0 => Style::new().fg(theme::accent::ERROR).bold(),
        1 => Style::new().fg(theme::accent::WARNING).bold(),
        2 => Style::new().fg(theme::accent::INFO),
        3 => Style::new().fg(theme::fg::SECONDARY),
        _ => Style::new().fg(theme::fg::MUTED),
    }
}

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

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

    fn items_from<'a>(self, beads: &'a BeadsSnapshot) -> &'a [BeadItem] {
        match self {
            Self::Ready => &beads.ready,
            Self::InProgress => &beads.in_progress,
            Self::Blocked => &beads.blocked,
        }
    }
}

pub struct BeadsOverviewScreen {
    active_section: Section,
    ready_state: RefCell<TableState>,
    in_progress_state: RefCell<TableState>,
    blocked_state: RefCell<TableState>,
    tick_count: u64,
    section_areas: RefCell<[Rect; 3]>,
}

impl BeadsOverviewScreen {
    pub fn new() -> Self {
        Self {
            active_section: Section::Ready,
            ready_state: RefCell::new(TableState::default()),
            in_progress_state: RefCell::new(TableState::default()),
            blocked_state: RefCell::new(TableState::default()),
            tick_count: 0,
            section_areas: RefCell::new([Rect::default(); 3]),
        }
    }

    fn active_table_state(&self) -> &RefCell<TableState> {
        match self.active_section {
            Section::Ready => &self.ready_state,
            Section::InProgress => &self.in_progress_state,
            Section::Blocked => &self.blocked_state,
        }
    }

    fn build_rows(items: &[BeadItem], show_blockers: bool) -> Vec<Row> {
        items
            .iter()
            .map(|item| {
                let assignee_display = if item.assignee.is_empty() {
                    "-"
                } else {
                    &item.assignee
                };
                let assignee_color = if item.assignee.is_empty() {
                    theme::fg::DISABLED
                } else {
                    theme::accent::ACCENT_5
                };

                let mut cells = vec![
                    Text::from(Line::from_spans([Span::styled(
                        item.id.clone(),
                        Style::new().fg(theme::accent::INFO),
                    )])),
                    Text::raw(&item.title),
                    Text::from(Line::from_spans([Span::styled(
                        priority_label(item.priority),
                        priority_style(item.priority),
                    )])),
                    Text::raw(&item.issue_type),
                    Text::from(Line::from_spans([Span::styled(
                        assignee_display,
                        Style::new().fg(assignee_color),
                    )])),
                ];
                if show_blockers {
                    let blockers = item.blocked_by.join(", ");
                    cells.push(Text::from(Line::from_spans([Span::styled(
                        blockers,
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
            Constraint::Fixed(12),
            Constraint::Fill,
            Constraint::Fixed(4),
            Constraint::Fixed(10),
            Constraint::Fixed(16),
        ];
        let mut header_cells = vec![
            Text::raw("ID"),
            Text::raw("Title"),
            Text::raw("Pri"),
            Text::raw("Type"),
            Text::raw("Assignee"),
        ];
        if show_blockers {
            widths.push(Constraint::Fixed(24));
            header_cells.push(Text::raw("Blocked By"));
        }

        let header = Row::new(header_cells).style(Style::new().bold());
        let rows = Self::build_rows(items, show_blockers);
        let theme_preset = TableTheme::preset(TablePresetId::Slate);
        let phase = crate::theme::table_theme_phase(tick_count);

        let border_style = if active {
            Style::new()
                .fg(theme::accent::PRIMARY)
                .bg(theme::bg::DEEP)
        } else {
            crate::theme::content_border()
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

    fn render_stats(&self, frame: &mut Frame, area: Rect, beads: &BeadsSnapshot) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Stats ")
            .title_alignment(Alignment::Left)
            .style(crate::theme::content_border());

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let open = beads.ready.len() + beads.in_progress.len() + beads.blocked.len();
        let total = beads.total_count as usize;
        let closed = total.saturating_sub(open);

        let lines = vec![
            Line::from_spans([
                Span::styled("Total: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{total}"),
                    Style::new().fg(theme::fg::PRIMARY).bold(),
                ),
                Span::styled("  Open: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{open}"),
                    Style::new().fg(theme::accent::INFO).bold(),
                ),
                Span::styled("  Closed: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{closed}"),
                    Style::new().fg(theme::accent::SUCCESS).bold(),
                ),
                Span::styled("  Blocked: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", beads.blocked.len()),
                    Style::new().fg(theme::accent::WARNING).bold(),
                ),
            ]),
            Line::from_spans([
                Span::styled("Ready: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", beads.ready.len()),
                    Style::new().fg(theme::accent::SUCCESS),
                ),
                Span::styled("  In Progress: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{}", beads.in_progress.len()),
                    Style::new().fg(theme::accent::INFO),
                ),
                Span::styled(
                    format!("  Active: {}", self.active_section.label()),
                    Style::new().fg(theme::fg::MUTED),
                ),
            ]),
            Line::from_spans([
                Span::styled("j/k", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled(" navigate  ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("Tab", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("/", Style::new().fg(theme::fg::MUTED)),
                Span::styled("S-Tab", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled(" switch section  ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("Home", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("/", Style::new().fg(theme::fg::MUTED)),
                Span::styled("End", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled(" jump", Style::new().fg(theme::fg::MUTED)),
            ]),
        ];

        let text_height = lines.len() as u16;
        let text_area = Rect::new(inner.x, inner.y, inner.width, text_height.min(inner.height));
        Paragraph::new(Text::from_lines(lines)).render(text_area, frame);

        // Sparkline showing distribution: ready / in-progress / blocked
        let spark_y = inner.y + text_height + 1;
        if spark_y < inner.bottom() {
            let spark_height = inner.bottom().saturating_sub(spark_y);
            let spark_label_area = Rect::new(inner.x, spark_y, inner.width, 1);
            let data: Vec<f64> = vec![
                beads.ready.len() as f64,
                beads.in_progress.len() as f64,
                beads.blocked.len() as f64,
            ];
            if spark_height > 1 {
                Paragraph::new("Distribution (ready/progress/blocked):")
                    .style(Style::new().fg(theme::fg::MUTED))
                    .render(spark_label_area, frame);

                let spark_area = Rect::new(
                    inner.x,
                    spark_y + 1,
                    inner.width,
                    spark_height.saturating_sub(1),
                );
                Sparkline::new(&data)
                    .style(Style::new().fg(theme::accent::SUCCESS))
                    .render(spark_area, frame);
            } else {
                let spark_area = Rect::new(inner.x, spark_y, inner.width, spark_height);
                Sparkline::new(&data)
                    .style(Style::new().fg(theme::accent::SUCCESS))
                    .render(spark_area, frame);
            }
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent, beads: &BeadsSnapshot) -> Cmd<Msg> {
        let row_count = self.active_section.items_from(beads).len();

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
        Cmd::None
    }

    pub fn handle_mouse(&mut self, mouse: &MouseEvent, beads: &BeadsSnapshot) -> Cmd<Msg> {
        let areas = *self.section_areas.borrow();

        let hovered_section = if areas[0].contains(mouse.x, mouse.y) {
            Some(Section::Ready)
        } else if areas[1].contains(mouse.x, mouse.y) {
            Some(Section::InProgress)
        } else if areas[2].contains(mouse.x, mouse.y) {
            Some(Section::Blocked)
        } else {
            None
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(section) = hovered_section {
                    self.active_section = section;
                    let area = match section {
                        Section::Ready => &areas[0],
                        Section::InProgress => &areas[1],
                        Section::Blocked => &areas[2],
                    };
                    let row = (mouse.y - area.y).saturating_sub(2) as usize;
                    let items = section.items_from(beads);
                    if row < items.len() {
                        self.active_table_state().borrow_mut().select(Some(row));
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if hovered_section.is_some() {
                    let mut state = self.active_table_state().borrow_mut();
                    if let Some(sel) = state.selected {
                        if sel > 0 {
                            state.select(Some(sel - 1));
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if hovered_section.is_some() {
                    let row_count = self.active_section.items_from(beads).len();
                    let mut state = self.active_table_state().borrow_mut();
                    if let Some(sel) = state.selected {
                        if sel + 1 < row_count {
                            state.select(Some(sel + 1));
                        }
                    } else if row_count > 0 {
                        state.select(Some(0));
                    }
                }
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect, beads: &BeadsSnapshot) {
        if area.is_empty() {
            return;
        }

        let main_chunks = Flex::vertical()
            .constraints([Constraint::Fill, Constraint::Fixed(7)])
            .split(area);

        let table_area = main_chunks[0];
        let stats_area = main_chunks[1];

        let sections = Flex::vertical()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(30.0),
                Constraint::Percentage(30.0),
            ])
            .split(table_area);

        *self.section_areas.borrow_mut() = [sections[0], sections[1], sections[2]];

        let ready_title = format!(" Ready ({}) ", beads.ready.len());
        {
            let table = Self::build_table(
                &beads.ready,
                &ready_title,
                self.active_section == Section::Ready,
                false,
                self.tick_count,
            );
            let mut state = self.ready_state.borrow_mut();
            StatefulWidget::render(&table, sections[0], frame, &mut state);
        }

        let progress_title = format!(" In Progress ({}) ", beads.in_progress.len());
        {
            let table = Self::build_table(
                &beads.in_progress,
                &progress_title,
                self.active_section == Section::InProgress,
                false,
                self.tick_count,
            );
            let mut state = self.in_progress_state.borrow_mut();
            StatefulWidget::render(&table, sections[1], frame, &mut state);
        }

        let blocked_title = format!(" Blocked ({}) ", beads.blocked.len());
        {
            let table = Self::build_table(
                &beads.blocked,
                &blocked_title,
                self.active_section == Section::Blocked,
                true,
                self.tick_count,
            );
            let mut state = self.blocked_state.borrow_mut();
            StatefulWidget::render(&table, sections[2], frame, &mut state);
        }

        self.render_stats(frame, stats_area, beads);
    }
}
