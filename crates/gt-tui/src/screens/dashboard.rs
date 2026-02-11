use std::cell::RefCell;

use ftui_core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_widgets::focus::graph::{FocusId, FocusNode, NavDirection};
use ftui_widgets::focus::manager::FocusManager;
use ftui_widgets::list::ListState;
use ftui_widgets::log_viewer::{LogViewer, LogViewerState};

use crate::data::{ConvoyItem, TownStatus};
use crate::msg::Msg;
use crate::panels;
use crate::tmux_pane::{ActivateResult, TmuxPaneControl};

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
    pub running: bool,
}

pub struct DashboardScreen {
    pub focus: FocusManager,
    pub convoy_list_state: RefCell<ListState>,
    pub tree_entries: Vec<TreeEntry>,
    pub tree_cursor: usize,
    pub tree_area: RefCell<Rect>,
    pub tmux_pane: TmuxPaneControl,
}

impl DashboardScreen {
    pub fn new() -> Self {
        Self {
            focus: build_focus_manager(),
            convoy_list_state: RefCell::new(ListState::default()),
            tree_entries: Vec::new(),
            tree_cursor: 0,
            tree_area: RefCell::new(Rect::default()),
            tmux_pane: TmuxPaneControl::new(),
        }
    }

    pub fn active_panel_id(&self) -> FocusId {
        self.focus.current().unwrap_or(FOCUS_AGENT_TREE)
    }

    /// Rebuild the flat tree entries from current status.
    pub fn rebuild_tree_entries(&mut self, status: &TownStatus) {
        // Re-detect pane layout on each status refresh
        self.tmux_pane.scan();
        self.tree_entries.clear();

        // Town-level agents
        for agent in &status.agents {
            self.tree_entries.push(TreeEntry {
                label: format!("{} ({})", agent.name, agent.role),
                tmux_session: agent.session.clone(),
                depth: 0,
                running: agent.running,
            });
        }

        // Rig agents
        for rig in &status.rigs {
            self.tree_entries.push(TreeEntry {
                label: rig.name.clone(),
                tmux_session: String::new(),
                depth: 0,
                running: false,
            });
            for agent in &rig.agents {
                self.tree_entries.push(TreeEntry {
                    label: format!("{} ({})", agent.name, agent.role),
                    tmux_session: agent.session.clone(),
                    depth: 1,
                    running: agent.running,
                });
            }
        }

        // Clamp cursor
        if !self.tree_entries.is_empty() && self.tree_cursor >= self.tree_entries.len() {
            self.tree_cursor = self.tree_entries.len() - 1;
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        event_viewer: &mut LogViewer,
        event_state: &RefCell<LogViewerState>,
        convoys: &[ConvoyItem],
    ) -> Cmd<Msg> {
        match key.code {
            KeyCode::Tab => {
                self.focus.navigate(NavDirection::Next);
                event_viewer
                    .push(format!("Panel: {}", focus_label(self.active_panel_id())));
                return Cmd::None;
            }
            KeyCode::BackTab => {
                self.focus.navigate(NavDirection::Prev);
                event_viewer
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
                    self.peek_selected(event_viewer);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.tree_cursor = self.tree_cursor.saturating_sub(1);
                    self.peek_selected(event_viewer);
                }
                KeyCode::Enter => {
                    self.link_selected(event_viewer);
                }
                KeyCode::Char('s') => {
                    self.switch_selected(event_viewer);
                }
                _ => {}
            }
        } else if active == FOCUS_EVENT_FEED {
            let viewport_h = event_state.borrow().last_viewport_height.max(1);
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    event_viewer.scroll_down(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    event_viewer.scroll_up(1);
                }
                KeyCode::PageDown => {
                    event_viewer.scroll_down(viewport_h as usize);
                }
                KeyCode::PageUp => {
                    event_viewer.scroll_up(viewport_h as usize);
                }
                KeyCode::Char('G') => {
                    event_viewer.scroll_to_bottom();
                }
                KeyCode::Char('g') => {
                    event_viewer.scroll_to_top();
                }
                _ => {}
            }
        } else if active == FOCUS_CONVOYS {
            let mut state = self.convoy_list_state.borrow_mut();
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    let current = state.selected().unwrap_or(0);
                    let max = convoys.len().saturating_sub(1);
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

    pub fn handle_mouse(
        &mut self,
        mouse: ftui_core::event::MouseEvent,
        event_viewer: &mut LogViewer,
    ) -> Cmd<Msg> {
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

                self.peek_selected(event_viewer);
            }

            return Cmd::None;
        }

