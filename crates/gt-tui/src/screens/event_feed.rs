//! Event Feed screen — search, filter, and rich color-coded event stream.

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
const EVENT_BURST_INTERVAL: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventType {
    Sling,
    Mail,
    Merge,
    Error,
    Status,
    Patrol,
}

impl EventType {
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

    fn matches(self, event_type: EventType) -> bool {
        match self {
            Self::All => true,
            Self::Sling => event_type == EventType::Sling,
            Self::Mail => event_type == EventType::Mail,
            Self::Merge => event_type == EventType::Merge,
            Self::Error => event_type == EventType::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Normal,
    Search,
}

struct FeedEvent {
    event_type: EventType,
    formatted: String,
}

const ACTORS: &[&str] = &[
    "mayor", "witness", "refinery", "obsidian", "granite", "basalt",
    "slate", "marble", "flint", "quartz",
];

const SLING_MESSAGES: &[&str] = &[
    "dispatched bd-{id} to {target}",
    "created molecule for bd-{id}",
    "assigned work to {target}",
    "spawned polecat {target}",
];

const MAIL_MESSAGES: &[&str] = &[
    "sent nudge to {target}",
    "inbox delivery for {target}",
    "handoff message from {target}",
    "escalation from {target}",
];

const MERGE_MESSAGES: &[&str] = &[
    "merged branch polecat/{target}/bd-{id}",
    "MQ entry processed for bd-{id}",
    "rebase successful for {target}",
    "fast-forward merge bd-{id}",
];

const ERROR_MESSAGES: &[&str] = &[
    "polecat {target} health check failed",
    "zombie detected: {target} unresponsive",
    "build failure on bd-{id}",
    "merge conflict in bd-{id}",
];

const STATUS_MESSAGES: &[&str] = &[
    "polecat {target} checked in",
    "bd-{id} status \u{2192} in_progress",
    "molecule step completed by {target}",
    "heartbeat OK from {target}",
];

const PATROL_MESSAGES: &[&str] = &[
    "patrol sweep: all polecats healthy",
    "warning: {target} idle for 5m",
    "patrol: MQ depth = {id} entries",
    "patrol: checking {target} progress",
];

fn det_hash(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn generate_event(seq: u64) -> FeedEvent {
    let h = det_hash(seq);
    let type_idx = (h % 6) as usize;
    let event_type = match type_idx {
        0 => EventType::Sling,
        1 => EventType::Mail,
        2 => EventType::Merge,
        3 => EventType::Error,
        4 => EventType::Status,
        _ => EventType::Patrol,
    };

    let actor_idx = ((h >> 8) % ACTORS.len() as u64) as usize;
    let actor = ACTORS[actor_idx];
    let target_idx = ((h >> 16) % ACTORS.len() as u64) as usize;
    let target = ACTORS[target_idx];

    let templates = match event_type {
        EventType::Sling => SLING_MESSAGES,
        EventType::Mail => MAIL_MESSAGES,
        EventType::Merge => MERGE_MESSAGES,
        EventType::Error => ERROR_MESSAGES,
        EventType::Status => STATUS_MESSAGES,
        EventType::Patrol => PATROL_MESSAGES,
    };

    let tmpl_idx = ((h >> 24) % templates.len() as u64) as usize;
    let template = templates[tmpl_idx];
    let fake_id = format!("{:04x}", (h >> 32) & 0xFFFF);
    let message = template
        .replace("{target}", target)
        .replace("{id}", &fake_id);

    let total_seconds = seq * 2;
    let hours = (total_seconds / 3600) % 24;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let formatted = format!(
        "[{:02}:{:02}:{:02}] {} {}: {}",
        hours, minutes, seconds,
        event_type.icon(),
        actor,
        message,
    );

    FeedEvent {
        event_type,
        formatted,
    }
}

fn colorize_event(event: &FeedEvent) -> Line {
    let color = event.event_type.color();
    let style = Style::new().fg(color);
    Line::styled(event.formatted.clone(), style)
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
    events_generated: u64,
    paused: bool,
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

        let mut feed = Self {
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
            events_generated: 0,
            paused: false,
            all_events: Vec::new(),
            last_log_area: Cell::new(Rect::default()),
        };

        for _ in 0..30 {
            feed.push_event();
        }

        feed
    }

    fn push_event(&mut self) {
        let event = generate_event(self.events_generated);
        self.events_generated += 1;

        if self.filter.matches(event.event_type) {
            let line = colorize_event(&event);
            self.viewer.push(line);
        }
        self.all_events.push(event);

        if self.all_events.len() > MAX_FEED_LINES {
            self.all_events.remove(0);
        }
    }

    /// Push a real GtEvent from the event tailer into the feed.
    pub fn push_real_event(&mut self, event: &GtEvent) {
        let color = match event.event_type.to_lowercase().as_str() {
            "sling" | "dispatch" => theme::accent::SUCCESS,
            "mail" | "nudge" => theme::accent::INFO,
            "merge" | "close" => theme::accent::SUCCESS,
            "error" | "fail" => theme::accent::ERROR,
            "patrol" | "health" => theme::accent::WARNING,
            _ => theme::fg::SECONDARY,
        };
        let formatted = format!(
            "[{}] {} {}: {}",
            &event.timestamp, event.event_type, event.actor, event.message,
        );
        let line = Line::styled(formatted.clone(), Style::new().fg(color));
        self.viewer.push(line);

        let feed_event = FeedEvent {
            event_type: EventType::Status, // categorize as Status for filter
            formatted,
        };
        self.all_events.push(feed_event);
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
            if self.filter.matches(event.event_type) {
                let line = colorize_event(event);
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
            (KeyCode::Char(' '), Modifiers::NONE) => {
                self.paused = !self.paused;
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
        let pause_label = if self.paused { "PAUSED" } else { "LIVE" };

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
            " filter: {} | {} | events: {}{}  [/]search [f]ilter [Space]pause",
            filter_label, pause_label, self.events_generated, match_info,
        );

        let style = Style::new().fg(theme::fg::MUTED);
        let para = Paragraph::new(Text::from(status)).style(style);
        Widget::render(&para, area, frame);
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;

        if !self.paused && tick_count % EVENT_BURST_INTERVAL == 0 {
            self.push_event();
        }
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

        let mut state = self.viewer_state.clone();
        StatefulWidget::render(&self.viewer, inner, frame, &mut state);

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
