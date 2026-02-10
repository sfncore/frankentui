#![forbid(unsafe_code)]

//! Agent Tree screen — displays the Gas Town agent hierarchy using the Tree widget.
//!
//! Shows: Town root → Rig nodes (expandable) → Agent leaves with status indicators.
//! Each agent leaf shows name, role, state icon, and session name.
//! Uses `TreeGuides::Rounded` and supports mouse click detection via `HitId`.
//! On click/Enter: emits `Msg::AgentSelected(agent_address)` for detail view.

use std::cell::Cell as StdCell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::{Frame, HitId};
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::tree::{Tree, TreeGuides, TreeNode};

use super::{HelpEntry, Screen};
use crate::theme;

/// Hit ID for the agent tree widget.
const TREE_HIT_ID: HitId = HitId::new(900);

/// Agent state with display icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentState {
    Running,
    Idle,
    Stuck,
    Zombie,
}

impl AgentState {
    const fn icon(self) -> &'static str {
        match self {
            Self::Running => "\u{25B6}", // ▶
            Self::Idle => "\u{25CB}",    // ○
            Self::Stuck => "\u{25A0}",   // ■
            Self::Zombie => "\u{2620}",  // ☠
        }
    }
}

/// An agent entry in the tree.
#[derive(Debug, Clone)]
struct AgentInfo {
    name: String,
    role: String,
    state: AgentState,
    session: String,
    address: String,
}

impl AgentInfo {
    fn label(&self) -> String {
        format!(
            "{} {} [{}] ({})",
            self.state.icon(),
            self.name,
            self.role,
            self.session,
        )
    }
}

/// A rig entry containing agents.
#[derive(Debug, Clone)]
struct RigInfo {
    name: String,
    agents: Vec<AgentInfo>,
}

/// Build sample agent hierarchy for demonstration.
fn build_sample_hierarchy() -> (Vec<RigInfo>, Vec<String>) {
    let rigs = vec![
        RigInfo {
            name: "frankentui".into(),
            agents: vec![
                AgentInfo {
                    name: "obsidian".into(),
                    role: "polecat".into(),
                    state: AgentState::Running,
                    session: "sess-80f9".into(),
                    address: "frankentui/polecats/obsidian".into(),
                },
                AgentInfo {
                    name: "witness".into(),
                    role: "witness".into(),
                    state: AgentState::Idle,
                    session: "sess-a1b2".into(),
                    address: "frankentui/witness".into(),
                },
                AgentInfo {
                    name: "refinery".into(),
                    role: "refinery".into(),
                    state: AgentState::Idle,
                    session: "sess-c3d4".into(),
                    address: "frankentui/refinery".into(),
                },
            ],
        },
        RigInfo {
            name: "gastown".into(),
            agents: vec![
                AgentInfo {
                    name: "granite".into(),
                    role: "polecat".into(),
                    state: AgentState::Running,
                    session: "sess-e5f6".into(),
                    address: "gastown/polecats/granite".into(),
                },
                AgentInfo {
                    name: "marble".into(),
                    role: "polecat".into(),
                    state: AgentState::Stuck,
                    session: "sess-g7h8".into(),
                    address: "gastown/polecats/marble".into(),
                },
                AgentInfo {
                    name: "witness".into(),
                    role: "witness".into(),
                    state: AgentState::Running,
                    session: "sess-i9j0".into(),
                    address: "gastown/witness".into(),
                },
                AgentInfo {
                    name: "refinery".into(),
                    role: "refinery".into(),
                    state: AgentState::Idle,
                    session: "sess-k1l2".into(),
                    address: "gastown/refinery".into(),
                },
            ],
        },
        RigInfo {
            name: "beads".into(),
            agents: vec![
                AgentInfo {
                    name: "slate".into(),
                    role: "polecat".into(),
                    state: AgentState::Zombie,
                    session: "sess-m3n4".into(),
                    address: "beads/polecats/slate".into(),
                },
                AgentInfo {
                    name: "witness".into(),
                    role: "witness".into(),
                    state: AgentState::Idle,
                    session: "sess-o5p6".into(),
                    address: "beads/witness".into(),
                },
            ],
        },
    ];

    // Build flat address list matching visible tree order for index lookup
    let mut addresses = Vec::new();
    // Root node at index 0
    addresses.push(String::new());
    for rig in &rigs {
        // Rig node
        addresses.push(rig.name.clone());
        for agent in &rig.agents {
            // Agent leaf
            addresses.push(agent.address.clone());
        }
    }

    (rigs, addresses)
}