        Cmd::None
    }

    /// Peek: temp-link the selected agent's window (replaced on next cursor move).
    fn peek_selected(&mut self, event_viewer: &mut LogViewer) {
        let entry = match self.tree_entries.get(self.tree_cursor) {
            Some(e) if e.running && !e.tmux_session.is_empty() => e.clone(),
            _ => return,
        };
        match self.tmux_pane.peek_session(&entry.tmux_session) {
            ActivateResult::Peeked => {
                event_viewer.push(format!("peek: {}", entry.tmux_session));
            }
            ActivateResult::AlreadyPeeked | ActivateResult::AlreadyLinked => {}
            ActivateResult::SameSession => {
                event_viewer.push(format!("{} (this session)", entry.tmux_session));
            }
            ActivateResult::SessionNotFound => {
                event_viewer.push(format!("{} not found", entry.tmux_session));
            }
            _ => {}
        }
    }

    /// Link: permanently keep the selected agent's window in our session.
    fn link_selected(&mut self, event_viewer: &mut LogViewer) {
        let entry = match self.tree_entries.get(self.tree_cursor) {
            Some(e) => e.clone(),
            None => return,
        };
        if !entry.running || entry.tmux_session.is_empty() {
            event_viewer.push(format!("{} is offline", entry.label));
            return;
        }
        match self.tmux_pane.link_session(&entry.tmux_session) {
            ActivateResult::Linked => {
                event_viewer.push(format!("linked: {}", entry.tmux_session));
            }
            ActivateResult::AlreadyLinked => {
                event_viewer.push(format!("already linked: {}", entry.tmux_session));
            }
            ActivateResult::SameSession => {
                event_viewer.push(format!("{} (this session)", entry.tmux_session));
            }
            ActivateResult::SessionNotFound => {
                event_viewer.push(format!("{} not found", entry.tmux_session));
            }
            _ => {}
        }
    }

    /// Switch: jump into the agent's session entirely.
    fn switch_selected(&mut self, event_viewer: &mut LogViewer) {
        let entry = match self.tree_entries.get(self.tree_cursor) {
            Some(e) => e.clone(),
            None => return,
        };
        if !entry.running || entry.tmux_session.is_empty() {
            event_viewer.push(format!("{} is offline", entry.label));
            return;
        }
        match self.tmux_pane.switch_session(&entry.tmux_session) {
            ActivateResult::Switched => {
                event_viewer.push(format!("→ {}", entry.tmux_session));
            }
            ActivateResult::SameSession => {
                event_viewer.push(format!("{} (this session)", entry.tmux_session));
            }
            ActivateResult::SessionNotFound => {
                event_viewer.push(format!("{} not found", entry.tmux_session));
            }
            _ => {}
        }
    }

    pub fn view(
        &self,
        frame: &mut Frame,
        area: Rect,
        status: &TownStatus,
        convoys: &[ConvoyItem],
        event_viewer: &LogViewer,
        event_state: &RefCell<LogViewerState>,
    ) {
        // Content: sidebar (30%) + main (70%)
        let content = Flex::horizontal()
            .constraints([
                Constraint::Percentage(30.0), // Agent tree
                Constraint::Min(20),          // Main content
            ])
            .split(area);

        // Save tree area for mouse hit detection
        *self.tree_area.borrow_mut() = content[0];

        let active = self.focus.current().unwrap_or(FOCUS_AGENT_TREE);

        // Agent tree (left sidebar) — with cursor
        panels::agent_tree::render(
            frame,
            content[0],
            status,
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
            convoys,
            active == FOCUS_CONVOYS,
            &mut convoy_state,
        );
        drop(convoy_state);

        // Event feed panel
        panels::event_feed::render(
            frame,
            main_split[1],
            event_viewer,
            event_state,
            active == FOCUS_EVENT_FEED,
        );
    }
}
