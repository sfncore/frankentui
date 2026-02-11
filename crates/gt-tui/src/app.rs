use std::cell::RefCell;
use std::process::Command;
use std::time::{Duration, Instant};

use ftui_core::event::{KeyCode, KeyEventKind, Modifiers, MouseButton, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_runtime::{Cmd, Every, Model, Subscription};
use ftui_widgets::focus::graph::{FocusId, FocusNode, NavDirection};
use ftui_widgets::focus::manager::FocusManager;
use ftui_widgets::list::ListState;
use ftui_widgets::log_viewer::{LogViewer, LogViewerState};
use ftui_widgets::spinner::SpinnerState;
use ftui_widgets::status_line::{StatusItem, StatusLine};
use ftui_widgets::Widget;

use crate::data::{self, ConvoyItem, TownStatus};
use crate::msg::Msg;
use crate::panels;

// ---------------------------------------------------------------------------
// Focus IDs for dashboard panels
// ---------------------------------------------------------------------------

pub const FOCUS_AGENT_TREE: FocusId = 1;
pub const FOCUS_CONVOYS: FocusId = 2;
pub const FOCUS_EVENT_FEED: FocusId = 3;

/// Label for a focus ID.
fn focus_label(id: FocusId) -> &'static str {
    match id {
        FOCUS_AGENT_TREE => "Agents",
        FOCUS_CONVOYS => "Convoys",
        FOCUS_EVENT_FEED => "Events",
        _ => "Unknown",
    }
}

/// Build a FocusManager with panel nodes wired for Tab/BackTab cycling.
fn build_focus_manager() -> FocusManager {
    let mut mgr = FocusManager::new();
    let graph = mgr.graph_mut();

    // Insert nodes (bounds updated each frame during view)
    graph.insert(FocusNode::new(FOCUS_AGENT_TREE, Rect::default()).with_tab_index(0));
    graph.insert(FocusNode::new(FOCUS_CONVOYS, Rect::default()).with_tab_index(1));
    graph.insert(FocusNode::new(FOCUS_EVENT_FEED, Rect::default()).with_tab_index(2));

    // Wire Next/Prev cycle: AgentTree <-> Convoys <-> EventFeed <-> AgentTree
    graph.connect(FOCUS_AGENT_TREE, NavDirection::Next, FOCUS_CONVOYS);
    graph.connect(FOCUS_CONVOYS, NavDirection::Next, FOCUS_EVENT_FEED);
    graph.connect(FOCUS_EVENT_FEED, NavDirection::Next, FOCUS_AGENT_TREE);

    graph.connect(FOCUS_AGENT_TREE, NavDirection::Prev, FOCUS_EVENT_FEED);
    graph.connect(FOCUS_CONVOYS, NavDirection::Prev, FOCUS_AGENT_TREE);
    graph.connect(FOCUS_EVENT_FEED, NavDirection::Prev, FOCUS_CONVOYS);

    // Spatial: Left/Right between sidebar and main panels
    graph.connect(FOCUS_AGENT_TREE, NavDirection::Right, FOCUS_CONVOYS);
    graph.connect(FOCUS_CONVOYS, NavDirection::Left, FOCUS_AGENT_TREE);
    graph.connect(FOCUS_EVENT_FEED, NavDirection::Left, FOCUS_AGENT_TREE);

    // Spatial: Up/Down between convoys and event feed
    graph.connect(FOCUS_CONVOYS, NavDirection::Down, FOCUS_EVENT_FEED);
    graph.connect(FOCUS_EVENT_FEED, NavDirection::Up, FOCUS_CONVOYS);

    // Start with agent tree focused
    mgr.focus(FOCUS_AGENT_TREE);

    mgr
}

/// A flattened tree entry for selection-based navigation.
#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub label: String,
    pub tmux_session: String,
    pub depth: u16,
}

