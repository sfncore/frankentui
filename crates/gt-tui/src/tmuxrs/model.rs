//! Config data types for tmuxrs layout configs.
//!
//! Supports three YAML schemas found in real configs:
//!
//! ## Schema 1: Named windows with layout + pane commands
//! ```yaml
//! windows:
//!   - name: main
//!     layout: main-vertical
//!     panes:
//!       - "cargo watch"
//!       - ""
//! ```
//!
//! ## Schema 2: Shorthand — window name maps directly to a command
//! ```yaml
//! windows:
//!   - witness: cd ~/gt && opencode
//!   - refinery: cd ~/gt && opencode
//! ```
//!
//! ## Schema 3: Named windows with structured pane configs
//! ```yaml
//! windows:
//!   - workspace:
//!       panes:
//!         - claude --resume
//!         - session: gt-frankentui-crew-frank
//!           direction: horizontal
//!         - session: hq-deacon
//!           direction: vertical
//!           full: true
//!           size: 30
//! ```

/// A tmuxrs session layout configuration.
#[derive(Debug, Clone)]
pub struct TmuxrsConfig {
    pub name: String,
    pub root: Option<String>,
    pub windows: Vec<TmuxrsWindow>,
}

/// A window within a tmuxrs config.
#[derive(Debug, Clone)]
pub struct TmuxrsWindow {
    pub name: String,
    pub layout: Option<String>,
    pub panes: Vec<PaneConfig>,
}

/// Configuration for a single pane — supports plain commands and session links.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaneConfig {
    /// Shell command to run in this pane (mutually exclusive with `session`).
    pub command: Option<String>,
    /// Tmux session to link into this pane (mutually exclusive with `command`).
    pub session: Option<String>,
    /// Split direction: "horizontal" or "vertical".
    pub direction: Option<String>,
    /// Whether this pane should take full width/height.
    pub full: Option<bool>,
    /// Size hint (percentage or fixed columns/rows).
    pub size: Option<u32>,
}

impl PaneConfig {
    /// Create a PaneConfig from a simple command string.
    pub fn from_command(cmd: &str) -> Self {
        Self {
            command: Some(cmd.to_string()),
            ..Default::default()
        }
    }

    /// Create a PaneConfig that links a tmux session.
    pub fn from_session(session: &str) -> Self {
        Self {
            session: Some(session.to_string()),
            ..Default::default()
        }
    }

    /// Human-readable label for display.
    pub fn label(&self) -> String {
        if let Some(ref sess) = self.session {
            format!("[session: {}]", sess)
        } else if let Some(ref cmd) = self.command {
            if cmd.is_empty() {
                "(shell)".to_string()
            } else {
                cmd.clone()
            }
        } else {
            "(empty)".to_string()
        }
    }

    /// Returns the legacy pane string (for backward compatibility).
    pub fn as_legacy_string(&self) -> String {
        if let Some(ref cmd) = self.command {
            cmd.clone()
        } else if let Some(ref sess) = self.session {
            format!("session:{}", sess)
        } else {
            String::new()
        }
    }
}

impl TmuxrsConfig {
    /// Generate skeleton YAML content for a new config.
    pub fn skeleton_yaml(name: &str) -> String {
        format!(
            r#"name: {name}
root: ~/
windows:
  - name: main
    layout: main-vertical
    panes:
      - ""
      - ""
"#
        )
    }

    /// Total pane count across all windows.
    pub fn pane_count(&self) -> usize {
        self.windows.iter().map(|w| w.panes.len()).sum()
    }
}

impl TmuxrsWindow {
    /// Legacy accessor: pane commands as strings (for backward compat).
    pub fn pane_strings(&self) -> Vec<String> {
        self.panes.iter().map(|p| p.as_legacy_string()).collect()
    }
}

// =========================================================================
// Pure-geometry layout model
// =========================================================================

/// A layout is pure geometry — no session references. Sessions are assigned
/// at runtime in the TUI.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub root: Option<String>,
    pub slots: Vec<LayoutSlot>,
}

/// One slot in a layout — represents a window with a layout preset and pane count.
#[derive(Debug, Clone)]
pub struct LayoutSlot {
    /// Window label (e.g., "main", "agents").
    pub label: String,
    /// Tmux layout preset (e.g., "main-vertical", "even-horizontal").
    pub preset: Option<String>,
    /// Number of panes in this window.
    pub pane_count: usize,
    /// Default commands for panes (from template, optional).
    pub default_commands: Vec<String>,
}

