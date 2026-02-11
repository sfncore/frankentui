//! Data types for tmux server state.

use ftui_widgets::tree::TreeNode;

use super::client;

// ---------------------------------------------------------------------------
// Info structs (parsed from tmux list-* output)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionInfo {
    pub name: String,
    pub window_count: u32,
    pub attached: bool,
    pub created: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowInfo {
    pub index: u32,
    pub name: String,
    pub active: bool,
    pub pane_count: u32,
    pub layout: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PaneInfo {
    pub id: String,
    pub index: u32,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub command: String,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Snapshot — full server state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub info: SessionInfo,
    pub windows: Vec<WindowSnapshot>,
}

#[derive(Debug, Clone)]
pub struct WindowSnapshot {
    pub info: WindowInfo,
    pub panes: Vec<PaneInfo>,
}

/// Full tmux server state: sessions -> windows -> panes.
#[derive(Debug, Clone, Default)]
pub struct TmuxSnapshot {
    pub sessions: Vec<SessionSnapshot>,
}

/// Which kind of node is selected in the tree.
#[derive(Debug, Clone)]
pub enum TmuxNodeKind {
    Session(String),
    Window(String, u32),       // session_name, window_index
    Pane(String, u32, String), // session_name, window_index, pane_id
}

impl TmuxSnapshot {
    /// Convert to a tree for rendering with ftui-widgets `Tree`.
    pub fn to_tree_node(&self) -> TreeNode {
        let root = TreeNode::new("tmux server").with_children(
            self.sessions
                .iter()
                .map(|sess| {
                    let attached = if sess.info.attached { " *" } else { "" };
                    let label = format!(
                        "{} ({} win{}{})",
                        sess.info.name,
                        sess.info.window_count,
                        if sess.info.window_count == 1 { "" } else { "s" },
                        attached,
                    );
                    TreeNode::new(label).with_children(
                        sess.windows
                            .iter()
                            .map(|win| {
                                let active = if win.info.active { " *" } else { "" };
                                let wlabel = format!(
                                    "{}:{}{} ({}p)",
                                    win.info.index,
                                    win.info.name,
                                    active,
                                    win.info.pane_count,
                                );
                                TreeNode::new(wlabel).with_children(
                                    win.panes
                                        .iter()
                                        .map(|pane| {
                                            let pactive = if pane.active { " *" } else { "" };
                                            TreeNode::new(format!(
                                                "{}{} [{}] {}",
                                                pane.id, pactive, pane.command, pane.path,
                                            ))
                                        })
                                        .collect(),
                                )
                            })
                            .collect(),
                    )
                })
                .collect(),
        );
        root
    }

    /// Map a visible tree row index to the corresponding `TmuxNodeKind`.
    /// Row 0 is the root ("tmux server"), so real content starts at 1.
    pub fn node_at_index(&self, visible_row: usize) -> Option<TmuxNodeKind> {
        // Row 0 = root node, skip it
        let mut idx = 1usize;
        for sess in &self.sessions {
            if idx == visible_row {
                return Some(TmuxNodeKind::Session(sess.info.name.clone()));
            }
            idx += 1;
            for win in &sess.windows {
                if idx == visible_row {
                    return Some(TmuxNodeKind::Window(
                        sess.info.name.clone(),
                        win.info.index,
                    ));
                }
                idx += 1;
                for pane in &win.panes {
                    if idx == visible_row {
                        return Some(TmuxNodeKind::Pane(
                            sess.info.name.clone(),
                            win.info.index,
                            pane.id.clone(),
                        ));
                    }
                    idx += 1;
                }
            }
        }
        None
    }
}

/// Blocking: query full tmux server state.
pub fn fetch_tmux_snapshot() -> TmuxSnapshot {
    let sessions = match client::list_sessions() {
        Ok(s) => s,
        Err(_) => return TmuxSnapshot::default(),
    };

    let mut session_snapshots = Vec::new();
    for sess in sessions {
        let windows = client::list_windows(&sess.name).unwrap_or_default();
        let mut window_snapshots = Vec::new();
        for win in &windows {
            let target = format!("{}:{}", sess.name, win.index);
            let panes = client::list_panes(&target).unwrap_or_default();
            window_snapshots.push(WindowSnapshot {
                info: win.clone(),
                panes,
            });
        }
        session_snapshots.push(SessionSnapshot {
            info: sess,
            windows: window_snapshots,
        });
    }

    TmuxSnapshot {
        sessions: session_snapshots,
    }
}
