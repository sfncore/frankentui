//! CLI wrapper for the `tmuxrs` binary.
//!
//! Shells out to `tmuxrs` for layout config operations. User must have
//! `tmuxrs` installed (`cargo install tmuxrs`).
//!
//! The YAML parser handles three schemas found in real configs:
//! 1. Named windows: `- name: X / layout: Y / panes: [cmds]`
//! 2. Shorthand: `- window_name: command_string`
//! 3. Structured: `- window_name: { panes: [mixed] }` where mixed is
//!    either a command string or `{ session: X, direction: Y, ... }`

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::model::{PaneConfig, TmuxrsConfig, TmuxrsWindow};

/// Check if the tmuxrs binary is available on PATH.
pub fn tmuxrs_available() -> bool {
    Command::new("which")
        .arg("tmuxrs")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Config directory: ~/.config/tmuxrs/
fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ubuntu".to_string());
    PathBuf::from(home).join(".config").join("tmuxrs")
}

/// List all tmuxrs configs by reading YAML files from the config directory.
pub fn list_configs() -> Result<Vec<TmuxrsConfig>, String> {
    let dir = config_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut configs = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("read dir: {e}"))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yml" && ext != "yaml" {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        match fs::read_to_string(&path) {
            Ok(content) => {
                let config = parse_yaml_config(&name, &content);
                configs.push(config);
            }
            Err(_) => continue,
        }
    }

    configs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(configs)
}

// ---------------------------------------------------------------------------
// YAML Parser — handles all three real-world schemas
// ---------------------------------------------------------------------------