pub struct GtDashboard {
    pub status: TownStatus,
    pub convoys: Vec<ConvoyItem>,
    pub event_viewer: LogViewer,
    pub event_state: RefCell<LogViewerState>,
    pub focus: FocusManager,
    pub spinner_state: SpinnerState,
    pub spinner_tick: u32,
    pub convoy_list_state: RefCell<ListState>,
    pub last_refresh: Instant,
    /// Flattened tree entries for cursor-based navigation
    pub tree_entries: Vec<TreeEntry>,
    /// Selected row in the agent tree
    pub tree_cursor: usize,
    /// Saved rect for the agent tree panel (for mouse hit detection)
    pub tree_area: RefCell<Rect>,
}

impl GtDashboard {
    pub fn new() -> Self {
        let mut event_viewer = LogViewer::new(5_000);
        event_viewer.push("Gas Town TUI starting...");
        event_viewer.push("Press Tab to switch panels, j/k to navigate, Enter to switch tmux");
        event_viewer.push("Arrow keys for spatial navigation between panels");
        event_viewer.push("Click agent names to jump to their tmux session");
        event_viewer.push("");

        Self {
            status: TownStatus {
                name: "Gas Town".to_string(),
                ..Default::default()
            },
            convoys: Vec::new(),
            event_viewer,
            event_state: RefCell::new(LogViewerState::default()),
            focus: build_focus_manager(),
            spinner_state: SpinnerState::default(),
            spinner_tick: 0,
            convoy_list_state: RefCell::new(ListState::default()),
            last_refresh: Instant::now(),
            tree_entries: Vec::new(),
            tree_cursor: 0,
            tree_area: RefCell::new(Rect::default()),
        }
    }

    /// Which panel currently has focus.
    fn active_panel_id(&self) -> FocusId {
        self.focus.current().unwrap_or(FOCUS_AGENT_TREE)
    }

    /// Rebuild the flat tree entries from current status.
    fn rebuild_tree_entries(&mut self) {
        self.tree_entries.clear();

        // Town-level agents
        for agent in &self.status.agents {
            self.tree_entries.push(TreeEntry {
                label: format!("{} ({})", agent.name, agent.role),
                tmux_session: agent.session.clone(),
                depth: 0,
            });
        }

        // Rig agents
        for rig in &self.status.rigs {
            self.tree_entries.push(TreeEntry {
                label: rig.name.clone(),
                tmux_session: String::new(),
                depth: 0,
            });
            for agent in &rig.agents {
                self.tree_entries.push(TreeEntry {
                    label: format!("{} ({})", agent.name, agent.role),
                    tmux_session: agent.session.clone(),
                    depth: 1,
                });
            }
        }

        // Clamp cursor
        if !self.tree_entries.is_empty() && self.tree_cursor >= self.tree_entries.len() {
            self.tree_cursor = self.tree_entries.len() - 1;
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
            KeyCode::Char('c') | KeyCode::Char('C')
                if key.modifiers.contains(Modifiers::CTRL) =>
            {
                return Cmd::Quit;
            }
            KeyCode::Tab => {
                self.focus.navigate(NavDirection::Next);
                self.event_viewer
                    .push(format!("Panel: {}", focus_label(self.active_panel_id())));
                return Cmd::None;
            }
            KeyCode::BackTab => {
                self.focus.navigate(NavDirection::Prev);
                self.event_viewer
                    .push(format!("Panel: {}", focus_label(self.active_panel_id())));
                return Cmd::None;
            }
            // Arrow keys for spatial navigation between panels
            KeyCode::Left if key.modifiers.contains(Modifiers::ALT) => {
                self.focus.navigate(NavDirection::Left);
                return Cmd::None;
            }
            KeyCode::Right if key.modifiers.contains(Modifiers::ALT) => {
                self.focus.navigate(NavDirection::Right);
                return Cmd::None;
            }
            KeyCode::Up if key.modifiers.contains(Modifiers::ALT) => {
                self.focus.navigate(NavDirection::Up);
                return Cmd::None;
            }
            KeyCode::Down if key.modifiers.contains(Modifiers::ALT) => {
                self.focus.navigate(NavDirection::Down);
                return Cmd::None;
            }
            KeyCode::Char('1') => {
                self.focus.focus(FOCUS_AGENT_TREE);
                return Cmd::None;
            }
            KeyCode::Char('2') => {
                self.focus.focus(FOCUS_CONVOYS);
                return Cmd::None;
            }
            KeyCode::Char('3') => {
                self.focus.focus(FOCUS_EVENT_FEED);
                return Cmd::None;
            }
            KeyCode::Char('r') => {
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

        // Panel-specific keys
        let active = self.active_panel_id();
        if active == FOCUS_AGENT_TREE {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !self.tree_entries.is_empty() {
                        self.tree_cursor =
                            (self.tree_cursor + 1).min(self.tree_entries.len() - 1);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.tree_cursor = self.tree_cursor.saturating_sub(1);
                }
                KeyCode::Enter => {
                    self.switch_selected_tmux();
                }
                _ => {}
            }
        } else if active == FOCUS_EVENT_FEED {
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
        } else if active == FOCUS_CONVOYS {
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

        Cmd::None
    }

    fn handle_mouse(&mut self, mouse: ftui_core::event::MouseEvent) -> Cmd<Msg> {
        // Only handle left clicks
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return Cmd::None;
        }

        let tree_area = *self.tree_area.borrow();

        // Check if click is inside the agent tree panel
        if mouse.x >= tree_area.x
            && mouse.x < tree_area.x + tree_area.width
            && mouse.y >= tree_area.y
            && mouse.y < tree_area.y + tree_area.height
        {
            // Map click row to tree entry (accounting for border = 1 row offset)
            let row_in_panel = (mouse.y - tree_area.y).saturating_sub(1) as usize;
            if row_in_panel < self.tree_entries.len() {
                self.tree_cursor = row_in_panel;
                self.focus.focus(FOCUS_AGENT_TREE);

                let entry = &self.tree_entries[row_in_panel];
                if !entry.tmux_session.is_empty() {
                    self.event_viewer.push(format!(
                        "Switching to tmux: {}",
                        entry.tmux_session
                    ));
                    let _ = Command::new("tmux")
                        .args(["switch-client", "-t", &entry.tmux_session])
                        .status();
                } else {
                    self.event_viewer
                        .push(format!("Selected: {} (no session)", entry.label));
                }
            }

            return Cmd::None;
        }

        Cmd::None
    }

    fn switch_selected_tmux(&mut self) {
        if let Some(entry) = self.tree_entries.get(self.tree_cursor) {
            if !entry.tmux_session.is_empty() {
                self.event_viewer.push(format!(
                    "Switching to tmux: {}",
                    entry.tmux_session
                ));
                let _ = Command::new("tmux")
                    .args(["switch-client", "-t", &entry.tmux_session])
                    .status();
            } else {
                self.event_viewer
                    .push(format!("No tmux session for: {}", entry.label));
            }
        }
    }
}

impl Model for GtDashboard {
    type Message = Msg;

