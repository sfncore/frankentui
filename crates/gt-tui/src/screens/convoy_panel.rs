//! Convoy Panel screen — Table widget with progress columns.
//! Uses real ConvoyItem data from `gt convoy list --json`.

use std::cell::RefCell;

use ftui_core::event::{KeyCode, KeyEvent, MouseEvent};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
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

use crate::data::ConvoyItem;
use crate::msg::Msg;

const CONVOY_TABLE_HIT: HitId = HitId::new(9001);

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

fn status_icon(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "running" | "active" => "\u{25b6}",
        "complete" | "completed" | "done" => "\u{2713}",
        "blocked" => "\u{2298}",
        "queued" | "pending" => "\u{25cc}",
        "failed" | "error" => "\u{2717}",
        _ => "\u{25cf}",
    }
}

fn status_style(status: &str) -> Style {
    match status.to_lowercase().as_str() {
        "running" | "active" => Style::new().fg(theme::accent::INFO),
        "complete" | "completed" | "done" => Style::new().fg(theme::accent::SUCCESS),
        "blocked" => Style::new().fg(theme::accent::WARNING),
        "queued" | "pending" => Style::new().fg(theme::fg::MUTED),
        "failed" | "error" => Style::new().fg(theme::accent::ERROR),
        _ => Style::new().fg(theme::fg::SECONDARY),
    }
}