/// Parse a tmuxrs YAML config. Supports all three schemas.
pub fn parse_yaml_config(name: &str, content: &str) -> TmuxrsConfig {
    let mut root = None;
    let mut windows = Vec::new();

    // State machine for parsing
    let mut state = ParseState::TopLevel;
    let mut current_window_name = String::new();
    let mut current_layout: Option<String> = None;
    let mut current_panes: Vec<PaneConfig> = Vec::new();
    // For structured pane parsing (schema 3)
    let mut current_pane_builder: Option<PaneConfigBuilder> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Top-level keys
        if !line.starts_with(' ') && !line.starts_with('\t') && !line.starts_with('-') {
            // Flush any in-progress window
            flush_window(
                &mut windows,
                &state,
                &current_window_name,
                &current_layout,
                &mut current_panes,
                &mut current_pane_builder,
            );
            state = ParseState::TopLevel;

            if trimmed.starts_with("root:") {
                root = Some(trimmed.trim_start_matches("root:").trim().to_string());
            } else if trimmed == "windows:" {
                state = ParseState::InWindows;
            }
            // name: is usually the config name (we already have it from filename)
            continue;
        }

        match state {
            ParseState::TopLevel => {
                if trimmed == "windows:" {
                    state = ParseState::InWindows;
                }
            }
            ParseState::InWindows => {
                if trimmed.starts_with("- ") {
                    // New window entry — flush previous
                    flush_window(
                        &mut windows,
                        &state,
                        &current_window_name,
                        &current_layout,
                        &mut current_panes,
                        &mut current_pane_builder,
                    );
                    current_layout = None;
                    current_panes.clear();
                    current_pane_builder = None;

                    let after_dash = trimmed[2..].trim();

                    if after_dash.starts_with("name:") {
                        // Schema 1: `- name: X`
                        current_window_name =
                            after_dash.trim_start_matches("name:").trim().to_string();
                        state = ParseState::InNamedWindow;
                    } else if let Some((key, value)) = split_yaml_kv(after_dash) {
                        if value.is_empty() {
                            // Schema 3: `- workspace:` (value is a nested block)
                            current_window_name = key.to_string();
                            state = ParseState::InStructuredWindow;
                        } else {
                            // Schema 2: `- witness: command`
                            current_window_name = key.to_string();
                            current_panes.push(PaneConfig::from_command(value));
                            // Stay in InWindows — this window is complete
                            windows.push(TmuxrsWindow {
                                name: current_window_name.clone(),
                                layout: None,
                                panes: current_panes.clone(),
                            });
                            current_panes.clear();
                        }
                    }
                }
            }
            ParseState::InNamedWindow => {
                // Inside a Schema 1 window: expect layout:, panes:, or a new - entry
                if trimmed.starts_with("- ") && !trimmed.starts_with("- \"") && !trimmed.starts_with("- '") {
                    // Could be a pane list item inside panes: or a new window
                    let indent = leading_spaces(line);
                    if indent <= 2 {
                        // New window at same level — go back to InWindows
                        flush_window(
                            &mut windows,
                            &state,
                            &current_window_name,
                            &current_layout,
                            &mut current_panes,
                            &mut current_pane_builder,
                        );
                        current_layout = None;
                        current_panes.clear();
                        current_pane_builder = None;

                        let after_dash = trimmed[2..].trim();
                        if after_dash.starts_with("name:") {
                            current_window_name =
                                after_dash.trim_start_matches("name:").trim().to_string();
                            state = ParseState::InNamedWindow;
                        } else if let Some((key, value)) = split_yaml_kv(after_dash) {
                            if value.is_empty() {
                                current_window_name = key.to_string();
                                state = ParseState::InStructuredWindow;
                            } else {
                                current_window_name = key.to_string();
                                current_panes.push(PaneConfig::from_command(value));
                                windows.push(TmuxrsWindow {
                                    name: current_window_name.clone(),
                                    layout: None,
                                    panes: current_panes.clone(),
                                });
                                current_panes.clear();
                                state = ParseState::InWindows;
                            }
                        }
                        continue;
                    }
                    // Pane item
                    let pane_val = strip_outer_quotes(trimmed[2..].trim());
                    current_panes.push(PaneConfig::from_command(pane_val));
                } else if trimmed.starts_with("layout:") {
                    current_layout =
                        Some(trimmed.trim_start_matches("layout:").trim().to_string());
                } else if trimmed == "panes:" {
                    // Start of panes list — next items are pane entries
                } else if trimmed.starts_with("- ") {
                    // Pane entry (after panes:)
                    let pane_val = strip_outer_quotes(trimmed[2..].trim());
                    current_panes.push(PaneConfig::from_command(pane_val));
                }
            }
            ParseState::InStructuredWindow => {
                // Inside Schema 3: `- workspace: \n    panes: \n      - ...`
                if trimmed.starts_with("- ") {
                    let indent = leading_spaces(line);
                    if indent <= 2 {
                        // New window at top level
                        flush_window(
                            &mut windows,
                            &state,
                            &current_window_name,
                            &current_layout,
                            &mut current_panes,
                            &mut current_pane_builder,
                        );
                        current_layout = None;
                        current_panes.clear();
                        current_pane_builder = None;

                        let after_dash = trimmed[2..].trim();
                        if after_dash.starts_with("name:") {
                            current_window_name =
                                after_dash.trim_start_matches("name:").trim().to_string();
                            state = ParseState::InNamedWindow;
                        } else if let Some((key, value)) = split_yaml_kv(after_dash) {
                            if value.is_empty() {
                                current_window_name = key.to_string();
                                state = ParseState::InStructuredWindow;
                            } else {
                                current_window_name = key.to_string();
                                current_panes.push(PaneConfig::from_command(value));
                                windows.push(TmuxrsWindow {
                                    name: current_window_name.clone(),
                                    layout: None,
                                    panes: current_panes.clone(),
                                });
                                current_panes.clear();
                                state = ParseState::InWindows;
                            }
                        }
                        continue;
                    }

                    // Pane list item
                    // Flush any in-progress structured pane
                    if let Some(builder) = current_pane_builder.take() {
                        current_panes.push(builder.build());
                    }

                    let after_dash = trimmed[2..].trim();
                    // Is this a structured pane (starts with key:) or a plain command?
                    if let Some((key, value)) = split_yaml_kv(after_dash) {
                        if key == "session" {
                            let mut builder = PaneConfigBuilder::default();
                            builder.session = Some(value.to_string());
                            current_pane_builder = Some(builder);
                        } else {
                            // Unknown key — treat as command
                            current_panes
                                .push(PaneConfig::from_command(strip_outer_quotes(after_dash)));
                        }
                    } else {
                        // Plain command
                        current_panes
                            .push(PaneConfig::from_command(strip_outer_quotes(after_dash)));
                    }
                } else if trimmed == "panes:" {
                    // Start panes block
                } else if trimmed.starts_with("layout:") {
                    current_layout =
                        Some(trimmed.trim_start_matches("layout:").trim().to_string());
                } else if let Some((key, value)) = split_yaml_kv(trimmed) {
                    // Continuation of a structured pane entry
                    if let Some(ref mut builder) = current_pane_builder {
                        match key {
                            "session" => builder.session = Some(value.to_string()),
                            "direction" => builder.direction = Some(value.to_string()),
                            "full" => builder.full = Some(value == "true"),
                            "size" => builder.size = value.parse().ok(),
                            "command" => builder.command = Some(value.to_string()),
                            _ => {} // ignore unknown keys
                        }
                    }
                }
            }
        }
    }

    // Flush final window
    flush_window(
        &mut windows,
        &state,
        &current_window_name,
        &current_layout,
        &mut current_panes,
        &mut current_pane_builder,
    );

    TmuxrsConfig {
        name: name.to_string(),
        root,
        windows,
    }
}

