//! CLI wrapper for the `tmuxrs` binary.
//!
//! Shells out to `tmuxrs` for layout config operations. User must have
//! `tmuxrs` installed (`cargo install tmuxrs`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::model::{TmuxrsConfig, TmuxrsWindow};

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

/// Parse a tmuxrs YAML config (simple field extraction, no full YAML parser dep).
fn parse_yaml_config(name: &str, content: &str) -> TmuxrsConfig {
    let mut root = None;
    let mut windows = Vec::new();
    let mut current_window: Option<(String, Option<String>, Vec<String>)> = None;
    let mut in_panes = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Top-level root
        if trimmed.starts_with("root:") {
            root = Some(trimmed.trim_start_matches("root:").trim().to_string());
            continue;
        }

        // Start of a window entry
        if trimmed.starts_with("- name:") && !in_panes {
            // Save previous window
            if let Some((wname, wlayout, wpanes)) = current_window.take() {
                windows.push(TmuxrsWindow {
                    name: wname,
                    layout: wlayout,
                    panes: wpanes,
                });
            }
            let wname = trimmed.trim_start_matches("- name:").trim().to_string();
            current_window = Some((wname, None, Vec::new()));
            in_panes = false;
            continue;
        }

        if let Some((_, ref mut wlayout, _)) = current_window {
            if trimmed.starts_with("layout:") {
                *wlayout = Some(trimmed.trim_start_matches("layout:").trim().to_string());
                continue;
            }
        }

        if trimmed == "panes:" {
            in_panes = true;
            continue;
        }

        if in_panes && trimmed.starts_with("- ") {
            if let Some((_, _, ref mut wpanes)) = current_window {
                let pane_cmd = trimmed.trim_start_matches("- ").trim_matches('"').to_string();
                wpanes.push(pane_cmd);
            }
            continue;
        }

        // If we hit a non-pane line, stop collecting panes
        if in_panes && !trimmed.is_empty() && !trimmed.starts_with('#') {
            in_panes = false;
        }
    }

    // Save last window
    if let Some((wname, wlayout, wpanes)) = current_window {
        windows.push(TmuxrsWindow {
            name: wname,
            layout: wlayout,
            panes: wpanes,
        });
    }

    TmuxrsConfig {
        name: name.to_string(),
        root,
        windows,
    }
}

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
        // Try .yaml extension
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
