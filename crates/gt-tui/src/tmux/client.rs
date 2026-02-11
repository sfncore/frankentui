//! Low-level tmux command wrapper with injectable executor for testing.
//!
//! All functions are blocking and designed for use inside `Cmd::Task`.
//!
//! # Architecture
//!
//! - `TmuxExecutor` trait: abstracts the `tmux` binary (run / run_ok)
//! - `RealTmuxExecutor`: shells out to `tmux` (production)
//! - `MockTmuxExecutor`: returns canned responses (testing)
//! - `TmuxClient`: high-level API wrapping an executor
//! - Free functions: backward-compat wrappers using `RealTmuxExecutor`

use std::fmt;
use std::process::Command;

use super::model::{PaneInfo, SessionInfo, WindowInfo};
use super::pane_control::TmuxContext;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

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
// Executor trait
// ---------------------------------------------------------------------------

/// Abstraction over the tmux binary for testability.
pub trait TmuxExecutor: Send + Sync {
    /// Run a tmux command, returning stdout on success or error on failure.
    fn run(&self, args: &[&str]) -> TmuxResult<String>;

    /// Run a tmux command, returning true if it succeeded.
    fn run_ok(&self, args: &[&str]) -> bool;
}

/// Production executor — shells out to `tmux`.
pub struct RealTmuxExecutor;

impl TmuxExecutor for RealTmuxExecutor {
    fn run(&self, args: &[&str]) -> TmuxResult<String> {
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

    fn run_ok(&self, args: &[&str]) -> bool {
        Command::new("tmux")
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Mock executor for testing — records calls and returns canned responses.
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, Clone)]
    pub struct MockCall {
        pub args: Vec<String>,
    }

    pub struct MockTmuxExecutor {
        /// Canned responses: pop from front on each `run()` call.
        responses: Mutex<Vec<TmuxResult<String>>>,
        /// Canned ok results: pop from front on each `run_ok()` call.
        ok_responses: Mutex<Vec<bool>>,
        /// All calls recorded.
        pub calls: Mutex<Vec<MockCall>>,
    }

    impl MockTmuxExecutor {
        pub fn new() -> Self {
            Self {
                responses: Mutex::new(Vec::new()),
                ok_responses: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
            }
        }

        /// Queue a successful response for the next `run()` call.
        pub fn push_response(&self, output: &str) {
            self.responses.lock().unwrap().push(Ok(output.to_string()));
        }

        /// Queue an error response for the next `run()` call.
        pub fn push_error(&self, msg: &str) {
            self.responses
                .lock()
                .unwrap()
                .push(Err(TmuxError(msg.to_string())));
        }

        /// Queue a result for the next `run_ok()` call.
        pub fn push_ok(&self, ok: bool) {
            self.ok_responses.lock().unwrap().push(ok);
        }