// ---------------------------------------------------------------------------
// Parser internals
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum ParseState {
    TopLevel,
    InWindows,
    InNamedWindow,
    InStructuredWindow,
}

#[derive(Default)]
struct PaneConfigBuilder {
    command: Option<String>,
    session: Option<String>,
    direction: Option<String>,
    full: Option<bool>,
    size: Option<u32>,
}

impl PaneConfigBuilder {
    fn build(self) -> PaneConfig {
        PaneConfig {
            command: self.command,
            session: self.session,
            direction: self.direction,
            full: self.full,
            size: self.size,
        }
    }
}

/// Flush current window state into the windows list.
fn flush_window(
    windows: &mut Vec<TmuxrsWindow>,
    state: &ParseState,
    name: &str,
    layout: &Option<String>,
    panes: &mut Vec<PaneConfig>,
    builder: &mut Option<PaneConfigBuilder>,
) {
    // Flush any in-progress structured pane
    if let Some(b) = builder.take() {
        panes.push(b.build());
    }

    if name.is_empty() || panes.is_empty() {
        return;
    }

    match state {
        ParseState::InNamedWindow | ParseState::InStructuredWindow => {
            windows.push(TmuxrsWindow {
                name: name.to_string(),
                layout: layout.clone(),
                panes: panes.clone(),
            });
        }
        _ => {}
    }
}

/// Split `key: value` returning (key, value). Returns None if no colon found.
fn split_yaml_kv(s: &str) -> Option<(&str, &str)> {
    let colon = s.find(':')?;
    let key = s[..colon].trim();
    let value = s[colon + 1..].trim();
    // Don't match things like "cd ~/gt && command" as key:value
    if key.contains(' ') || key.contains('/') || key.contains('&') {
        return None;
    }
    Some((key, value))
}

/// Count leading spaces.
fn leading_spaces(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

/// Strip matching outer quotes (only if both ends match).
fn strip_outer_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        if (s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\''))
        {
            return &s[1..s.len() - 1];
        }
    }
    s
}

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

/// Start a tmuxrs session (detached by default).
#[allow(dead_code)]
pub fn start_session(name: &str, detached: bool) -> Result<String, String> {
    let mut args = vec!["start", name];
    if detached {
        args.push("--no-attach");
    }
    run_tmuxrs(&args)
}

