//! Async action builders returning `Cmd<Msg>`.
//!
//! Each wraps a blocking `client::*` call in `Cmd::Task`.

use ftui_runtime::Cmd;

use super::client;
use super::model;
use crate::msg::Msg;

/// Fetch a full tmux server snapshot (sessions -> windows -> panes).
pub fn fetch_snapshot() -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(|| Msg::TmuxSnapshot(model::fetch_tmux_snapshot())),
    )
}

/// Create a new tmux session.
pub fn create_session(name: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::new_session(&name, true).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("new-session {name}"), result)
        }),
    )
}

/// Kill a tmux session.
pub fn kill_session_cmd(name: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::kill_session(&name).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("kill-session {name}"), result)
        }),
    )
}

/// Rename a tmux session.
pub fn rename_session_cmd(old: String, new: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::rename_session(&old, &new).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("rename-session {old} -> {new}"), result)
        }),
    )
}

/// Switch tmux client to a target session.
pub fn switch_client_cmd(target: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::switch_client(&target).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("switch-client {target}"), result)
        }),
    )
}

/// Create a new window in a session.
pub fn new_window_cmd(session: String, name: Option<String>) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::new_window(&session, name.as_deref()).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("new-window {session}"), result)
        }),
    )
}

/// Kill a window.
pub fn kill_window_cmd(target: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::kill_window(&target).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("kill-window {target}"), result)
        }),
    )
}

/// Rename a window.
pub fn rename_window_cmd(target: String, new_name: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::rename_window(&target, &new_name).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("rename-window {target} -> {new_name}"), result)
        }),
    )
}

/// Split a pane (horizontal or vertical).
pub fn split_pane_cmd(target: String, horizontal: bool) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let dir = if horizontal { "h" } else { "v" };
            let result = client::split_pane(&target, horizontal).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("split-{dir} {target}"), result)
        }),
    )
}

/// Kill a pane.
pub fn kill_pane_cmd(target: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::kill_pane(&target).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("kill-pane {target}"), result)
        }),
    )
}

/// Send keys to a pane.
pub fn send_keys_cmd(target: String, keys: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::send_keys(&target, &keys).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("send-keys {target}"), result)
        }),
    )
}

/// Select layout for a window.
pub fn select_layout_cmd(target: String, layout: String) -> Cmd<Msg> {
    Cmd::Task(
        Default::default(),
        Box::new(move || {
            let result = client::select_layout(&target, &layout).map_err(|e| e.to_string());
            Msg::TmuxActionResult(format!("select-layout {target} {layout}"), result)
        }),
    )
}