fn progress_ratio(item: &ConvoyItem) -> f64 {
    if item.total == 0 {
        0.0
    } else {
        item.done as f64 / item.total as f64
    }
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct ConvoyPanelScreen {
    table_state: RefCell<TableState>,
    tick_count: u64,
    selected_detail: Option<usize>,
}

impl ConvoyPanelScreen {
    pub fn new() -> Self {
        Self {
            table_state: RefCell::new(TableState::default()),
            tick_count: 0,
            selected_detail: None,
        }
    }

    fn build_rows(convoys: &[ConvoyItem]) -> Vec<Row> {
        convoys
            .iter()
            .map(|item| {
                let progress_text = format!("{}/{}", item.done, item.total);
                let status_text =
                    format!("{} {}", status_icon(&item.status), &item.status);
                Row::new([
                    Text::raw(&item.id),
                    Text::raw(&item.title),
                    Text::raw(progress_text),
                    Text::from(Line::from_spans([Span::styled(
                        status_text,
                        status_style(&item.status),
                    )])),
                    Text::raw(&item.created_at),
                ])
            })
            .collect()
    }

    fn build_table<'a>(
        &self,
        convoys: &[ConvoyItem],
        title: &'a str,
    ) -> Table<'a> {
        let widths = [
            Constraint::Fixed(12),
            Constraint::Fill,
            Constraint::Fixed(12),
            Constraint::Fixed(12),
            Constraint::Fixed(12),
        ];

        let header = Row::new([
            Text::raw("ID"),
            Text::raw("Title"),
            Text::raw("Progress"),
            Text::raw("Status"),
            Text::raw("Created"),
        ])
        .style(Style::new().bold());

        let rows = Self::build_rows(convoys);
        let table_theme = TableTheme::preset(TablePresetId::Slate);
        let phase = crate::theme::table_theme_phase(self.tick_count);

        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(crate::theme::content_border()),
            )
            .highlight_style(Style::new().bg(theme::bg::SURFACE).bold())
            .theme(table_theme)
            .theme_phase(phase)
            .hit_id(CONVOY_TABLE_HIT)
    }

    fn render_progress_overlay(
        &self,
        frame: &mut Frame,
        area: Rect,
        convoys: &[ConvoyItem],
    ) {
        let block_inner = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .inner(area);

        if block_inner.height < 2 {
            return;
        }

        let data_start_y = block_inner.y + 1;
        let spacing: u16 = 1;
        let fixed_total: u16 = 12 + 12 + 12 + 12 + spacing * 4;
        let title_width = block_inner.width.saturating_sub(fixed_total);
        let progress_x = block_inner.x + 12 + spacing + title_width + spacing;
        let progress_width: u16 = 12;

        let state = self.table_state.borrow();
        let offset = state.offset;
        let visible_rows = (block_inner.height.saturating_sub(1)) as usize;

        for i in 0..visible_rows {
            let entry_idx = offset + i;
            if entry_idx >= convoys.len() {
                break;
            }
            let item = &convoys[entry_idx];
            let row_y = data_start_y + i as u16;
            if row_y >= area.bottom() {
                break;
            }

            let w = progress_width.min(area.right().saturating_sub(progress_x));
            let bar_area = Rect::new(progress_x, row_y, w, 1);

            let ratio = progress_ratio(item);
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

    fn render_detail(&self, frame: &mut Frame, area: Rect, convoys: &[ConvoyItem]) {
        let idx = match self.selected_detail {
            Some(i) if i < convoys.len() => i,
            _ => {
                let block = Block::default()
                    .title(" Detail ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(crate::theme::content_border());
                Paragraph::new(Text::raw("Select a convoy to view details"))
                    .block(block)
                    .style(crate::theme::muted())
                    .render(area, frame);
                return;
            }
        };

        let item = &convoys[idx];
        let ratio = progress_ratio(item);
        let pct = (ratio * 100.0) as u32;
        let landed_str = if item.landed { "Yes" } else { "No" };

        let lines = [
            Line::from_spans([
                Span::styled("ID:       ", Style::new().bold()),
                Span::raw(&item.id),
            ]),
            Line::from_spans([
                Span::styled("Title:    ", Style::new().bold()),
                Span::raw(&item.title),
            ]),
            Line::from_spans([
                Span::styled("Status:   ", Style::new().bold()),
                Span::styled(
                    format!("{} {}", status_icon(&item.status), &item.status),
                    status_style(&item.status),
                ),
            ]),
            Line::from_spans([
                Span::styled("Progress: ", Style::new().bold()),
                Span::raw(format!("{}/{} ({}%)", item.done, item.total, pct)),
            ]),
            Line::from_spans([
                Span::styled("Created:  ", Style::new().bold()),
                Span::raw(&item.created_at),
            ]),
            Line::from_spans([
                Span::styled("Landed:   ", Style::new().bold()),
                Span::raw(landed_str),
            ]),
        ];

        let detail_title = format!(" {} ", item.title);
        let block = Block::default()
            .title(detail_title.as_str())
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(crate::theme::content_border());

        Paragraph::new(Text::from_lines(lines))
            .block(block)
            .render(area, frame);
    }

    pub fn handle_key(
        &mut self,
        key: &KeyEvent,
        convoys: &[ConvoyItem],
    ) -> Cmd<Msg> {
        let row_count = convoys.len();
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
                return Cmd::None;
            }
            KeyCode::Escape => {
                state.select(None);
                drop(state);
                self.selected_detail = None;
                return Cmd::None;
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
        Cmd::None
    }

    pub fn handle_mouse(
        &mut self,
        mouse: &MouseEvent,
        convoys: &[ConvoyItem],
    ) -> Cmd<Msg> {
        let row_count = convoys.len();
        let result = self.table_state.borrow_mut().handle_mouse(
            mouse,
            None,
            CONVOY_TABLE_HIT,
            row_count,
        );
        if let MouseResult::Activated(idx) = result {
            self.selected_detail = Some(idx);
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect, convoys: &[ConvoyItem]) {
        let has_detail = self.selected_detail.is_some();

        let chunks = if has_detail {
            Flex::vertical()
                .constraints([Constraint::Fill, Constraint::Fixed(9)])
                .split(area)
        } else {
            vec![area]
        };

        let table_area = chunks[0];
        let title = format!(" Convoys ({}) ", convoys.len());
        let table = self.build_table(convoys, &title);
        let mut state = self.table_state.borrow_mut();
        StatefulWidget::render(&table, table_area, frame, &mut state);
        drop(state);

        self.render_progress_overlay(frame, table_area, convoys);

        if has_detail && chunks.len() > 1 {
            self.render_detail(frame, chunks[1], convoys);
        }
    }
}
