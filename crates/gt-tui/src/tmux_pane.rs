use std::process::Command;

/// Whether we're running inside tmux.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxMode {
    /// Not running inside tmux at all.
    NoTmux,
    /// Inside tmux — can use switch-client.
    Active,
}

/// Our position in the tmux hierarchy, detected during scan.
#[derive(Debug, Clone, Default)]
pub struct TmuxContext {
    pub session_name: String,
    pub window_index: String,
    pub window_name: String,
    pub pane_id: String,
}

/// Switches the tmux client to view agent sessions via switch-client.
pub struct TmuxPaneControl {
    pub mode: TmuxMode,
    pub context: TmuxContext,
    /// The session we last switched to (so we can show "last viewed").
    active_session: Option<String>,
}

impl TmuxPaneControl {
    pub fn new() -> Self {
        let mut ctrl = Self {
            mode: TmuxMode::NoTmux,
            context: TmuxContext::default(),
            active_session: None,
        };
        ctrl.scan();
        ctrl
    }

    /// Detect tmux context. Called at init and on status refreshes.
    pub fn scan(&mut self) {
        if std::env::var("TMUX").is_err() {
            self.mode = TmuxMode::NoTmux;
            self.context = TmuxContext::default();
            return;
        }

        let ctx_fmt = "#{session_name}\t#{window_index}\t#{window_name}\t#{pane_id}";
        let ctx_raw = match tmux_cmd(&["display-message", "-p", ctx_fmt]) {
            Some(s) => s,
            None => {
                self.mode = TmuxMode::NoTmux;
                return;
            }
        };
        let parts: Vec<&str> = ctx_raw.trim().split('\t').collect();
        if parts.len() < 4 {
            self.mode = TmuxMode::NoTmux;
            return;
        }
        self.context.session_name = parts[0].to_string();
        self.context.window_index = parts[1].to_string();
        self.context.window_name = parts[2].to_string();
        self.context.pane_id = parts[3].to_string();
        self.mode = TmuxMode::Active;
    }

    /// Switch the tmux client to view the given session.
    /// The user can return with `switch-client -l` (last session).
    pub fn activate_session(&mut self, session: &str) -> ActivateResult {
        if self.mode == TmuxMode::NoTmux {
            return ActivateResult::NoTmux;
        }

        // Don't switch to our own session
        if session == self.context.session_name {
            return ActivateResult::SameSession;
        }

        let ok = Command::new("tmux")
            .args(["switch-client", "-t", session])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if ok {
            self.active_session = Some(session.to_string());
            ActivateResult::Switched
        } else {
            ActivateResult::SessionNotFound
        }
    }

    /// Return the name of the session we last switched to.
    pub fn active_session_name(&self) -> Option<&str> {
        self.active_session.as_deref()
    }

    /// Is tmux available?
    pub fn in_tmux(&self) -> bool {
        self.mode == TmuxMode::Active
    }
}

/// Result of an activate_session call.
pub enum ActivateResult {
    /// Client switched to the requested session.
    Switched,
    /// Not running inside tmux.
    NoTmux,
    /// Target session is our own session.
    SameSession,
    /// Target session doesn't exist.
    SessionNotFound,
}

/// Run a tmux command and return stdout.
fn tmux_cmd(args: &[&str]) -> Option<String> {
    let output = Command::new("tmux").args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}