    fn init(&mut self) -> Cmd<Self::Message> {
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
                self.rebuild_tree_entries();
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
                Constraint::Fixed(1), // Status bar
                Constraint::Min(6),   // Content
                Constraint::Fixed(1), // Keybinds
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

        // Save tree area for mouse hit detection
        *self.tree_area.borrow_mut() = content[0];

        let active = self.focus.current().unwrap_or(FOCUS_AGENT_TREE);

        // Agent tree (left sidebar) — with cursor
        panels::agent_tree::render(
            frame,
            content[0],
            &self.status,
            active == FOCUS_AGENT_TREE,
            &self.tree_entries,
            self.tree_cursor,
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
            active == FOCUS_CONVOYS,
            &mut convoy_state,
        );
        drop(convoy_state);

        // Event feed panel
        panels::event_feed::render(
            frame,
            main_split[1],
            &self.event_viewer,
            &self.event_state,
            active == FOCUS_EVENT_FEED,
        );

        // --- Keybind Help Line ---
        let panel_label = format!("[{}]", focus_label(active));
        let keybind_bar = StatusLine::new()
            .style(crate::theme::status_bar_style())
            .separator("  ")
            .left(StatusItem::key_hint("Tab", "Panel"))
            .left(StatusItem::key_hint("1-3", "Jump"))
            .left(StatusItem::key_hint("Alt+Arrows", "Spatial"))
            .left(StatusItem::key_hint("j/k", "Nav"))
            .left(StatusItem::key_hint("Enter", "Tmux"))
            .left(StatusItem::key_hint("Click", "Switch"))
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
