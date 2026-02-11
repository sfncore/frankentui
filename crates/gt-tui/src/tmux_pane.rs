use std::process::Command;

/// Whether we're running inside tmux.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxMode {
    NoTmux,
    Active,
}

/// Our position in the tmux hierarchy.
#[derive(Debug, Clone, Default)]
pub struct TmuxContext {
    pub session_name: String,
    pub window_index: String,
    pub window_name: String,
    pub pane_id: String,
}

/// Links agent tmux sessions as windows in our session.
///
/// Three interaction tiers:
/// - **peek**: temp link-window that gets replaced as cursor moves
/// - **link**: permanent link that stays in the window list
/// - **switch**: switch-client to jump into the agent's session entirely
pub struct TmuxPaneControl {
    pub mode: TmuxMode,
    pub context: TmuxContext,
    /// Temp peek: (window_index, session_name). Replaced on next peek.
    peek: Option<(String, String)>,
    /// Permanently linked: window_index → session_name.
    linked: Vec<(String, String)>,
    /// Last session we interacted with.
    active_session: Option<String>,
}

impl TmuxPaneControl {
    pub fn new() -> Self {
        let mut ctrl = Self {
            mode: TmuxMode::NoTmux,
            context: TmuxContext::default(),
            peek: None,
            linked: Vec::new(),
            active_session: None,
        };
        ctrl.scan();
        ctrl
    }

    /// Detect tmux context.
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

    // ----- Peek: temp link that auto-replaces on cursor move -----

    /// Temporarily link an agent's window. Unlinks the previous peek first.
    /// Stays on the TUI window — the linked window just appears in the
    /// status bar so the user knows it's available.
    pub fn peek_session(&mut self, session: &str) -> ActivateResult {
        if self.mode == TmuxMode::NoTmux {
            return ActivateResult::NoTmux;
        }
        if session == self.context.session_name {
            return ActivateResult::SameSession;
        }

        // Already peeking this session
        if let Some((_, ref s)) = self.peek {
            if s == session {
                return ActivateResult::AlreadyPeeked;
            }
        }

        // Skip if already permanently linked
        if self.linked.iter().any(|(_, s)| s == session) {
            return ActivateResult::AlreadyLinked;
        }

        // Unlink previous peek
        self.unlink_peek();

        // Link the new one
        match self.do_link_window(session) {
            Some(idx) => {
                self.peek = Some((idx, session.to_string()));
                self.active_session = Some(session.to_string());
                ActivateResult::Peeked
            }
            None => ActivateResult::SessionNotFound,
        }
    }

    /// Remove the current peek link (if any).
    fn unlink_peek(&mut self) {
        if let Some((idx, _)) = self.peek.take() {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = Command::new("tmux")
                .args(["unlink-window", "-t", &target])
                .status();
        }
    }

    /// The session currently being peeked (if any).
    pub fn peek_session_name(&self) -> Option<&str> {
        self.peek.as_ref().map(|(_, s)| s.as_str())
    }

    // ----- Link: permanent, stays in window list -----

    /// Permanently link an agent's window. If currently peeking this session,
    /// promotes the peek to a permanent link.
    pub fn link_session(&mut self, session: &str) -> ActivateResult {
        if self.mode == TmuxMode::NoTmux {
            return ActivateResult::NoTmux;
        }
        if session == self.context.session_name {
            return ActivateResult::SameSession;
        }

        // Already permanently linked — just select it
        if let Some((idx, _)) = self.linked.iter().find(|(_, s)| s == session) {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = Command::new("tmux")
                .args(["select-window", "-t", &target])
                .status();
            self.active_session = Some(session.to_string());
            return ActivateResult::AlreadyLinked;
        }

        // Promote from peek if peeking this session
        if let Some((idx, s)) = &self.peek {
            if s == session {
                let idx_owned = idx.clone();
                let session_owned = session.to_string();
                self.peek = None;
                let target = format!("{}:{}", self.context.session_name, idx_owned);
                let _ = Command::new("tmux")
                    .args(["select-window", "-t", &target])
                    .status();
                self.linked.push((idx_owned, session_owned));
                self.active_session = Some(session.to_string());
                return ActivateResult::Linked;
            }
        }

        // Fresh link
        self.unlink_peek();
        match self.do_link_window(session) {
            Some(idx) => {
                let target = format!("{}:{}", self.context.session_name, idx);
                let _ = Command::new("tmux")
                    .args(["select-window", "-t", &target])
                    .status();
                self.linked.push((idx, session.to_string()));
                self.active_session = Some(session.to_string());
                ActivateResult::Linked
            }
            None => ActivateResult::SessionNotFound,
        }
    }

    // ----- Switch: leave the TUI entirely -----

    /// Switch the tmux client to the agent's session.
    pub fn switch_session(&mut self, session: &str) -> ActivateResult {
        if self.mode == TmuxMode::NoTmux {
            return ActivateResult::NoTmux;
        }
        if session == self.context.session_name {
            return ActivateResult::SameSession;
        }

        // Clean up peek before switching
        self.unlink_peek();

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

    // ----- Shared helpers -----

    /// Link a window and return its new index. Does not select it.
    fn do_link_window(&self, session: &str) -> Option<String> {
        // Verify source exists
        let has = Command::new("tmux")
            .args(["has-session", "-t", session])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !has {
            return None;
        }

        let source = format!("{}:1", session);
        let ok = Command::new("tmux")
            .args(["link-window", "-d", "-s", &source, "-t", &self.context.session_name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return None;
        }

        // Find the new window index (highest)
        let idx = tmux_cmd(&[
            "list-windows", "-t", &self.context.session_name,
            "-F", "#{window_index}",
        ])
        .and_then(|out| out.lines().last().map(|l| l.trim().to_string()))?;

        // Rename to the session name
        let target = format!("{}:{}", self.context.session_name, idx);
        let _ = Command::new("tmux")
            .args(["rename-window", "-t", &target, session])
            .status();

        Some(idx)
    }

    /// Unlink all windows we manage (peek + permanent). Call on exit.
    pub fn unlink_all(&mut self) {
        self.unlink_peek();
        while let Some((idx, _)) = self.linked.pop() {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = Command::new("tmux")
                .args(["unlink-window", "-t", &target])
                .status();
        }
    }

    pub fn active_session_name(&self) -> Option<&str> {
        self.active_session.as_deref()
    }

    pub fn in_tmux(&self) -> bool {
        self.mode == TmuxMode::Active
    }

    pub fn linked_count(&self) -> usize {
        self.linked.len()
    }

    pub fn is_linked(&self, session: &str) -> bool {
        self.linked.iter().any(|(_, s)| s == session)
    }
}

/// Result of a tmux operation.
pub enum ActivateResult {
    /// Peek: temp-linked, appears in status bar.
    Peeked,
    /// Already peeking this session.
    AlreadyPeeked,
    /// Permanently linked into our session.
    Linked,
    /// Already permanently linked.
    AlreadyLinked,
    /// Switched client to the session.
    Switched,
    /// Not running inside tmux.
    NoTmux,
    /// Target is our own session.
    SameSession,
    /// Target session doesn't exist.
    SessionNotFound,
}

fn tmux_cmd(args: &[&str]) -> Option<String> {
    let output = Command::new("tmux").args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}
