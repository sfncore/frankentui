use std::cell::RefCell;
use std::process::Command;
use std::time::{Duration, Instant};

use ftui_core::event::{KeyCode, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_runtime::{Cmd, Every, Model, Subscription};
use ftui_widgets::list::ListState;
use ftui_widgets::log_viewer::{LogViewer, LogViewerState};
use ftui_widgets::spinner::SpinnerState;
use ftui_widgets::status_line::{StatusItem, StatusLine};
use ftui_widgets::Widget;

use crate::data::{self, ConvoyItem, TownStatus};
use crate::msg::Msg;
use crate::panels;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    AgentTree,
    Convoys,
    EventFeed,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::AgentTree => Panel::Convoys,
            Panel::Convoys => Panel::EventFeed,
            Panel::EventFeed => Panel::AgentTree,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Panel::AgentTree => Panel::EventFeed,
            Panel::Convoys => Panel::AgentTree,
            Panel::EventFeed => Panel::Convoys,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Panel::AgentTree => "Agents",
            Panel::Convoys => "Convoys",
            Panel::EventFeed => "Events",
        }
    }
}

pub struct GtDashboard {
    pub status: TownStatus,
    pub convoys: Vec<ConvoyItem>,
    pub event_viewer: LogViewer,
    pub event_state: RefCell<LogViewerState>,
    pub active_panel: Panel,
    pub spinner_state: SpinnerState,
    pub spinner_tick: u32,
    pub convoy_list_state: RefCell<ListState>,
    pub last_refresh: Instant,
}

impl GtDashboard {
    pub fn new() -> Self {
        let mut event_viewer = LogViewer::new(5_000);
        event_viewer.push("Gas Town TUI starting...");
        event_viewer.push("Waiting for events from ~/.events.jsonl");
        event_viewer.push("");

        Self {
            status: TownStatus {
                name: "Gas Town".to_string(),
                ..Default::default()
            },
            convoys: Vec::new(),
            event_viewer,
            event_state: RefCell::new(LogViewerState::default()),
            active_panel: Panel::AgentTree,
            spinner_state: SpinnerState::default(),
            spinner_tick: 0,
            convoy_list_state: RefCell::new(ListState::default()),
            last_refresh: Instant::now(),
        }
    }

    fn handle_key(&mut self, key: ftui_core::event::KeyEvent) -> Cmd<Msg> {
        if key.kind != KeyEventKind::Press {
            return Cmd::None;
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') if !key.modifiers.contains(Modifiers::CTRL) => {
                return Cmd::Quit;
            }
            KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(Modifiers::CTRL) => {
                return Cmd::Quit;
            }
            KeyCode::Tab => {
                self.active_panel = self.active_panel.next();
                return Cmd::None;
            }
            KeyCode::BackTab => {
                self.active_panel = self.active_panel.prev();
                return Cmd::None;
            }
            KeyCode::Char('1') => {
                self.active_panel = Panel::AgentTree;
                return Cmd::None;
            }
            KeyCode::Char('2') => {
                self.active_panel = Panel::Convoys;
                return Cmd::None;
            }
            KeyCode::Char('3') => {
                self.active_panel = Panel::EventFeed;
                return Cmd::None;
            }
            KeyCode::Char('r') => {
                // Force refresh
                self.last_refresh = Instant::now();
                self.event_viewer.push("Refreshing...");
                return Cmd::Batch(vec![
                    Cmd::Task(
                        Default::default(),
                        Box::new(|| Msg::StatusRefresh(data::fetch_status())),
                    ),
                    Cmd::Task(
                        Default::default(),
                        Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
                    ),
                ]);
            }
            _ => {}
        }

        // Panel-specific scrolling
        match self.active_panel {
            Panel::EventFeed => {
                let viewport_h = self.event_state.borrow().last_viewport_height.max(1);
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.event_viewer.scroll_down(1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.event_viewer.scroll_up(1);
                    }
                    KeyCode::PageDown => {
                        self.event_viewer.scroll_down(viewport_h as usize);
                    }
                    KeyCode::PageUp => {
                        self.event_viewer.scroll_up(viewport_h as usize);
                    }
                    KeyCode::Char('G') => {
                        self.event_viewer.scroll_to_bottom();
                    }
                    KeyCode::Char('g') => {
                        self.event_viewer.scroll_to_top();
                    }
                    _ => {}
                }
            }
            Panel::Convoys => {
                let mut state = self.convoy_list_state.borrow_mut();
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let current = state.selected().unwrap_or(0);
                        let max = self.convoys.len().saturating_sub(1);
                        state.select(Some(current.saturating_add(1).min(max)));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let current = state.selected().unwrap_or(0);
                        state.select(Some(current.saturating_sub(1)));
                    }
                    _ => {}
                }
            }
            Panel::AgentTree => {
                match key.code {
                    KeyCode::Enter => {
                        // Clickable tmux session switching — find selected agent and switch
                        self.switch_to_tmux_session();
                    }
                    _ => {}
                }
            }
        }

        Cmd::None
    }

    fn handle_mouse(&mut self, _mouse: ftui_core::event::MouseEvent) -> Cmd<Msg> {
        // Mouse click in agent tree triggers tmux session switch
        // The tree widget uses hit regions, but for now we handle basic clicks
        Cmd::None
    }

    /// Collect all tmux sessions from status for menu navigation.
    pub fn tmux_sessions(&self) -> Vec<(String, String)> {
        let mut sessions = Vec::new();

        // Town-level agents
        for agent in &self.status.agents {
            if !agent.session.is_empty() {
                sessions.push((
                    format!("{} ({})", agent.name, agent.role),
                    agent.session.clone(),
                ));
            }
        }

        // Rig agents
        for rig in &self.status.rigs {
            for agent in &rig.agents {
                if !agent.session.is_empty() {
                    sessions.push((
                        format!("{}/{}", rig.name, agent.name),
                        agent.session.clone(),
                    ));
                }
            }
        }

        sessions
    }

    fn switch_to_tmux_session(&self) {
        // Switch to first available running session
        for agent in &self.status.agents {
            if agent.running && !agent.session.is_empty() {
                let _ = Command::new("tmux")
                    .args(["switch-client", "-t", &agent.session])
                    .status();
                return;
            }
        }

        for rig in &self.status.rigs {
            for agent in &rig.agents {
                if agent.running && !agent.session.is_empty() {
                    let _ = Command::new("tmux")
                        .args(["switch-client", "-t", &agent.session])
                        .status();
                    return;
                }
            }
        }
    }
}