/// Build the Tree widget from rig/agent data.
fn build_tree(rigs: &[RigInfo]) -> TreeNode {
    let mut root = TreeNode::new("\u{1F3D8} Gas Town"); // 🏘
    for rig in rigs {
        let mut rig_node = TreeNode::new(format!("\u{2699} {}", rig.name)); // ⚙
        for agent in &rig.agents {
            rig_node = rig_node.child(TreeNode::new(agent.label()));
        }
        root = root.child(rig_node);
    }
    root
}

/// Agent Tree screen state.
pub struct AgentTree {
    /// The tree widget holding the hierarchy.
    tree: Tree,
    /// Current cursor position (visible row index).
    cursor: usize,
    /// Flat address list mapping visible index → agent address.
    addresses: Vec<String>,
    /// Last selected agent address (shown in detail panel).
    selected: Option<String>,
    /// Cached layout areas.
    tree_area: StdCell<Rect>,
    detail_area: StdCell<Rect>,
}

impl Default for AgentTree {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentTree {
    /// Create a new agent tree screen with sample data.
    pub fn new() -> Self {
        let (rigs, addresses) = build_sample_hierarchy();
        let root = build_tree(&rigs);
        let tree = Tree::new(root)
            .with_guides(TreeGuides::Rounded)
            .with_show_root(true)
            .with_label_style(Style::new().fg(theme::fg::PRIMARY))
            .with_root_style(Style::new().fg(theme::accent::INFO).bold())
            .with_guide_style(Style::new().fg(theme::fg::MUTED))
            .hit_id(TREE_HIT_ID);

        Self {
            tree,
            cursor: 0,
            addresses,
            selected: None,
            tree_area: StdCell::new(Rect::default()),
            detail_area: StdCell::new(Rect::default()),
        }
    }

    /// Move cursor up in the visible tree.
    fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor down in the visible tree.
    fn cursor_down(&mut self) {
        let max = self.tree.root().visible_count().saturating_sub(1);
        if self.cursor < max {
            self.cursor += 1;
        }
    }

    /// Select the node at the current cursor position.
    fn select_at_cursor(&mut self) {
        // Check if the node has children (rig or root) → toggle expand
        if let Some(node) = self.tree.node_at_visible_index_mut(self.cursor) {
            if !node.children().is_empty() {
                node.toggle_expanded();
                // Rebuild addresses after expansion change
                self.rebuild_addresses();
            } else {
                // Leaf node → select agent
                if let Some(addr) = self.addresses.get(self.cursor) {
                    self.selected = Some(addr.clone());
                }
            }
        }
    }

    /// Rebuild the flat address list after tree expansion changes.
    fn rebuild_addresses(&mut self) {
        let (rigs, _) = build_sample_hierarchy();
        let root = self.tree.root();
        let mut addresses = Vec::new();
        // Root node
        addresses.push(String::new());
        if root.is_expanded() {
            for (i, child) in root.children().iter().enumerate() {
                if let Some(rig) = rigs.get(i) {
                    addresses.push(rig.name.clone());
                    if child.is_expanded() {
                        for agent in &rig.agents {
                            addresses.push(agent.address.clone());
                        }
                    }
                }
            }
        }
        self.addresses = addresses;
    }

    /// Render the tree panel.
    fn render_tree_panel(&self, frame: &mut Frame, area: Rect) {
        let is_focused = true;
        let border_style = theme::panel_border_style(is_focused, theme::screen_accent::FILE_BROWSER);

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Agent Tree ")
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);
        self.tree_area.set(inner);

        // Render the tree
        self.tree.render(inner, frame);

        // Draw cursor highlight by overwriting cell backgrounds
        if inner.height > 0 && self.cursor < inner.height as usize {
            let cursor_y = inner.y + self.cursor as u16;
            let highlight_bg = theme::bg::HIGHLIGHT.resolve();
            for x in inner.x..inner.right() {
                if let Some(cell) = frame.buffer.get_mut(x, cursor_y) {
                    cell.bg = highlight_bg;
                }
            }
        }
    }

    /// Render the detail panel showing selected agent info.
    fn render_detail_panel(&self, frame: &mut Frame, area: Rect) {
        let border_style = theme::panel_border_style(false, theme::screen_accent::FILE_BROWSER);

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Agent Detail ")
            .title_alignment(Alignment::Center)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);
        self.detail_area.set(inner);

