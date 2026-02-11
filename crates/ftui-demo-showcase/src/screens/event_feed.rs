#![forbid(unsafe_code)]

//! Event Feed screen — search, filter, and rich color-coded event stream.
//!
//! Demonstrates a Gas Town–style event feed using [`LogViewer`] with:
//!
//! - `/` to open inline search bar (literal search)
//! - `n` / `N` for next/prev match navigation
//! - `f` to cycle filter (all → sling → mail → merge → error)
//! - Rich color-coding per event type via theme accent tokens
//! - Format: `[HH:MM:SS] icon actor: message`

use std::cell::Cell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers, MouseEventKind};
use ftui_core::geometry::Rect;
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

use super::{HelpEntry, Screen};
use crate::theme;

/// Maximum lines retained in the event feed.
const MAX_FEED_LINES: usize = 2_000;
/// Interval between simulated event bursts (in ticks).
const EVENT_BURST_INTERVAL: u64 = 5;

// ---------------------------------------------------------------------------
// Event types and color mapping
// ---------------------------------------------------------------------------

/// Categories of events in the feed.
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
    /// Icon prefix for the event line.
    fn icon(self) -> &'static str {
        match self {
            Self::Sling => "→",
            Self::Mail => "✉",
            Self::Merge => "⊕",
            Self::Error => "✗",
            Self::Status => "●",
            Self::Patrol => "⚑",
        }
    }

    /// Color token for this event type.
    fn color(self) -> theme::ColorToken {
        match self {
            Self::Sling => theme::accent::SUCCESS,   // green
            Self::Mail => theme::accent::INFO,        // blue
            Self::Merge => theme::accent::SUCCESS,    // green
            Self::Error => theme::accent::ERROR,      // red
            Self::Status => theme::accent::ACCENT_5,  // cyan
            Self::Patrol => theme::accent::WARNING,   // yellow
        }
    }
}

/// Filter mode — which event types to show.
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

    /// Returns true if the given event type passes this filter.
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

/// UI interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Normal,
    Search,
}

// ---------------------------------------------------------------------------
// Simulated event data
// ---------------------------------------------------------------------------

/// A single event record.
struct FeedEvent {
    event_type: EventType,
    formatted: String,
}

/// Actors for simulated events.
const ACTORS: &[&str] = &[
    "mayor", "witness", "refinery", "obsidian", "granite", "basalt",
    "slate", "marble", "flint", "quartz",
];

/// Simulated event templates per type.
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
    "bd-{id} status → in_progress",
    "molecule step completed by {target}",
    "heartbeat OK from {target}",
];

const PATROL_MESSAGES: &[&str] = &[
    "patrol sweep: all polecats healthy",
    "warning: {target} idle for 5m",
    "patrol: MQ depth = {id} entries",
    "patrol: checking {target} progress",
];

/// Deterministic hash for reproducible randomness.
fn det_hash(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Generate a simulated event for a given sequence number.
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

    // Simulate time: each event is ~2 seconds apart
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

// ---------------------------------------------------------------------------
// EventFeed screen
// ---------------------------------------------------------------------------

/// Event feed screen with search, filter, and color-coding.
pub struct EventFeed {
    /// LogViewer widget for the event lines.
    viewer: LogViewer,
    /// LogViewer state (scroll position, etc.).
    viewer_state: LogViewerState,
    /// Current UI mode (normal or search).
    mode: UiMode,
    /// Current search query text.
    query: String,
    /// Last applied search string.
    last_search: String,
    /// Search configuration.
    search_config: SearchConfig,
    /// Current filter mode.
    filter: FilterMode,
    /// Global tick counter.
    tick_count: u64,
    /// Number of events generated so far.
    events_generated: u64,
    /// Whether the feed is paused.
    paused: bool,
    /// All events (unfiltered) for re-filtering.
    all_events: Vec<FeedEvent>,
    /// Cached log panel area for mouse hit-testing.
    last_log_area: Cell<Rect>,
}

impl Default for EventFeed {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFeed {
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

        // Seed with initial events
        for _ in 0..30 {
            feed.push_event();
        }

        feed
    }

    /// Generate and push a new event.
    fn push_event(&mut self) {
        let event = generate_event(self.events_generated);
        self.events_generated += 1;

        if self.filter.matches(event.event_type) {
            let line = colorize_event(&event);
            self.viewer.push(line);
        }
        self.all_events.push(event);

        // Trim old events to prevent unbounded growth
        if self.all_events.len() > MAX_FEED_LINES {
            self.all_events.remove(0);
        }
    }

    /// Rebuild the viewer with current filter applied.
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

        // Re-apply search if active
        if !self.last_search.is_empty() {
            self.viewer
                .search_with_config(&self.last_search, self.search_config.clone());
        }
    }

    // -------------------------------------------------------------------
    // Key handling
    // -------------------------------------------------------------------

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

    fn handle_mouse(&mut self, event: &Event) {
        if let Event::Mouse(mouse) = event {
            let log_area = self.last_log_area.get();
            if !log_area.contains(mouse.x, mouse.y) {
                return;
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
        }
    }

    // -------------------------------------------------------------------
    // Rendering helpers
    // -------------------------------------------------------------------

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
}

/// Create a color-styled Line from a FeedEvent.
fn colorize_event(event: &FeedEvent) -> Line {
    let color = event.event_type.color();
    let style = Style::new().fg(color);
    Line::styled(event.formatted.clone(), style)
}

// ---------------------------------------------------------------------------
// Screen trait implementation
// ---------------------------------------------------------------------------

impl Screen for EventFeed {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if matches!(event, Event::Mouse(_)) {
            self.handle_mouse(event);
            return Cmd::none();
        }
        if let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
        {
            match self.mode {
                UiMode::Normal => self.handle_normal_key(key),
                UiMode::Search => self.handle_search_key(key),
            }
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
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

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;

        if !self.paused && tick_count.is_multiple_of(EVENT_BURST_INTERVAL) {
            self.push_event();
        }
    }

    fn consumes_text_input(&self) -> bool {
        self.mode == UiMode::Search
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "/",
                action: "Open search bar",
            },
            HelpEntry {
                key: "f",
                action: "Cycle filter (all/sling/mail/merge/error)",
            },
            HelpEntry {
                key: "n / N",
                action: "Next / previous match",
            },
            HelpEntry {
                key: "Space",
                action: "Pause / resume feed",
            },
            HelpEntry {
                key: "g / G",
                action: "Top / bottom",
            },
            HelpEntry {
                key: "j/k",
                action: "Scroll down / up",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Event Feed"
    }

    fn tab_label(&self) -> &'static str {
        "Events"
    }
}
