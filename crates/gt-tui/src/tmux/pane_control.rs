//! Migrated TmuxPaneControl — same public API as the original tmux_pane.rs,
//! but internals rewritten to call `client::*` functions.

use super::client;

/// Whether we're running inside tmux.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxMode {
    NoTmux,
    Active,
}

/// Our position in the tmux hierarchy.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
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
    /// Permanently linked: window_index -> session_name.
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
        match client::detect_context() {
            Ok(ctx) => {
                self.context = ctx;
                self.mode = TmuxMode::Active;
            }
            Err(_) => {
                self.mode = TmuxMode::NoTmux;
                self.context = TmuxContext::default();
            }
        }
    }

    // ----- Peek: temp link that auto-replaces on cursor move -----

    /// Temporarily link an agent's window. Unlinks the previous peek first.
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
        if !client::has_session(session) {
            return ActivateResult::SessionNotFound;
        }

        let source = format!("{}:1", session);
        match client::link_window(&source, &self.context.session_name) {
            Ok(idx) => {
                // Rename to the session name
                let target = format!("{}:{}", self.context.session_name, idx);
                let _ = client::rename_window(&target, session);
                self.peek = Some((idx, session.to_string()));
                self.active_session = Some(session.to_string());
                ActivateResult::Peeked
            }
            Err(_) => ActivateResult::SessionNotFound,
        }
    }

    /// Remove the current peek link (if any).
    fn unlink_peek(&mut self) {
        if let Some((idx, _)) = self.peek.take() {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = client::unlink_window(&target);
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

        // Already permanently linked -- just select it
        if let Some((idx, _)) = self.linked.iter().find(|(_, s)| s == session) {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = client::select_window(&target);
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
                let _ = client::select_window(&target);
                self.linked.push((idx_owned, session_owned));
                self.active_session = Some(session.to_string());
                return ActivateResult::Linked;
            }
        }

        // Fresh link
        self.unlink_peek();
        if !client::has_session(session) {
            return ActivateResult::SessionNotFound;
        }

        let source = format!("{}:1", session);
        match client::link_window(&source, &self.context.session_name) {
            Ok(idx) => {
                let target = format!("{}:{}", self.context.session_name, idx);
                let _ = client::select_window(&target);
                let _ = client::rename_window(&target, session);
                self.linked.push((idx, session.to_string()));
                self.active_session = Some(session.to_string());
                ActivateResult::Linked
            }
            Err(_) => ActivateResult::SessionNotFound,
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

        match client::switch_client(session) {
            Ok(()) => {
                self.active_session = Some(session.to_string());
                ActivateResult::Switched
            }
            Err(_) => ActivateResult::SessionNotFound,
        }
    }

    // ----- Cleanup -----

    /// Unlink all windows we manage (peek + permanent). Call on exit.
    pub fn unlink_all(&mut self) {
        self.unlink_peek();
        while let Some((idx, _)) = self.linked.pop() {
            let target = format!("{}:{}", self.context.session_name, idx);
            let _ = client::unlink_window(&target);
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
    Peeked,
    AlreadyPeeked,
    Linked,
    AlreadyLinked,
    Switched,
    NoTmux,
    SameSession,
    SessionNotFound,
}
