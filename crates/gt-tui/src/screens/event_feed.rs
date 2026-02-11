//! Event Feed screen — search, filter, and rich color-coded event stream.
//!
//! All events come from real sources:
//! - EventTailer (tails ~/.events.jsonl)
//! - StatusRefresh deltas (rig/agent/polecat changes)
//! - CommandOutput results

use std::cell::Cell;

use ftui_core::event::{KeyCode, KeyEvent, Modifiers, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::{Line, Text};
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::log_viewer::{LogViewer, LogViewerState, LogWrapMode, SearchConfig, SearchMode};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::{StatefulWidget, Widget};

use crate::data::GtEvent;
use crate::msg::Msg;

const MAX_FEED_LINES: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventCategory {
    Sling,
    Mail,
    Merge,
    Error,
    Status,
    Patrol,
}

impl EventCategory {
    fn icon(self) -> &'static str {
        match self {
            Self::Sling => "\u{2192}",
            Self::Mail => "\u{2709}",
            Self::Merge => "\u{2295}",
            Self::Error => "\u{2717}",
            Self::Status => "\u{25cf}",
            Self::Patrol => "\u{2691}",
        }
    }

    fn color(self) -> theme::ColorToken {
        match self {
            Self::Sling => theme::accent::SUCCESS,
            Self::Mail => theme::accent::INFO,
            Self::Merge => theme::accent::SUCCESS,
            Self::Error => theme::accent::ERROR,
            Self::Status => theme::accent::ACCENT_5,
            Self::Patrol => theme::accent::WARNING,
        }
    }

    /// Classify a GtEvent into a category by looking at its type string.
    fn from_event_type(event_type: &str) -> Self {
        let lower = event_type.to_lowercase();
        if lower.contains("sling") || lower.contains("dispatch") || lower.contains("spawn") {
            Self::Sling
        } else if lower.contains("mail") || lower.contains("nudge") || lower.contains("message") {
            Self::Mail
        } else if lower.contains("merge") || lower.contains("close") || lower.contains("land") {
            Self::Merge
        } else if lower.contains("error") || lower.contains("fail") || lower.contains("offline")
            || lower.contains("removed") || lower.contains("stopped")
        {
            Self::Error
        } else if lower.contains("patrol") || lower.contains("health") || lower.contains("warning") {
            Self::Patrol
        } else {
            Self::Status
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterMode {
    All,
    Sling,
    Mail,
    Merge,
    Error,
}

impl FilterMode {
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Sling => "sling",
            Self::Mail => "mail",
            Self::Merge => "merge",
            Self::Error => "error",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::All => Self::Sling,
            Self::Sling => Self::Mail,
            Self::Mail => Self::Merge,
            Self::Merge => Self::Error,
            Self::Error => Self::All,
        }
    }

    fn matches(self, cat: EventCategory) -> bool {
        match self {
            Self::All => true,
            Self::Sling => matches!(cat, EventCategory::Sling),
            Self::Mail => matches!(cat, EventCategory::Mail),
            Self::Merge => matches!(cat, EventCategory::Merge),
            Self::Error => matches!(cat, EventCategory::Error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Normal,
    Search,
}

struct FeedEvent {
    category: EventCategory,
    formatted: String,
}

pub struct EventFeedScreen {
    viewer: LogViewer,
    viewer_state: LogViewerState,
    mode: UiMode,
    query: String,
    last_search: String,
    search_config: SearchConfig,
    filter: FilterMode,
    tick_count: u64,
    event_count: u64,
    all_events: Vec<FeedEvent>,
    last_log_area: Cell<Rect>,
}

impl EventFeedScreen {
    pub fn new() -> Self {
        let viewer = LogViewer::new(MAX_FEED_LINES)
            .wrap_mode(LogWrapMode::NoWrap)
            .search_highlight_style(
                Style::new()
                    .fg(theme::bg::BASE)
                    .bg(theme::accent::WARNING)
                    .bold(),
            );

        Self {
            viewer,
            viewer_state: LogViewerState::default(),
            mode: UiMode::Normal,
            query: String::new(),
            last_search: String::new(),
            search_config: SearchConfig {
                mode: SearchMode::Literal,
                case_sensitive: false,
                context_lines: 0,
            },
            filter: FilterMode::All,
            tick_count: 0,
            event_count: 0,
            all_events: Vec::new(),
            last_log_area: Cell::new(Rect::default()),
        }
    }

    /// Push a real GtEvent from the event tailer or status deltas.
    pub fn push_real_event(&mut self, event: &GtEvent) {
        let category = EventCategory::from_event_type(&event.event_type);

        let ts = if event.timestamp.is_empty() {
            String::new()
        } else if event.timestamp.len() > 19 {
            format!("{} ", &event.timestamp[11..19])
        } else {
            format!("{} ", &event.timestamp)
        };

        let actor = if event.actor.is_empty() {
            String::new()
        } else {
            format!("{}: ", event.actor)
        };

        let formatted = format!(
            "{}{} {}{}",
            ts,
            category.icon(),
            actor,
            event.message,
        );

        let line = Line::styled(formatted.clone(), Style::new().fg(category.color()));

        if self.filter.matches(category) {
            self.viewer.push(line);
        }

        self.all_events.push(FeedEvent {
            category,
            formatted,
        });
        self.event_count += 1;

        if self.all_events.len() > MAX_FEED_LINES {
            self.all_events.remove(0);
        }
    }

    fn rebuild_viewer(&mut self) {
        self.viewer = LogViewer::new(MAX_FEED_LINES)
            .wrap_mode(LogWrapMode::NoWrap)
            .search_highlight_style(
                Style::new()
                    .fg(theme::bg::BASE)
                    .bg(theme::accent::WARNING)
                    .bold(),
            );
        self.viewer_state = LogViewerState::default();

        for event in &self.all_events {
            if self.filter.matches(event.category) {
                let line = Line::styled(
                    event.formatted.clone(),
                    Style::new().fg(event.category.color()),
                );
                self.viewer.push(line);
            }
        }

        if !self.last_search.is_empty() {
            self.viewer
                .search_with_config(&self.last_search, self.search_config.clone());
        }
    }

    pub fn consumes_text_input(&self) -> bool {
        self.mode == UiMode::Search
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match self.mode {
            UiMode::Normal => self.handle_normal_key(key),
            UiMode::Search => self.handle_search_key(key),
        }
        Cmd::None
    }

    pub fn handle_mouse(&mut self, mouse: &ftui_core::event::MouseEvent) -> Cmd<Msg> {
        let log_area = self.last_log_area.get();
        if !log_area.contains(mouse.x, mouse.y) {
            return Cmd::None;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.viewer.scroll_up(3);
            }
            MouseEventKind::ScrollDown => {
                self.viewer.scroll_down(3);
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_normal_key(&mut self, key: &KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('/'), Modifiers::NONE) => {
                self.mode = UiMode::Search;
                self.query.clear();
            }
            (KeyCode::Char('f'), Modifiers::NONE) => {
                self.filter = self.filter.next();
                self.rebuild_viewer();
            }
            (KeyCode::Char('n'), Modifiers::NONE) => {
                self.viewer.next_match();
            }
            (KeyCode::Char('N'), Modifiers::NONE) => {
                self.viewer.prev_match();
            }
            (KeyCode::Char('G'), Modifiers::NONE) => {
                self.viewer.scroll_to_bottom();
            }
            (KeyCode::Char('g'), Modifiers::NONE) => {
                self.viewer.scroll_to_top();
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), Modifiers::NONE) => {
                self.viewer.scroll_up(1);
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), Modifiers::NONE) => {
                self.viewer.scroll_down(1);
            }
            (KeyCode::PageUp, _) => {
                self.viewer.page_up(&self.viewer_state);
            }
            (KeyCode::PageDown, _) => {
                self.viewer.page_down(&self.viewer_state);
            }
            (KeyCode::Escape, _) => {
                self.viewer.clear_search();
                self.last_search.clear();
            }
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: &KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Escape, _) => {
                self.mode = UiMode::Normal;
            }
            (KeyCode::Enter, _) => {
                if !self.query.is_empty() {
                    self.last_search = self.query.clone();
                    self.viewer
                        .search_with_config(&self.last_search, self.search_config.clone());
                }
                self.mode = UiMode::Normal;
            }
            (KeyCode::Backspace, _) => {
                self.query.pop();
                if self.query.is_empty() {
                    self.viewer.clear_search();
                    self.last_search.clear();
                } else {
                    self.viewer
                        .search_with_config(&self.query, self.search_config.clone());
                }
            }
            (KeyCode::Char('c'), m) if m.contains(Modifiers::CTRL) => {
                self.search_config.case_sensitive = !self.search_config.case_sensitive;
                if !self.query.is_empty() {
                    self.viewer
                        .search_with_config(&self.query, self.search_config.clone());
                }
            }
            (KeyCode::Char(c), _) => {
                self.query.push(c);
                self.viewer
                    .search_with_config(&self.query, self.search_config.clone());
            }
            _ => {}
        }
    }

    fn render_search_bar(&self, frame: &mut Frame, area: Rect) {
        let prompt = if self.search_config.case_sensitive {
            "Search [Aa]: "
        } else {
            "Search: "
        };
        let text = format!("{}{}", prompt, self.query);
        let style = Style::new().fg(theme::accent::WARNING).bold();
        Paragraph::new(Text::styled(text, style)).render(area, frame);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 || area.width < 10 {
            return;
        }

        let filter_label = self.filter.label();

        let match_info = if !self.last_search.is_empty() {
            if let Some((pos, total)) = self.viewer.search_info() {
                format!(" | match {}/{}", pos, total)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let status = format!(
            " filter: {} | events: {}{}  [/]search [f]ilter [n/N]ext/prev",
            filter_label, self.event_count, match_info,
        );

        let style = Style::new().fg(theme::fg::MUTED);
        let para = Paragraph::new(Text::from(status)).style(style);
        Widget::render(&para, area, frame);
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        let search_active = self.mode == UiMode::Search;
        let bar_height = if search_active { 2 } else { 1 };

        let sections = Flex::vertical()
            .constraints([Constraint::Min(3), Constraint::Fixed(bar_height)])
            .split(area);

        let log_area = sections[0];
        let bar_area = sections[1];

        let title = if self.filter != FilterMode::All {
            format!(" Event Feed [{}] ", self.filter.label())
        } else {
            " Event Feed ".to_string()
        };

        let border_style = if search_active {
            Style::new().fg(theme::accent::WARNING)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let block = Block::new()
            .title(&title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        self.last_log_area.set(log_area);
        let inner = block.inner(log_area);
        Widget::render(&block, log_area, frame);

        if self.all_events.is_empty() {
            Paragraph::new("Waiting for events... (status changes, mail, commands)")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
        } else {
            let mut state = self.viewer_state.clone();
            StatefulWidget::render(&self.viewer, inner, frame, &mut state);
        }

        if search_active {
            let bar_sections = Flex::vertical()
                .constraints([Constraint::Fixed(1), Constraint::Fixed(1)])
                .split(bar_area);
            self.render_search_bar(frame, bar_sections[0]);
            self.render_status_bar(frame, bar_sections[1]);
        } else {
            self.render_status_bar(frame, bar_area);
        }
    }
}