/// Stop a tmuxrs session.
#[allow(dead_code)]
pub fn stop_session(name: &str) -> Result<String, String> {
    run_tmuxrs(&["stop", name])
}

/// Read raw config YAML content.
#[allow(dead_code)]
pub fn read_config(name: &str) -> Result<String, String> {
    let dir = config_dir();
    let path = dir.join(format!("{name}.yml"));
    if !path.exists() {
        let path2 = dir.join(format!("{name}.yaml"));
        return fs::read_to_string(path2).map_err(|e| format!("read: {e}"));
    }
    fs::read_to_string(path).map_err(|e| format!("read: {e}"))
}

/// Write config YAML content.
pub fn write_config(name: &str, content: &str) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let path = dir.join(format!("{name}.yml"));
    fs::write(path, content).map_err(|e| format!("write: {e}"))
}

/// Delete a config file.
pub fn delete_config(name: &str) -> Result<(), String> {
    let dir = config_dir();
    let path = dir.join(format!("{name}.yml"));
    if path.exists() {
        return fs::remove_file(path).map_err(|e| format!("delete: {e}"));
    }
    let path2 = dir.join(format!("{name}.yaml"));
    if path2.exists() {
        return fs::remove_file(path2).map_err(|e| format!("delete: {e}"));
    }
    Err("config not found".into())
}