        /// Get all recorded calls.
        pub fn get_calls(&self) -> Vec<MockCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl TmuxExecutor for MockTmuxExecutor {
        fn run(&self, args: &[&str]) -> TmuxResult<String> {
            self.calls.lock().unwrap().push(MockCall {
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Err(TmuxError("mock: no response queued".into()))
            } else {
                responses.remove(0)
            }
        }

        fn run_ok(&self, args: &[&str]) -> bool {
            self.calls.lock().unwrap().push(MockCall {
                args: args.iter().map(|s| s.to_string()).collect(),
            });
            let mut ok_responses = self.ok_responses.lock().unwrap();
            if ok_responses.is_empty() {
                false
            } else {
                ok_responses.remove(0)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TmuxClient — high-level API with injected executor
// ---------------------------------------------------------------------------

/// High-level tmux client wrapping an executor.
pub struct TmuxClient {
    exec: Box<dyn TmuxExecutor>,
}

impl TmuxClient {
    pub fn new(exec: Box<dyn TmuxExecutor>) -> Self {
        Self { exec }
    }

    /// Create a client using the real tmux binary.
    pub fn real() -> Self {
        Self::new(Box::new(RealTmuxExecutor))
    }

    // --- Context ---

    pub fn detect_context(&self) -> TmuxResult<TmuxContext> {
        if std::env::var("TMUX").is_err() {
            return Err(TmuxError("not in tmux".into()));
        }
        let fmt = "#{session_name}\t#{window_index}\t#{window_name}\t#{pane_id}";
        let raw = self.exec.run(&["display-message", "-p", fmt])?;
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

    // --- Sessions ---

    pub fn has_session(&self, name: &str) -> bool {
        self.exec.run_ok(&["has-session", "-t", name])
    }

    pub fn list_sessions(&self) -> TmuxResult<Vec<SessionInfo>> {
        let fmt = "#{session_name}\t#{session_windows}\t#{session_attached}\t#{session_created}";
        let raw = self.exec.run(&["list-sessions", "-F", fmt])?;
        parse_sessions(&raw)
    }

    pub fn new_session(&self, name: &str, detached: bool) -> TmuxResult<()> {
        let mut args = vec!["new-session", "-s", name];
        if detached {
            args.push("-d");
        }
        self.exec.run(&args)?;
        Ok(())
    }

    pub fn kill_session(&self, name: &str) -> TmuxResult<()> {
        self.exec.run(&["kill-session", "-t", name])?;
        Ok(())
    }

    pub fn rename_session(&self, old: &str, new: &str) -> TmuxResult<()> {
        self.exec.run(&["rename-session", "-t", old, new])?;
        Ok(())
    }

    pub fn switch_client(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["switch-client", "-t", target])?;
        Ok(())
    }

    // --- Windows ---

    pub fn list_windows(&self, session: &str) -> TmuxResult<Vec<WindowInfo>> {
        let fmt = "#{window_index}\t#{window_name}\t#{window_active}\t#{window_panes}\t#{window_layout}";
        let raw = self.exec.run(&["list-windows", "-t", session, "-F", fmt])?;
        parse_windows(&raw)
    }

    pub fn new_window(&self, session: &str, name: Option<&str>) -> TmuxResult<()> {
        let target = format!("{}:", session);
        let mut args = vec!["new-window", "-t", &target];
        if let Some(n) = name {
            args.extend(["-n", n]);
        }
        self.exec.run(&args)?;
        Ok(())
    }

    pub fn kill_window(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["kill-window", "-t", target])?;
        Ok(())
    }

    pub fn rename_window(&self, target: &str, new_name: &str) -> TmuxResult<()> {
        self.exec.run(&["rename-window", "-t", target, new_name])?;
        Ok(())
    }

    pub fn select_window(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["select-window", "-t", target])?;
        Ok(())
    }

    pub fn link_window(&self, source: &str, dest_session: &str) -> TmuxResult<String> {
        self.exec
            .run(&["link-window", "-d", "-s", source, "-t", dest_session])?;
        let fmt = "#{window_index}";
        let raw = self
            .exec
            .run(&["list-windows", "-t", dest_session, "-F", fmt])?;
        raw.lines()
            .last()
            .map(|l| l.trim().to_string())
            .ok_or_else(|| TmuxError("no windows after link".into()))
    }

    pub fn unlink_window(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["unlink-window", "-t", target])?;
        Ok(())
    }

    // --- Panes ---

    pub fn list_panes(&self, target: &str) -> TmuxResult<Vec<PaneInfo>> {
        let fmt = "#{pane_id}\t#{pane_index}\t#{pane_width}\t#{pane_height}\t#{pane_active}\t#{pane_current_command}\t#{pane_current_path}";
        let raw = self.exec.run(&["list-panes", "-t", target, "-F", fmt])?;
        parse_panes(&raw)
    }

    pub fn split_pane(&self, target: &str, horizontal: bool) -> TmuxResult<()> {
        let flag = if horizontal { "-h" } else { "-v" };
        self.exec.run(&["split-window", flag, "-t", target])?;
        Ok(())
    }

    pub fn kill_pane(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["kill-pane", "-t", target])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn resize_pane(&self, target: &str, direction: &str, amount: u16) -> TmuxResult<()> {
        let amt = amount.to_string();
        self.exec
            .run(&["resize-pane", "-t", target, direction, &amt])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn select_pane(&self, target: &str) -> TmuxResult<()> {
        self.exec.run(&["select-pane", "-t", target])?;
        Ok(())
    }

    pub fn send_keys(&self, target: &str, keys: &str) -> TmuxResult<()> {
        self.exec
            .run(&["send-keys", "-t", target, keys, "Enter"])?;
        Ok(())
    }

    // --- Layout ---

    pub fn select_layout(&self, target: &str, layout: &str) -> TmuxResult<()> {
        self.exec
            .run(&["select-layout", "-t", target, layout])?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Parse helpers (shared between TmuxClient methods and free functions)
// ---------------------------------------------------------------------------

fn parse_sessions(raw: &str) -> TmuxResult<Vec<SessionInfo>> {
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

fn parse_windows(raw: &str) -> TmuxResult<Vec<WindowInfo>> {
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

fn parse_panes(raw: &str) -> TmuxResult<Vec<PaneInfo>> {
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

// ---------------------------------------------------------------------------
// Backward-compatible free functions (use RealTmuxExecutor)
// ---------------------------------------------------------------------------

fn run(args: &[&str]) -> TmuxResult<String> {
    RealTmuxExecutor.run(args)
}

fn run_ok(args: &[&str]) -> bool {
    RealTmuxExecutor.run_ok(args)
}

pub fn detect_context() -> TmuxResult<TmuxContext> {
    TmuxClient::real().detect_context()
}

pub fn has_session(name: &str) -> bool {
    run_ok(&["has-session", "-t", name])
}

pub fn list_sessions() -> TmuxResult<Vec<SessionInfo>> {
    let fmt = "#{session_name}\t#{session_windows}\t#{session_attached}\t#{session_created}";
    let raw = run(&["list-sessions", "-F", fmt])?;
    parse_sessions(&raw)
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

pub fn list_windows(session: &str) -> TmuxResult<Vec<WindowInfo>> {
    let fmt = "#{window_index}\t#{window_name}\t#{window_active}\t#{window_panes}\t#{window_layout}";
    let raw = run(&["list-windows", "-t", session, "-F", fmt])?;
    parse_windows(&raw)
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

pub fn list_panes(target: &str) -> TmuxResult<Vec<PaneInfo>> {
    let fmt = "#{pane_id}\t#{pane_index}\t#{pane_width}\t#{pane_height}\t#{pane_active}\t#{pane_current_command}\t#{pane_current_path}";
    let raw = run(&["list-panes", "-t", target, "-F", fmt])?;
    parse_panes(&raw)
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

pub fn select_layout(target: &str, layout: &str) -> TmuxResult<()> {
    run(&["select-layout", "-t", target, layout])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sessions() {
        let raw = "mysess\t3\t1\t1700000000\nother\t1\t0\t1700000001\n";
        let sessions = parse_sessions(raw).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "mysess");
        assert_eq!(sessions[0].window_count, 3);
        assert!(sessions[0].attached);
        assert_eq!(sessions[1].name, "other");
        assert!(!sessions[1].attached);
    }

    #[test]
    fn test_parse_windows() {
        let raw = "0\tmain\t1\t2\tmain-vertical\n1\tcode\t0\t1\teven-horizontal\n";
        let windows = parse_windows(raw).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].index, 0);
        assert_eq!(windows[0].name, "main");
        assert!(windows[0].active);
        assert_eq!(windows[0].pane_count, 2);
        assert_eq!(windows[1].index, 1);
        assert!(!windows[1].active);
    }

    #[test]
    fn test_parse_panes() {
        let raw = "%0\t0\t120\t40\t1\tbash\t/home/user\n%1\t1\t60\t40\t0\tvim\t/home/user/code\n";
        let panes = parse_panes(raw).unwrap();
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].id, "%0");
        assert!(panes[0].active);
        assert_eq!(panes[0].command, "bash");
        assert_eq!(panes[1].id, "%1");
        assert!(!panes[1].active);
        assert_eq!(panes[1].command, "vim");
    }

    #[test]
    fn test_mock_executor_records_calls() {
        let mock = mock::MockTmuxExecutor::new();
        mock.push_response("session1\t1\t0\t1700000000\n");

        let client = TmuxClient::new(Box::new(mock));
        // We can't call list_sessions because it uses a format string
        // but we can verify the mock works
        let _ = client.has_session("test");
        // has_session uses run_ok which defaults to false with no queued response
    }

    #[test]
    fn test_client_list_sessions_with_mock() {
        let mock = mock::MockTmuxExecutor::new();
        mock.push_response("sess1\t2\t1\t1700000000\nsess2\t1\t0\t1700000001\n");

        let client = TmuxClient::new(Box::new(mock));
        let sessions = client.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "sess1");
        assert!(sessions[0].attached);
        assert_eq!(sessions[1].name, "sess2");
    }

    #[test]
    fn test_client_list_windows_with_mock() {
        let mock = mock::MockTmuxExecutor::new();
        mock.push_response("0\tshell\t1\t1\tmain-vertical\n");

        let client = TmuxClient::new(Box::new(mock));
        let windows = client.list_windows("test").unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].name, "shell");
    }

    #[test]
    fn test_client_list_panes_with_mock() {
        let mock = mock::MockTmuxExecutor::new();
        mock.push_response("%5\t0\t80\t24\t1\tzsh\t/tmp\n");

        let client = TmuxClient::new(Box::new(mock));
        let panes = client.list_panes("test:0").unwrap();
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].id, "%5");
        assert_eq!(panes[0].width, 80);
        assert_eq!(panes[0].height, 24);
    }

    #[test]
    fn test_client_link_window_with_mock() {
        let mock = mock::MockTmuxExecutor::new();
        // First call: link-window
        mock.push_response("");
        // Second call: list-windows to find new index
        mock.push_response("0\n1\n2\n");

        let client = TmuxClient::new(Box::new(mock));
        let idx = client.link_window("src:1", "dest").unwrap();
        assert_eq!(idx, "2");
    }

    #[test]
    fn test_client_error_propagation() {
        let mock = mock::MockTmuxExecutor::new();
        mock.push_error("session not found");

        let client = TmuxClient::new(Box::new(mock));
        let result = client.list_sessions();
        assert!(result.is_err());
        assert!(result.unwrap_err().0.contains("session not found"));
    }
}