impl Model for GtDashboard {
    type Message = Msg;

    fn init(&mut self) -> Cmd<Self::Message> {
        // Kick off initial data fetch
        Cmd::Batch(vec![
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::StatusRefresh(data::fetch_status())),
            ),
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
            ),
        ])
    }

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        match msg {
            Msg::Key(key) => self.handle_key(key),
            Msg::Mouse(mouse) => self.handle_mouse(mouse),
            Msg::Resize { .. } => Cmd::None,
            Msg::StatusRefresh(status) => {
                self.status = status;
                self.last_refresh = Instant::now();
                Cmd::None
            }
            Msg::ConvoyRefresh(convoys) => {
                self.convoys = convoys;
                Cmd::None
            }
            Msg::NewEvent(event) => {
                panels::event_feed::push_event(&mut self.event_viewer, &event);
                Cmd::None
            }
            Msg::Tick => {
                self.spinner_state.tick();
                self.spinner_tick = self.spinner_tick.wrapping_add(1);
                Cmd::None
            }
            Msg::Noop => Cmd::None,
        }
    }

    fn view(&self, frame: &mut Frame) {
        let area = Rect::from_size(frame.buffer.width(), frame.buffer.height());

        // Fill background
        frame.buffer.fill(
            area,
            Cell::default()
                .with_bg(theme::bg::DEEP.into())
                .with_fg(theme::fg::PRIMARY.into()),
        );

        // Main layout: status bar (1), content (fill), keybinds (1)
        let outer = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),  // Status bar
                Constraint::Min(6),    // Content
                Constraint::Fixed(1),  // Keybinds
            ])
            .split(area);

        // --- Status Bar ---
        panels::status_bar::render(frame, outer[0], &self.status, self.spinner_tick);

        // --- Content: sidebar (30%) + main (70%) ---
        let content = Flex::horizontal()
            .constraints([
                Constraint::Percentage(30.0), // Agent tree
                Constraint::Min(20),          // Main content
            ])
            .split(outer[1]);

        // Agent tree (left sidebar)
        panels::agent_tree::render(
            frame,
            content[0],
            &self.status,
            self.active_panel == Panel::AgentTree,
        );

        // Main content area: convoys (40%) + events (60%)
        let main_split = Flex::vertical()
            .constraints([
                Constraint::Percentage(40.0), // Convoys
                Constraint::Min(4),           // Events
            ])
            .split(content[1]);

        // Convoys panel
        let mut convoy_state = self.convoy_list_state.borrow_mut();
        panels::convoys::render(
            frame,
            main_split[0],
            &self.convoys,
            self.active_panel == Panel::Convoys,
            &mut convoy_state,
        );
        drop(convoy_state);

        // Event feed panel
        panels::event_feed::render(
            frame,
            main_split[1],
            &self.event_viewer,
            &self.event_state,
            self.active_panel == Panel::EventFeed,
        );

        // --- Keybind Help Line ---
        let panel_label = format!("[{}]", self.active_panel.label());
        let keybind_bar = StatusLine::new()
            .style(crate::theme::status_bar_style())
            .separator("  ")
            .left(StatusItem::key_hint("Tab", "Panel"))
            .left(StatusItem::key_hint("1-3", "Jump"))
            .left(StatusItem::key_hint("j/k", "Scroll"))
            .left(StatusItem::key_hint("Enter", "Tmux"))
            .center(StatusItem::text(&panel_label))
            .right(StatusItem::key_hint("r", "Refresh"))
            .right(StatusItem::key_hint("q", "Quit"));

        keybind_bar.render(outer[2], frame);
    }

    fn subscriptions(&self) -> Vec<Box<dyn Subscription<Self::Message>>> {
        vec![
            Box::new(Every::new(Duration::from_millis(100), || Msg::Tick)),
            Box::new(data::StatusPoller),
            Box::new(data::ConvoyPoller),
            Box::new(data::EventTailer),
        ]
    }
}