        let text = if let Some(addr) = &self.selected {
            format!(
                "Selected: {}\n\nPress Enter on an agent leaf\nto view details here.\n\n\
                 Agent address:\n  {}\n\n\
                 (In a live system, this panel\n would show agent metrics,\n \
                 current task, session info,\n and available actions.)",
                addr, addr,
            )
        } else {
            "No agent selected.\n\n\
             Navigate with \u{2191}/\u{2193} arrows\n\
             and press Enter to select\n\
             an agent leaf node.\n\n\
             Press Space to expand/collapse\n\
             rig nodes."
                .to_string()
        };

        let paragraph = Paragraph::new(text)
            .style(Style::new().fg(theme::fg::SECONDARY));
        paragraph.render(inner, frame);
    }
}

impl Screen for AgentTree {
    type Message = ();

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.cursor_up();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.cursor_down();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.select_at_cursor();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(' '),
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Space toggles expansion without selecting
                if let Some(node) = self.tree.node_at_visible_index_mut(self.cursor) {
                    if !node.children().is_empty() {
                        node.toggle_expanded();
                        self.rebuild_addresses();
                    }
                }
            }
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    let tree_area = self.tree_area.get();
                    if tree_area.contains(mouse.x, mouse.y) {
                        // Calculate which row was clicked
                        let row = (mouse.y - tree_area.y) as usize;
                        let max = self.tree.root().visible_count();
                        if row < max {
                            self.cursor = row;
                            self.select_at_cursor();
                        }
                    }
                }
            }
            _ => {}
        }
        Cmd::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let chunks = Flex::horizontal()
            .constraints([
                Constraint::Percentage(60.0),
                Constraint::Percentage(40.0),
            ])
            .split(area);

        self.render_tree_panel(frame, chunks[0]);
        self.render_detail_panel(frame, chunks[1]);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "\u{2191}/\u{2193}",
                action: "Navigate tree",
            },
            HelpEntry {
                key: "Enter",
                action: "Select/toggle node",
            },
            HelpEntry {
                key: "Space",
                action: "Expand/collapse",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Agent Tree"
    }

    fn tab_label(&self) -> &'static str {
        "Agents"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_tree_creates_with_sample_data() {
        let tree = AgentTree::new();
        assert_eq!(tree.cursor, 0);
        assert!(tree.selected.is_none());
        assert!(!tree.addresses.is_empty());
    }

    #[test]
    fn agent_state_icons_are_distinct() {
        let icons: Vec<&str> = [
            AgentState::Running,
            AgentState::Idle,
            AgentState::Stuck,
            AgentState::Zombie,
        ]
        .iter()
        .map(|s| s.icon())
        .collect();
        // All icons should be unique
        for (i, icon) in icons.iter().enumerate() {
            for (j, other) in icons.iter().enumerate() {
                if i != j {
                    assert_ne!(icon, other, "icons at {i} and {j} should differ");
                }
            }
        }
    }

    #[test]
    fn cursor_movement_stays_in_bounds() {
        let mut tree = AgentTree::new();
        // Already at 0, moving up should stay at 0
        tree.cursor_up();
        assert_eq!(tree.cursor, 0);

        // Move to end
        let max = tree.tree.root().visible_count().saturating_sub(1);
        tree.cursor = max;
        tree.cursor_down();
        assert_eq!(tree.cursor, max);
    }

    #[test]
    fn build_tree_has_correct_structure() {
        let (rigs, _) = build_sample_hierarchy();
        let root = build_tree(&rigs);
        assert_eq!(root.children().len(), 3); // 3 rigs
        assert_eq!(root.children()[0].children().len(), 3); // frankentui: 3 agents
        assert_eq!(root.children()[1].children().len(), 4); // gastown: 4 agents
        assert_eq!(root.children()[2].children().len(), 2); // beads: 2 agents
    }

    #[test]
    fn agent_label_format() {
        let agent = AgentInfo {
            name: "test".into(),
            role: "polecat".into(),
            state: AgentState::Running,
            session: "sess-1234".into(),
            address: "rig/polecats/test".into(),
        };
        let label = agent.label();
        assert!(label.contains("test"));
        assert!(label.contains("polecat"));
        assert!(label.contains("sess-1234"));
        assert!(label.contains(AgentState::Running.icon()));
    }
}
