//! Low-level tmux command wrapper (pure functions, no state).
//!
//! All functions are blocking and designed for use inside `Cmd::Task`.

use std::fmt;
use std::process::Command;

use super::model::{PaneInfo, SessionInfo, WindowInfo};
use super::pane_control::TmuxContext;

/// Errors from tmux operations.
#[derive(Debug, Clone)]
pub struct TmuxError(pub String);

impl fmt::Display for TmuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tmux: {}", self.0)
    }
}

pub type TmuxResult<T> = Result<T, TmuxError>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run(args: &[&str]) -> TmuxResult<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|e| TmuxError(format!("exec: {e}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(TmuxError(stderr.trim().to_string()))
    }
}

fn run_ok(args: &[&str]) -> bool {
    Command::new("tmux")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Context detection
// ---------------------------------------------------------------------------

/// Detect our tmux context from environment + display-message.
pub fn detect_context() -> TmuxResult<TmuxContext> {
    if std::env::var("TMUX").is_err() {
        return Err(TmuxError("not in tmux".into()));
    }
    let fmt = "#{session_name}\t#{window_index}\t#{window_name}\t#{pane_id}";
    let raw = run(&["display-message", "-p", fmt])?;
    let parts: Vec<&str> = raw.trim().split('\t').collect();
    if parts.len() < 4 {
        return Err(TmuxError("unexpected display-message output".into()));
    }
    Ok(TmuxContext {
        session_name: parts[0].to_string(),
        window_index: parts[1].to_string(),
        window_name: parts[2].to_string(),
        pane_id: parts[3].to_string(),
    })
}

// ---------------------------------------------------------------------------
// Session operations
// ---------------------------------------------------------------------------

pub fn has_session(name: &str) -> bool {
    run_ok(&["has-session", "-t", name])
}

pub fn list_sessions() -> TmuxResult<Vec<SessionInfo>> {
    let fmt = "#{session_name}\t#{session_windows}\t#{session_attached}\t#{session_created}";
    let raw = run(&["list-sessions", "-F", fmt])?;
    let mut sessions = Vec::new();
    for line in raw.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() >= 4 {
            sessions.push(SessionInfo {
                name: p[0].to_string(),
                window_count: p[1].parse().unwrap_or(0),
                attached: p[2] == "1",
                created: p[3].to_string(),
            });
        }
    }
    Ok(sessions)
}

pub fn new_session(name: &str, detached: bool) -> TmuxResult<()> {
    let mut args = vec!["new-session", "-s", name];
    if detached {
        args.push("-d");
    }
    run(&args)?;
    Ok(())
}

pub fn kill_session(name: &str) -> TmuxResult<()> {
    run(&["kill-session", "-t", name])?;
    Ok(())
}

pub fn rename_session(old: &str, new: &str) -> TmuxResult<()> {
    run(&["rename-session", "-t", old, new])?;
    Ok(())
}

pub fn switch_client(target: &str) -> TmuxResult<()> {
    run(&["switch-client", "-t", target])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Window operations
// ---------------------------------------------------------------------------

pub fn list_windows(session: &str) -> TmuxResult<Vec<WindowInfo>> {
    let fmt = "#{window_index}\t#{window_name}\t#{window_active}\t#{window_panes}\t#{window_layout}";
    let raw = run(&["list-windows", "-t", session, "-F", fmt])?;
    let mut windows = Vec::new();
    for line in raw.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() >= 5 {
            windows.push(WindowInfo {
                index: p[0].parse().unwrap_or(0),
                name: p[1].to_string(),
                active: p[2] == "1",
                pane_count: p[3].parse().unwrap_or(0),
                layout: p[4].to_string(),
            });
        }
    }
    Ok(windows)
}

pub fn new_window(session: &str, name: Option<&str>) -> TmuxResult<()> {
    let target = format!("{}:", session);
    let mut args = vec!["new-window", "-t", &target];
    if let Some(n) = name {
        args.extend(["-n", n]);
    }
    run(&args)?;
    Ok(())
}

pub fn kill_window(target: &str) -> TmuxResult<()> {
    run(&["kill-window", "-t", target])?;
    Ok(())
}

pub fn rename_window(target: &str, new_name: &str) -> TmuxResult<()> {
    run(&["rename-window", "-t", target, new_name])?;
    Ok(())
}

pub fn select_window(target: &str) -> TmuxResult<()> {
    run(&["select-window", "-t", target])?;
    Ok(())
}

pub fn link_window(source: &str, dest_session: &str) -> TmuxResult<String> {
    run(&["link-window", "-d", "-s", source, "-t", dest_session])?;
    // Find the new window index (highest)
    let fmt = "#{window_index}";
    let raw = run(&["list-windows", "-t", dest_session, "-F", fmt])?;
    raw.lines()
        .last()
        .map(|l| l.trim().to_string())
        .ok_or_else(|| TmuxError("no windows after link".into()))
}

pub fn unlink_window(target: &str) -> TmuxResult<()> {
    run(&["unlink-window", "-t", target])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pane operations
// ---------------------------------------------------------------------------

pub fn list_panes(target: &str) -> TmuxResult<Vec<PaneInfo>> {
    let fmt = "#{pane_id}\t#{pane_index}\t#{pane_width}\t#{pane_height}\t#{pane_active}\t#{pane_current_command}\t#{pane_current_path}";
    let raw = run(&["list-panes", "-t", target, "-F", fmt])?;
    let mut panes = Vec::new();
    for line in raw.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() >= 7 {
            panes.push(PaneInfo {
                id: p[0].to_string(),
                index: p[1].parse().unwrap_or(0),
                width: p[2].parse().unwrap_or(0),
                height: p[3].parse().unwrap_or(0),
                active: p[4] == "1",
                command: p[5].to_string(),
                path: p[6].to_string(),
            });
        }
    }
    Ok(panes)
}

pub fn split_pane(target: &str, horizontal: bool) -> TmuxResult<()> {
    let flag = if horizontal { "-h" } else { "-v" };
    run(&["split-window", flag, "-t", target])?;
    Ok(())
}

pub fn kill_pane(target: &str) -> TmuxResult<()> {
    run(&["kill-pane", "-t", target])?;
    Ok(())
}

#[allow(dead_code)]
pub fn resize_pane(target: &str, direction: &str, amount: u16) -> TmuxResult<()> {
    let amt = amount.to_string();
    run(&["resize-pane", "-t", target, direction, &amt])?;
    Ok(())
}

#[allow(dead_code)]
pub fn select_pane(target: &str) -> TmuxResult<()> {
    run(&["select-pane", "-t", target])?;
    Ok(())
}

pub fn send_keys(target: &str, keys: &str) -> TmuxResult<()> {
    run(&["send-keys", "-t", target, keys, "Enter"])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

pub fn select_layout(target: &str, layout: &str) -> TmuxResult<()> {
    run(&["select-layout", "-t", target, layout])?;
    Ok(())
}
