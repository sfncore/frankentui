#![forbid(unsafe_code)]

//! Mind-blowing dashboard screen.
//!
//! Showcases EVERY major FrankenTUI capability simultaneously:
//! - Animated gradient title
//! - Live plasma visual effect (Braille canvas)
//! - Real-time sparkline charts
//! - Syntax-highlighted code preview
//! - GFM markdown preview
//! - System stats (FPS, theme, size)
//! - Keyboard shortcuts
//!
//! Dynamically reflowable from 40x10 to 200x50+.

use std::collections::VecDeque;
use std::time::Instant;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::canvas::{Canvas, Mode, Painter};
use ftui_extras::charts::Sparkline;
use ftui_extras::markdown::{MarkdownRenderer, MarkdownTheme};
use ftui_extras::syntax::SyntaxHighlighter;
use ftui_extras::text_effects::{ColorGradient, StyledText, TextEffect};
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::PackedRgba;
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::text::Text;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::data::SimulatedData;
use crate::theme;

/// Dashboard state.
pub struct Dashboard {
    // Animation
    tick_count: u64,
    time: f64,

    // Data sources
    simulated_data: SimulatedData,

    // FPS tracking
    frame_times: VecDeque<u64>,
    last_frame: Option<Instant>,
    fps: f64,

    // Syntax highlighter (cached)
    highlighter: SyntaxHighlighter,

    // Markdown renderer (cached)
    md_renderer: MarkdownRenderer,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Dashboard {
    pub fn new() -> Self {
        let mut simulated_data = SimulatedData::default();
        // Pre-populate some history
        for t in 0..30 {
            simulated_data.tick(t);
        }

        Self {
            tick_count: 30,
            time: 0.0,
            simulated_data,
            frame_times: VecDeque::with_capacity(60),
            last_frame: None,
            fps: 0.0,
            highlighter: SyntaxHighlighter::new(),
            md_renderer: MarkdownRenderer::new(MarkdownTheme::default()),
        }
    }

    /// Update FPS calculation.
    fn update_fps(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            let elapsed_us = now.duration_since(last).as_micros() as u64;
            self.frame_times.push_back(elapsed_us);
            if self.frame_times.len() > 30 {
                self.frame_times.pop_front();
            }
            if !self.frame_times.is_empty() {
                let avg_us: u64 =
                    self.frame_times.iter().sum::<u64>() / self.frame_times.len() as u64;
                if avg_us > 0 {
                    self.fps = 1_000_000.0 / avg_us as f64;
                }
            }
        }
        self.last_frame = Some(now);
    }

    // =========================================================================
    // Panel Renderers
    // =========================================================================

    /// Render animated gradient header.
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.height < 1 {
            return;
        }

        let title = "FRANKENTUI DASHBOARD";
        let effect = TextEffect::AnimatedGradient {
            gradient: ColorGradient::cyberpunk(),
            speed: 0.3,
        };

        let styled = StyledText::new(title).effect(effect).bold().time(self.time);

