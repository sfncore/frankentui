//! Config data types for tmuxrs layout configs.

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
    pub panes: Vec<String>,
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
}
