//! Workflows screen — browse and run GT formula templates.
//!
//! Left panel: formula list table (name, type, steps, vars).
//! Right panel: detail view with steps, variables, and description.

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

use ftui_extras::theme::ColorToken;

use crate::data::{self, FormulaDetail, FormulaItem};
use crate::msg::Msg;

// ---------------------------------------------------------------------------
// Focus panels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Detail,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Focus::List => Focus::Detail,
            Focus::Detail => Focus::List,
        }
    }
}

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

pub struct WorkflowsScreen {
    selected: usize,
    list_scroll: usize,
    detail: Option<FormulaDetail>,
    detail_scroll: usize,
    focus: Focus,
    tick_count: u64,
    layout_list: Cell<Rect>,
    layout_detail: Cell<Rect>,
    /// Name of formula whose detail is currently loaded (or loading).
    loaded_name: String,
}

impl WorkflowsScreen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            list_scroll: 0,
            detail: None,
            detail_scroll: 0,
            focus: Focus::List,
            tick_count: 0,
            layout_list: Cell::new(Rect::default()),
            layout_detail: Cell::new(Rect::default()),
            loaded_name: String::new(),
        }
    }

    pub fn set_detail(&mut self, detail: FormulaDetail) {
        self.loaded_name = detail.formula.clone();
        self.detail = Some(detail);
        self.detail_scroll = 0;
    }

    fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.detail_scroll = 0;
        }
    }

    fn select_down(&mut self, len: usize) {
        if self.selected + 1 < len {
            self.selected += 1;
            self.detail_scroll = 0;
        }
    }

    fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    fn scroll_detail_down(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_add(1);
    }

    fn focus_from_point(&mut self, x: u16, y: u16) {
        let list = self.layout_list.get();
        let detail = self.layout_detail.get();
        if !list.is_empty() && list.contains(x, y) {
            self.focus = Focus::List;
        } else if !detail.is_empty() && detail.contains(x, y) {
            self.focus = Focus::Detail;
        }
    }

    /// Load detail for the currently selected formula (async).
    fn load_selected_detail(&mut self, formulas: &[FormulaItem]) -> Cmd<Msg> {
        let Some(item) = formulas.get(self.selected) else {
            return Cmd::None;
        };
        if item.name == self.loaded_name && self.detail.is_some() {
            return Cmd::None; // already loaded
        }
        let name = item.name.clone();
        self.loaded_name = name.clone();
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                match data::fetch_formula_detail(&name) {
                    Some(d) => Msg::FormulaDetailLoaded(d),
                    None => Msg::Noop,
                }
            }),
        )
    }

    /// Type badge color.
    fn type_color(formula_type: &str) -> ColorToken {
        match formula_type {
            "workflow" => theme::accent::INFO,
            "convoy" => theme::accent::SUCCESS,
            "expansion" => theme::accent::WARNING,
            "aspect" => theme::accent::ACCENT_5,
            _ => theme::fg::MUTED,
        }
    }

    // -- Rendering --

    fn render_list(&self, frame: &mut Frame, area: Rect, formulas: &[FormulaItem]) {
        let title = format!("Formulas ({})", formulas.len());
        let border_style = if self.focus == Focus::List {
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

        if formulas.is_empty() {
            Paragraph::new("No formulas found. Run `gt formula list`.")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
            return;
        }

        // Header row
        let header_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let header = format!(
            " {:<20} {:<10} {:>5} {:>4}",
            "Name", "Type", "Steps", "Vars"
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

        // Scroll to keep selected visible
        let mut scroll = self.list_scroll;
        if self.selected < scroll {
            scroll = self.selected;
        } else if self.selected >= scroll + list_height {
            scroll = self.selected.saturating_sub(list_height.saturating_sub(1));
        }

        let end = (scroll + list_height).min(formulas.len());
        for (i, item) in formulas[scroll..end].iter().enumerate() {
            let row_y = inner.y + 1 + i as u16;
            let row_area = Rect::new(inner.x, row_y, inner.width, 1);
            let is_selected = scroll + i == self.selected;

            let line = format!(
                " {:<20} {:<10} {:>5} {:>4}",
                truncate_to_width(&item.name, 20),
                truncate_to_width(&item.formula_type, 10),
                item.steps,
                item.vars,
            );
            let line = truncate_to_width(&line, inner.width);

            if is_selected {
                let bg = if self.focus == Focus::List {
                    theme::alpha::HIGHLIGHT
                } else {
                    theme::alpha::SURFACE
                };
                frame.buffer.fill(
                    row_area,
                    RenderCell::default().with_bg(bg.into()),
                );
                Paragraph::new(line)
                    .style(Style::new().fg(theme::fg::PRIMARY).bg(bg))
                    .render(row_area, frame);
            } else {
                let type_fg = Self::type_color(&item.formula_type);
                // Render the whole line with type color for visual distinction
                Paragraph::new(line)
                    .style(Style::new().fg(type_fg))
                    .render(row_area, frame);
            }
        }

        // Scrollbar
        if formulas.len() > list_height {
            let sb_area = Rect::new(
                inner.x + inner.width.saturating_sub(1),
                inner.y + 1,
                1,
                inner.height.saturating_sub(1),
            );
            let mut sb_state = ScrollbarState::new(formulas.len(), scroll, list_height);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::new().fg(theme::accent::PRIMARY))
                .track_style(Style::new().fg(theme::bg::SURFACE));
            StatefulWidget::render(&scrollbar, sb_area, frame, &mut sb_state);
        }
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let title = match &self.detail {
            Some(d) => format!("{} [{}]", d.formula, d.formula_type),
            None => "Select a formula".to_string(),
        };
        let border_style = if self.focus == Focus::Detail {
            Style::new().fg(theme::accent::PRIMARY)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };
        let title_trunc = truncate_to_width(&title, area.width.saturating_sub(4));
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(&title_trunc)
            .title_alignment(Alignment::Left)
            .style(border_style);
        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height == 0 || inner.width < 10 {
            return;
        }

        let Some(detail) = &self.detail else {
            Paragraph::new("Press Enter on a formula to load details")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
            return;
        };

        // Build content lines
        let mut lines: Vec<(String, ColorToken)> = Vec::new();

        // Description
        if !detail.description.is_empty() {
            for line in detail.description.lines() {
                lines.push((line.to_string(), theme::fg::SECONDARY));
            }
            lines.push((String::new(), theme::fg::MUTED));
        }

        // Variables section
        if let Some(obj) = detail.vars.as_object()
            && !obj.is_empty()
        {
            lines.push((
                "\u{2500} Variables \u{2500}".to_string(),
                theme::accent::PRIMARY,
            ));
            for (name, val) in obj {
                let desc = val
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let required = val
                    .get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let req_badge = if required { " (required)" } else { "" };
                lines.push((
                    format!("  \u{2022} {}{}: {}", name, req_badge, desc),
                    theme::fg::PRIMARY,
                ));
            }
            lines.push((String::new(), theme::fg::MUTED));
        }

        // Steps section
        if !detail.steps.is_empty() {
            lines.push((
                format!("\u{2500} Steps ({}) \u{2500}", detail.steps.len()),
                theme::accent::PRIMARY,
            ));
            for (i, step) in detail.steps.iter().enumerate() {
                let needs_str = if step.needs.is_empty() {
                    String::new()
                } else {
                    format!("  \u{2190} needs: {}", step.needs.join(", "))
                };
                lines.push((
                    format!("  {}. [{}] {}{}", i + 1, step.id, step.title, needs_str),
                    theme::fg::PRIMARY,
                ));
            }
            lines.push((String::new(), theme::fg::MUTED));
        }

        // Source
        if !detail.source.is_empty() {
            lines.push((
                format!("Source: {}", detail.source),
                theme::fg::MUTED,
            ));
        }

        // Render with scroll
        let visible = inner.height as usize;
        let max_scroll = lines.len().saturating_sub(visible);
        let scroll = self.detail_scroll.min(max_scroll);

        for (i, (text, color)) in lines.iter().skip(scroll).take(visible).enumerate() {
            let row_y = inner.y + i as u16;
            let row_area = Rect::new(inner.x, row_y, inner.width, 1);
            Paragraph::new(truncate_to_width(text, inner.width))
                .style(Style::new().fg(*color))
                .render(row_area, frame);
        }

        // Scrollbar
        if lines.len() > visible {
            let sb_area = Rect::new(
                inner.x + inner.width.saturating_sub(1),
                inner.y,
                1,
                inner.height,
            );
            let mut sb_state = ScrollbarState::new(lines.len(), scroll, visible);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::new().fg(theme::accent::PRIMARY))
                .track_style(Style::new().fg(theme::bg::SURFACE));
            StatefulWidget::render(&scrollbar, sb_area, frame, &mut sb_state);
        }
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect, formulas: &[FormulaItem]) {
        if area.is_empty() {
            return;
        }
        frame.buffer.fill(
            area,
            RenderCell::default().with_bg(theme::alpha::SURFACE.into()),
        );

        let sel = if formulas.is_empty() {
            0
        } else {
            self.selected + 1
        };
        let status = format!(
            " Formula {}/{} | Focus: {:?} | Enter=detail  Tab=switch  r=run  j/k=navigate",
            sel,
            formulas.len(),
            self.focus,
        );
        Paragraph::new(truncate_to_width(&status, area.width))
            .style(Style::new().fg(theme::fg::MUTED))
            .render(area, frame);
    }

    // -- Public interface --

    pub fn handle_key(&mut self, key: &KeyEvent, formulas: &[FormulaItem]) -> Cmd<Msg> {
        match key.code {
            KeyCode::Tab => {
                self.focus = self.focus.next();
            }
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                Focus::List => {
                    self.select_up();
                    return self.load_selected_detail(formulas);
                }
                Focus::Detail => self.scroll_detail_up(),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                Focus::List => {
                    self.select_down(formulas.len());
                    return self.load_selected_detail(formulas);
                }
                Focus::Detail => self.scroll_detail_down(),
            },
            KeyCode::Enter => {
                return self.load_selected_detail(formulas);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                self.list_scroll = 0;
                self.detail_scroll = 0;
                return self.load_selected_detail(formulas);
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !formulas.is_empty() {
                    self.selected = formulas.len() - 1;
                    self.detail_scroll = 0;
                    return self.load_selected_detail(formulas);
                }
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn handle_mouse(&mut self, mouse: &MouseEvent, formulas: &[FormulaItem]) -> Cmd<Msg> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus_from_point(mouse.x, mouse.y);
                if self.focus == Focus::List {
                    let list = self.layout_list.get();
                    if list.contains(mouse.x, mouse.y) {
                        let rel_y = mouse.y.saturating_sub(list.y + 2) as usize;
                        let clicked = self.list_scroll + rel_y;
                        if clicked < formulas.len() {
                            self.selected = clicked;
                            self.detail_scroll = 0;
                            return self.load_selected_detail(formulas);
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => match self.focus {
                Focus::List => {
                    self.select_up();
                    return self.load_selected_detail(formulas);
                }
                Focus::Detail => self.scroll_detail_up(),
            },
            MouseEventKind::ScrollDown => match self.focus {
                Focus::List => {
                    self.select_down(formulas.len());
                    return self.load_selected_detail(formulas);
                }
                Focus::Detail => self.scroll_detail_down(),
            },
            _ => {}
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect, formulas: &[FormulaItem]) {
        if area.height < 5 || area.width < 20 {
            Paragraph::new("Terminal too small")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(area, frame);
            return;
        }

        let v_chunks = Flex::vertical()
            .constraints([Constraint::Min(6), Constraint::Fixed(1)])
            .split(area);

        let h_chunks = Flex::horizontal()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Min(20),
            ])
            .split(v_chunks[0]);

        self.layout_list.set(h_chunks[0]);
        self.layout_detail.set(h_chunks[1]);

        self.render_list(frame, h_chunks[0], formulas);
        self.render_detail(frame, h_chunks[1]);
        self.render_status_bar(frame, v_chunks[1], formulas);
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