        styled.render(area, frame);
    }

    /// Render mini plasma effect using Braille canvas.
    fn render_plasma(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.width < 4 || area.height < 3 {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Plasma")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::DASHBOARD));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() || inner.width < 2 || inner.height < 2 {
            return;
        }

        let mut painter = Painter::for_area(inner, Mode::Braille);
        let (pw, ph) = painter.size();

        // Simple plasma using two sine waves
        let t = self.time * 0.5;
        for py in 0..ph as i32 {
            for px in 0..pw as i32 {
                let x = px as f64 / pw as f64;
                let y = py as f64 / ph as f64;

                // Two-wave plasma formula
                let v1 = (x * 10.0 + t * 2.0).sin();
                let v2 = (y * 10.0 + t * 1.5).sin();
                let v3 = ((x + y) * 8.0 + t).sin();
                let v = (v1 + v2 + v3) / 3.0;

                // Map to HSV color
                let hue = (v + 1.0) / 2.0 * 360.0 + t * 50.0;
                let color = hsv_to_rgb(hue, 0.9, 0.9);

                painter.point_colored(px, py, color);
            }
        }

        Canvas::from_painter(&painter)
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    /// Render sparklines panel.
    fn render_sparklines(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.height < 3 {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Charts")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::DATA_VIZ));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() || inner.height < 2 {
            return;
        }

        // Split into rows for each sparkline
        let rows = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),
                Constraint::Fixed(1),
                Constraint::Fixed(1),
            ])
            .split(inner);

        let cpu_data: Vec<f64> = self.simulated_data.cpu_history.iter().copied().collect();
        let mem_data: Vec<f64> = self.simulated_data.memory_history.iter().copied().collect();
        let net_data: Vec<f64> = self.simulated_data.network_in.iter().copied().collect();

        // CPU sparkline
        if !rows[0].is_empty() && !cpu_data.is_empty() {
            let label_area = Rect::new(rows[0].x, rows[0].y, 4.min(rows[0].width), 1);
            let spark_area = Rect::new(
                rows[0].x + 4.min(rows[0].width),
                rows[0].y,
                rows[0].width.saturating_sub(4),
                1,
            );
            Paragraph::new("CPU ")
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(label_area, frame);
            if !spark_area.is_empty() {
                Sparkline::new(&cpu_data)
                    .style(Style::new().fg(theme::accent::PRIMARY))
                    .gradient(
                        theme::accent::PRIMARY.into(),
                        theme::accent::ACCENT_7.into(),
                    )
                    .render(spark_area, frame);
            }
        }

        // Memory sparkline
        if rows.len() > 1 && !rows[1].is_empty() && !mem_data.is_empty() {
            let label_area = Rect::new(rows[1].x, rows[1].y, 4.min(rows[1].width), 1);
            let spark_area = Rect::new(
                rows[1].x + 4.min(rows[1].width),
                rows[1].y,
                rows[1].width.saturating_sub(4),
                1,
            );
            Paragraph::new("MEM ")
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(label_area, frame);
            if !spark_area.is_empty() {
                Sparkline::new(&mem_data)
                    .style(Style::new().fg(theme::accent::SUCCESS))
                    .gradient(
                        theme::accent::SUCCESS.into(),
                        theme::accent::ACCENT_9.into(),
                    )
                    .render(spark_area, frame);
            }
        }

        // Network sparkline
        if rows.len() > 2 && !rows[2].is_empty() && !net_data.is_empty() {
            let label_area = Rect::new(rows[2].x, rows[2].y, 4.min(rows[2].width), 1);
            let spark_area = Rect::new(
                rows[2].x + 4.min(rows[2].width),
                rows[2].y,
                rows[2].width.saturating_sub(4),
                1,
            );
            Paragraph::new("NET ")
                .style(Style::new().fg(theme::fg::SECONDARY))
                .render(label_area, frame);
            if !spark_area.is_empty() {
                Sparkline::new(&net_data)
                    .style(Style::new().fg(theme::accent::WARNING))
                    .gradient(
                        theme::accent::WARNING.into(),
                        theme::accent::ACCENT_10.into(),
                    )
                    .render(spark_area, frame);
            }
        }
    }

    /// Render syntax-highlighted code preview.
    fn render_code(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.height < 3 {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Code")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::CODE_EXPLORER));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        // Sample Rust code
        let code = "// FrankenTUI\nuse ftui::*;\n\nfn main() {\n  App::run()\n}";
        let highlighted = self.highlighter.highlight(code, "rs");

        // Render as paragraph with styled text
        render_text(frame, inner, &highlighted);
    }

    /// Render system info panel.
    ///
    /// `dashboard_size` is the total dashboard area (width, height) for display.
    fn render_info(&self, frame: &mut Frame, area: Rect, dashboard_size: (u16, u16)) {
        if area.is_empty() || area.height < 3 {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Info")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::PERFORMANCE));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let theme_name = theme::current_theme_name();
        let info = format!(
            "FPS: {:.0}\n{}x{}\n{}\nTick: {}",
            self.fps, dashboard_size.0, dashboard_size.1, theme_name, self.tick_count
        );

        Paragraph::new(info)
            .style(Style::new().fg(theme::fg::SECONDARY))
            .render(inner, frame);
    }

    /// Render markdown preview.
    fn render_markdown(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || area.height < 2 {
            return;
        }

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Markdown")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::MARKDOWN));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        // Compact GFM sample
        let md = "**Bold** _italic_ `code` $E=mc^2$\n- [x] TUI framework";
        let rendered = self.md_renderer.render(md);

        render_text(frame, inner, &rendered);
    }

    /// Render navigation footer.
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let hint = "1-9:screens | Tab:next | t:theme | ?:help | q:quit";
        Paragraph::new(hint)
            .style(Style::new().fg(theme::fg::MUTED).bg(theme::alpha::SURFACE))
            .render(area, frame);
    }

    // =========================================================================
    // Layout Variants
    // =========================================================================

    /// Large layout (100x30+).
    fn render_large(&self, frame: &mut Frame, area: Rect) {
        // Main vertical split: header, content, footer
        let main = Flex::vertical()
            .constraints([
                Constraint::Fixed(1), // Header
                Constraint::Min(10),  // Content
                Constraint::Fixed(1), // Footer
            ])
            .split(area);

        self.render_header(frame, main[0]);
        self.render_footer(frame, main[2]);

        // Content area: split into top row and bottom row
        let content_rows = Flex::vertical()
            .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
            .split(main[1]);

        // Top row: 4 panels (plasma, charts, code, info)
        let top_cols = Flex::horizontal()
            .constraints([
                Constraint::Percentage(20.0),
                Constraint::Percentage(30.0),
                Constraint::Percentage(30.0),
                Constraint::Percentage(20.0),
            ])
            .split(content_rows[0]);

        self.render_plasma(frame, top_cols[0]);
        self.render_sparklines(frame, top_cols[1]);
        self.render_code(frame, top_cols[2]);
        self.render_info(frame, top_cols[3], (area.width, area.height));

        // Bottom row: markdown preview
        self.render_markdown(frame, content_rows[1]);
    }

    /// Medium layout (70x20+).
    fn render_medium(&self, frame: &mut Frame, area: Rect) {
        let main = Flex::vertical()
            .constraints([
                Constraint::Fixed(1), // Header
                Constraint::Min(8),   // Content
                Constraint::Fixed(1), // Footer
            ])
            .split(area);

        self.render_header(frame, main[0]);
        self.render_footer(frame, main[2]);

        // Content: top row with panels, bottom row with markdown
        let content_rows = Flex::vertical()
            .constraints([Constraint::Percentage(65.0), Constraint::Percentage(35.0)])
            .split(main[1]);

        // Top row: 3 panels
        let top_cols = Flex::horizontal()
            .constraints([
                Constraint::Percentage(25.0),
                Constraint::Percentage(40.0),
                Constraint::Percentage(35.0),
            ])
            .split(content_rows[0]);

        self.render_plasma(frame, top_cols[0]);
        self.render_sparklines(frame, top_cols[1]);

        // Combined code + info in the third column
        let right_split = Flex::vertical()
            .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
            .split(top_cols[2]);

        self.render_code(frame, right_split[0]);
        self.render_info(frame, right_split[1], (area.width, area.height));

        self.render_markdown(frame, content_rows[1]);
    }

    /// Tiny layout (<70x20).
    fn render_tiny(&self, frame: &mut Frame, area: Rect) {
        let main = Flex::vertical()
            .constraints([
                Constraint::Fixed(1), // Header
                Constraint::Min(4),   // Content
                Constraint::Fixed(1), // Footer
            ])
            .split(area);

        self.render_header(frame, main[0]);

        // Compact footer
        let hint = "t:theme q:quit";
        Paragraph::new(hint)
            .style(Style::new().fg(theme::fg::MUTED).bg(theme::alpha::SURFACE))
            .render(main[2], frame);

        // Content: two columns
        let cols = Flex::horizontal()
            .constraints([Constraint::Percentage(35.0), Constraint::Percentage(65.0)])
            .split(main[1]);

        // Left: plasma
        self.render_plasma(frame, cols[0]);

        // Right: compact info with sparklines
        let right_rows = Flex::vertical()
            .constraints([Constraint::Min(1), Constraint::Fixed(2)])
            .split(cols[1]);

        // Sparklines (just CPU and MEM)
        if !right_rows[0].is_empty() {
            let spark_rows = Flex::vertical()
                .constraints([Constraint::Fixed(1), Constraint::Fixed(1)])
                .split(right_rows[0]);

            let cpu_data: Vec<f64> = self.simulated_data.cpu_history.iter().copied().collect();
            let mem_data: Vec<f64> = self.simulated_data.memory_history.iter().copied().collect();

            if !spark_rows[0].is_empty() && !cpu_data.is_empty() {
                let label_w = 4.min(spark_rows[0].width);
                Paragraph::new("CPU ")
                    .style(Style::new().fg(theme::fg::SECONDARY))
                    .render(
                        Rect::new(spark_rows[0].x, spark_rows[0].y, label_w, 1),
                        frame,
                    );
                let spark_area = Rect::new(
                    spark_rows[0].x + label_w,
                    spark_rows[0].y,
                    spark_rows[0].width.saturating_sub(label_w),
                    1,
                );
                if !spark_area.is_empty() {
                    Sparkline::new(&cpu_data)
                        .style(Style::new().fg(theme::accent::PRIMARY))
                        .render(spark_area, frame);
                }
            }

            if spark_rows.len() > 1 && !spark_rows[1].is_empty() && !mem_data.is_empty() {
                let label_w = 4.min(spark_rows[1].width);
                Paragraph::new("MEM ")
                    .style(Style::new().fg(theme::fg::SECONDARY))
                    .render(
                        Rect::new(spark_rows[1].x, spark_rows[1].y, label_w, 1),
                        frame,
                    );
                let spark_area = Rect::new(
                    spark_rows[1].x + label_w,
                    spark_rows[1].y,
                    spark_rows[1].width.saturating_sub(label_w),
                    1,
                );
                if !spark_area.is_empty() {
                    Sparkline::new(&mem_data)
                        .style(Style::new().fg(theme::accent::SUCCESS))
                        .render(spark_area, frame);
                }
            }
        }

        // Compact info
        if !right_rows[1].is_empty() {
            let info = format!("FPS:{:.0} {}x{}", self.fps, area.width, area.height);
            Paragraph::new(info)
                .style(Style::new().fg(theme::fg::MUTED))
                .render(right_rows[1], frame);
        }
    }
}