#[allow(dead_code)]
fn run_tmuxrs(args: &[&str]) -> Result<String, String> {
    let output = Command::new("tmuxrs")
        .args(args)
        .output()
        .map_err(|e| format!("exec tmuxrs: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema1_named_windows() {
        let yaml = r#"
name: frankentui-build
root: ~/gt/frankentui/crew/frank

windows:
  - name: build
    layout: main-vertical
    panes:
      - cargo watch -x 'build -p gt-tui'
      - cargo watch -x 'clippy -p gt-tui'

  - name: test
    layout: even-horizontal
    panes:
      - cargo watch -x 'test -p gt-tui'
      - ""

  - name: run
    layout: main-vertical
    panes:
      - ""
      - ""
"#;
        let config = parse_yaml_config("frankentui-build", yaml);
        assert_eq!(config.name, "frankentui-build");
        assert_eq!(config.root, Some("~/gt/frankentui/crew/frank".to_string()));
        assert_eq!(config.windows.len(), 3);

        let w0 = &config.windows[0];
        assert_eq!(w0.name, "build");
        assert_eq!(w0.layout, Some("main-vertical".to_string()));
        assert_eq!(w0.panes.len(), 2);
        assert_eq!(
            w0.panes[0].command,
            Some("cargo watch -x 'build -p gt-tui'".to_string())
        );

        let w1 = &config.windows[1];
        assert_eq!(w1.name, "test");
        assert_eq!(w1.layout, Some("even-horizontal".to_string()));
        assert_eq!(w1.panes.len(), 2);

        let w2 = &config.windows[2];
        assert_eq!(w2.name, "run");
        assert_eq!(w2.panes.len(), 2);
        assert_eq!(w2.panes[0].command, Some("".to_string()));
    }

    #[test]
    fn test_schema2_shorthand() {
        let yaml = r#"
name: frankencord
root: ~/gt/frankencord
windows:
  - witness: cd ~/gt/frankencord/witness && opencode
  - refinery: cd ~/gt/frankencord/refinery && opencode
"#;
        let config = parse_yaml_config("frankencord", yaml);
        assert_eq!(config.windows.len(), 2);

        assert_eq!(config.windows[0].name, "witness");
        assert_eq!(
            config.windows[0].panes[0].command,
            Some("cd ~/gt/frankencord/witness && opencode".to_string())
        );

        assert_eq!(config.windows[1].name, "refinery");
        assert_eq!(
            config.windows[1].panes[0].command,
            Some("cd ~/gt/frankencord/refinery && opencode".to_string())
        );
    }

    #[test]
    fn test_schema3_structured_panes() {
        let yaml = r#"
name: mayor-workspace
root: ~/gt/mayor
windows:
  - workspace:
      panes:
        - claude --resume
        - session: gt-frankentui-crew-frank
          direction: horizontal
        - session: hq-deacon
          direction: vertical
          full: true
          size: 30
"#;
        let config = parse_yaml_config("mayor-workspace", yaml);
        assert_eq!(config.windows.len(), 1);

        let w = &config.windows[0];
        assert_eq!(w.name, "workspace");
        assert_eq!(w.panes.len(), 3);

        // First pane: plain command
        assert_eq!(w.panes[0].command, Some("claude --resume".to_string()));
        assert_eq!(w.panes[0].session, None);

        // Second pane: session link
        assert_eq!(w.panes[1].command, None);
        assert_eq!(
            w.panes[1].session,
            Some("gt-frankentui-crew-frank".to_string())
        );
        assert_eq!(w.panes[1].direction, Some("horizontal".to_string()));

        // Third pane: session link with full + size
        assert_eq!(w.panes[2].session, Some("hq-deacon".to_string()));
        assert_eq!(w.panes[2].direction, Some("vertical".to_string()));
        assert_eq!(w.panes[2].full, Some(true));
        assert_eq!(w.panes[2].size, Some(30));
    }

    #[test]
    fn test_schema2_single_pane() {
        let yaml = r#"
name: gtui
root: ~/gt
windows:
  - tui: gt-tui
"#;
        let config = parse_yaml_config("gtui", yaml);
        assert_eq!(config.windows.len(), 1);
        assert_eq!(config.windows[0].name, "tui");
        assert_eq!(
            config.windows[0].panes[0].command,
            Some("gt-tui".to_string())
        );
    }

    #[test]
    fn test_mixed_schemas_in_one_file() {
        // Not a real scenario but tests robustness
        let yaml = r#"
name: mixed
root: ~/
windows:
  - name: editor
    layout: main-vertical
    panes:
      - vim
      - ""
  - logs: tail -f /var/log/syslog
"#;
        let config = parse_yaml_config("mixed", yaml);
        assert_eq!(config.windows.len(), 2);
        assert_eq!(config.windows[0].name, "editor");
        assert_eq!(config.windows[0].panes.len(), 2);
        assert_eq!(config.windows[1].name, "logs");
        assert_eq!(
            config.windows[1].panes[0].command,
            Some("tail -f /var/log/syslog".to_string())
        );
    }

    #[test]
    fn test_pane_config_label() {
        let cmd = PaneConfig::from_command("cargo build");
        assert_eq!(cmd.label(), "cargo build");

        let empty = PaneConfig::from_command("");
        assert_eq!(empty.label(), "(shell)");

        let sess = PaneConfig::from_session("gt-mayor");
        assert_eq!(sess.label(), "[session: gt-mayor]");

        let none = PaneConfig::default();
        assert_eq!(none.label(), "(empty)");
    }

    #[test]
    fn test_pane_config_legacy_string() {
        let cmd = PaneConfig::from_command("vim");
        assert_eq!(cmd.as_legacy_string(), "vim");

        let sess = PaneConfig::from_session("my-session");
        assert_eq!(sess.as_legacy_string(), "session:my-session");
    }

    #[test]
    fn test_empty_config() {
        let yaml = "";
        let config = parse_yaml_config("empty", yaml);
        assert_eq!(config.name, "empty");
        assert_eq!(config.windows.len(), 0);
        assert_eq!(config.root, None);
    }

    #[test]
    fn test_config_pane_count() {
        let yaml = r#"
name: test
windows:
  - name: a
    panes:
      - ""
      - ""
  - name: b
    panes:
      - ""
"#;
        let config = parse_yaml_config("test", yaml);
        assert_eq!(config.pane_count(), 3);
    }
}
