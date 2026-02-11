use std::process::Command;

/// How we interact with the adjacent tmux pane (if any).
#[derive(Debug, Clone)]
pub enum PaneMode {
    /// Not running inside tmux at all.
    NoTmux,
    /// Single pane in the window — use popup fallback.
    Solo,
    /// Adjacent pane detected — we can respawn it to show agent sessions.
    Adjacent(String),
}

/// Our position in the tmux hierarchy, detected during scan.
#[derive(Debug, Clone, Default)]
pub struct TmuxContext {
    pub session_name: String,
    pub window_index: String,
    pub window_name: String,
    pub pane_id: String,
    pub pane_count: usize,
}

/// Info about what's running in the adjacent pane.
#[derive(Debug, Clone, Default)]
pub struct AdjacentPaneInfo {
    /// The command currently running (e.g. "bash", "claude", "python").
    pub command: String,
    /// The pane title (often shows the running program).
    pub title: String,
}

/// Detects and controls an adjacent tmux pane for displaying agent sessions.
pub struct TmuxPaneControl {
    pub mode: PaneMode,
    pub context: TmuxContext,
    /// What's running in the adjacent pane (if any).
    pub adjacent_info: Option<AdjacentPaneInfo>,
    /// The tmux session last opened via popup.
    active_session: Option<String>,
}

impl TmuxPaneControl {
    /// Create a new controller and immediately scan the tmux environment.
    pub fn new() -> Self {
        let mut ctrl = Self {
            mode: PaneMode::NoTmux,
            context: TmuxContext::default(),
            adjacent_info: None,
            active_session: None,
        };
        ctrl.scan();
        ctrl
    }

    /// Re-detect pane layout and context. Called at init and when status refreshes.
    pub fn scan(&mut self) {
        self.adjacent_info = None;

        // Step 1: Are we inside tmux?
        if std::env::var("TMUX").is_err() {
            self.mode = PaneMode::NoTmux;
            self.context = TmuxContext::default();
            return;
        }

        // Step 2: Get our full context in one call
        let ctx_fmt = "#{session_name}\t#{window_index}\t#{window_name}\t#{pane_id}";
        let ctx_raw = match tmux_cmd(&["display-message", "-p", ctx_fmt]) {
            Some(s) => s,
            None => {
                self.mode = PaneMode::NoTmux;
                return;
            }
        };
        let parts: Vec<&str> = ctx_raw.trim().split('\t').collect();
        if parts.len() < 4 {
            self.mode = PaneMode::NoTmux;
            return;
        }
        self.context.session_name = parts[0].to_string();
        self.context.window_index = parts[1].to_string();
        self.context.window_name = parts[2].to_string();
        self.context.pane_id = parts[3].to_string();

        // Step 3: List all panes in our window
        let pane_list = match tmux_cmd(&["list-panes", "-F", "#{pane_id}"]) {
            Some(output) => output,
            None => {
                self.mode = PaneMode::Solo;
                self.context.pane_count = 1;
                return;
            }
        };

        let panes: Vec<&str> = pane_list.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        self.context.pane_count = panes.len();

        if panes.len() <= 1 {
            self.mode = PaneMode::Solo;
            return;
        }

        // Step 4: Find the adjacent pane (prefer next pane after ours)
        let our_idx = panes.iter().position(|p| *p == self.context.pane_id);
        let adjacent = match our_idx {
            Some(idx) => {
                let next = (idx + 1) % panes.len();
                panes[next].to_string()
            }
            None => {
                match panes.iter().find(|p| **p != self.context.pane_id) {
                    Some(p) => p.to_string(),
                    None => {
                        self.mode = PaneMode::Solo;
                        return;
                    }
                }
            }
        };

        // Step 5: Query what's running in the adjacent pane
        let info_fmt = "#{pane_current_command}\t#{pane_title}";
        self.adjacent_info = tmux_cmd(&["display-message", "-p", "-t", &adjacent, info_fmt])
            .map(|raw| {
                let fields: Vec<&str> = raw.trim().split('\t').collect();
                AdjacentPaneInfo {
                    command: fields.first().unwrap_or(&"").to_string(),
                    title: fields.get(1).unwrap_or(&"").to_string(),
                }
            });

        self.mode = PaneMode::Adjacent(adjacent);
    }

    /// Switch the adjacent pane to show the given tmux session.
    /// Does nothing if no adjacent pane exists.
    pub fn activate_session(&mut self, session: &str) -> ActivateResult {
        // Skip if already showing this session
        if self.active_session.as_deref() == Some(session) {
            return ActivateResult::AlreadyActive;
        }

        // Don't attach to our own session — causes recursive nested view
        if session == self.context.session_name {
            return ActivateResult::SameSession;
        }

        match &self.mode {
            PaneMode::Adjacent(pane_id) => {
                // Unset TMUX so the nested attach doesn't get rejected as a
                // "sessions should be nested with care" error.  Fall back to
                // a plain shell if the target session doesn't exist so the
                // pane stays alive instead of dying and collapsing the layout.
                let attach_cmd = format!(
                    "TMUX='' tmux attach-session -t {} || exec $SHELL",
                    session
                );
                let ok = Command::new("tmux")
                    .args(["respawn-pane", "-k", "-t", pane_id, &attach_cmd])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if ok {
                    self.active_session = Some(session.to_string());
                    ActivateResult::Switched
                } else {
                    ActivateResult::NoPane
                }
            }
            PaneMode::Solo | PaneMode::NoTmux => ActivateResult::NoPane,
        }
    }

    /// Return the name of the session currently showing in the adjacent pane (if any).
    pub fn active_session_name(&self) -> Option<&str> {
        self.active_session.as_deref()
    }

}

/// Result of an activate_session call.
pub enum ActivateResult {
    /// Adjacent pane switched to the requested session.
    Switched,
    /// Already showing the requested session.
    AlreadyActive,
    /// No adjacent pane available — nothing happened.
    NoPane,
    /// Target session is our own session — would cause recursive view.
    SameSession,
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