/// Helper to render Text widget line by line.
fn render_text(frame: &mut Frame, area: Rect, text: &Text) {
    if area.is_empty() {
        return;
    }

    let lines = text.lines();
    for (i, line) in lines.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }
        let line_y = area.y + i as u16;
        // Render each span in the line
        let mut x_offset = 0u16;
        for span in line.spans() {
            let text_len = span.content.chars().count() as u16;
            if x_offset >= area.width {
                break;
            }
            let span_area = Rect::new(
                area.x + x_offset,
                line_y,
                (area.width - x_offset).min(text_len),
                1,
            );
            let style = span.style.unwrap_or_default();
            Paragraph::new(span.content.as_ref())
                .style(style)
                .render(span_area, frame);
            x_offset += text_len;
        }
    }
}

/// Convert HSV to RGB PackedRgba.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> PackedRgba {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    PackedRgba::rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

impl Screen for Dashboard {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        // Handle 'r' to reset animations
        if let Event::Key(KeyEvent {
            code: KeyCode::Char('r'),
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            self.tick_count = 0;
            self.time = 0.0;
        }

        Cmd::None
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        self.time = tick_count as f64 * 0.1; // 100ms per tick
        self.simulated_data.tick(tick_count);
        self.update_fps();
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        // Choose layout based on terminal size
        match (area.width, area.height) {
            (w, h) if w >= 100 && h >= 30 => self.render_large(frame, area),
            (w, h) if w >= 70 && h >= 20 => self.render_medium(frame, area),
            _ => self.render_tiny(frame, area),
        }
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "r",
                action: "Reset animations",
            },
            HelpEntry {
                key: "t",
                action: "Cycle theme",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Dashboard"
    }

    fn tab_label(&self) -> &'static str {
        "Dashboard"
    }
}