impl Layout {
    /// Convert a parsed TmuxrsConfig into a pure-geometry Layout.
    /// Session references in panes are stripped — only commands are kept as defaults.
    pub fn from_config(config: &TmuxrsConfig) -> Self {
        Layout {
            name: config.name.clone(),
            root: config.root.clone(),
            slots: config
                .windows
                .iter()
                .map(|win| {
                    let default_commands = win
                        .panes
                        .iter()
                        .filter_map(|p| p.command.clone())
                        .filter(|c| !c.is_empty())
                        .collect();
                    LayoutSlot {
                        label: win.name.clone(),
                        preset: win.layout.clone(),
                        pane_count: win.panes.len().max(1),
                        default_commands,
                    }
                })
                .collect(),
        }
    }

    /// Total pane count across all slots.
    pub fn total_panes(&self) -> usize {
        self.slots.iter().map(|s| s.pane_count).sum()
    }
}

impl LayoutSlot {
    /// Create a simple slot with just a label and pane count.
    pub fn new(label: impl Into<String>, pane_count: usize) -> Self {
        Self {
            label: label.into(),
            preset: None,
            pane_count,
            default_commands: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> TmuxrsConfig {
        TmuxrsConfig {
            name: "test-layout".into(),
            root: Some("~/gt".into()),
            windows: vec![
                TmuxrsWindow {
                    name: "main".into(),
                    layout: Some("main-vertical".into()),
                    panes: vec![
                        PaneConfig::from_command("cargo watch"),
                        PaneConfig::from_command(""),
                        PaneConfig::from_session("gt-frank-witness"),
                    ],
                },
                TmuxrsWindow {
                    name: "agents".into(),
                    layout: Some("even-horizontal".into()),
                    panes: vec![
                        PaneConfig::from_command("htop"),
                    ],
                },
            ],
        }
    }

    #[test]
    fn test_layout_from_config_strips_sessions() {
        let config = make_config();
        let layout = Layout::from_config(&config);

        assert_eq!(layout.name, "test-layout");
        assert_eq!(layout.root, Some("~/gt".into()));
        assert_eq!(layout.slots.len(), 2);

        // First slot: 3 panes (including session), but only 1 command default
        let s0 = &layout.slots[0];
        assert_eq!(s0.label, "main");
        assert_eq!(s0.preset.as_deref(), Some("main-vertical"));
        assert_eq!(s0.pane_count, 3);
        // Session reference stripped, empty command stripped
        assert_eq!(s0.default_commands, vec!["cargo watch"]);

        // Second slot
        let s1 = &layout.slots[1];
        assert_eq!(s1.label, "agents");
        assert_eq!(s1.pane_count, 1);
        assert_eq!(s1.default_commands, vec!["htop"]);
    }

    #[test]
    fn test_layout_total_panes() {
        let config = make_config();
        let layout = Layout::from_config(&config);
        assert_eq!(layout.total_panes(), 4); // 3 + 1
    }

    #[test]
    fn test_layout_slot_new() {
        let slot = LayoutSlot::new("my-window", 3);
        assert_eq!(slot.label, "my-window");
        assert_eq!(slot.pane_count, 3);
        assert!(slot.preset.is_none());
        assert!(slot.default_commands.is_empty());
    }

    #[test]
    fn test_layout_from_empty_config() {
        let config = TmuxrsConfig {
            name: "empty".into(),
            root: None,
            windows: vec![],
        };
        let layout = Layout::from_config(&config);
        assert_eq!(layout.slots.len(), 0);
        assert_eq!(layout.total_panes(), 0);
    }

    #[test]
    fn test_layout_window_with_no_panes_gets_min_1() {
        let config = TmuxrsConfig {
            name: "no-panes".into(),
            root: None,
            windows: vec![TmuxrsWindow {
                name: "win".into(),
                layout: None,
                panes: vec![],
            }],
        };
        let layout = Layout::from_config(&config);
        // pane_count.max(1) ensures at least 1
        assert_eq!(layout.slots[0].pane_count, 1);
    }

    #[test]
    fn test_pane_config_labels() {
        assert_eq!(PaneConfig::from_command("ls").label(), "ls");
        assert_eq!(PaneConfig::from_command("").label(), "(shell)");
        assert_eq!(PaneConfig::from_session("my-sess").label(), "[session: my-sess]");
        assert_eq!(PaneConfig::default().label(), "(empty)");
    }

    #[test]
    fn test_skeleton_yaml() {
        let yaml = TmuxrsConfig::skeleton_yaml("my-config");
        assert!(yaml.contains("name: my-config"));
        assert!(yaml.contains("root: ~/"));
        assert!(yaml.contains("main-vertical"));
    }
}
