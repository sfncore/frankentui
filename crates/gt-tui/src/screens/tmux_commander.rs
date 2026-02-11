//! F8 — Tmux Commander screen.
//!
//! Full tmux control with a session/window/pane tree view and
//! context-sensitive actions.

use ftui_core::event::{KeyCode, KeyEvent, MouseEvent};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::{Line, Span, Text};
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::tree::{Tree, TreeGuides};
use ftui_widgets::Widget;

use crate::msg::Msg;
use crate::tmux::actions;
use crate::tmux::model::{TmuxNodeKind, TmuxSnapshot};

// ---------------------------------------------------------------------------
// Focus & input modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Detail,
    SendKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    SendKeys,
    Rename,
    NewSession,
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct TmuxCommanderScreen {
    pub snapshot: TmuxSnapshot,
    tree_cursor: usize,
    focus: Focus,
    input_mode: InputMode,
    input_buf: String,
    /// Two-press confirm: (action_label, tick_when_started)
    pending_confirm: Option<(String, u64)>,
    /// Feedback message with TTL
    feedback: Option<(String, u64)>,
    tick_count: u64,
}

impl TmuxCommanderScreen {
    pub fn new() -> Self {
        Self {
            snapshot: TmuxSnapshot::default(),
            tree_cursor: 0,
            focus: Focus::Tree,
            input_mode: InputMode::None,
            input_buf: String::new(),
            pending_confirm: None,
            feedback: None,
            tick_count: 0,
        }
    }

    pub fn set_snapshot(&mut self, snapshot: TmuxSnapshot) {
        self.snapshot = snapshot;
        // Clamp cursor to valid range
        let total = self.total_visible_rows();
        if total > 0 && self.tree_cursor >= total {
            self.tree_cursor = total - 1;
        }
    }

    fn total_visible_rows(&self) -> usize {
        // Root + all sessions + their windows + their panes
        let mut count = 1usize; // root node
        for sess in &self.snapshot.sessions {
            count += 1; // session node
            for win in &sess.windows {
                count += 1; // window node
                count += win.panes.len(); // pane nodes
            }
        }
        count
    }

    fn selected_node(&self) -> Option<TmuxNodeKind> {
        self.snapshot.node_at_index(self.tree_cursor)
    }

    fn set_feedback(&mut self, msg: impl Into<String>) {
        self.feedback = Some((msg.into(), self.tick_count + 30));
    }

    /// Returns true if this screen consumes text input.
    pub fn consumes_text_input(&self) -> bool {
        self.input_mode != InputMode::None
    }

    // ----- Key handling -----

    pub fn handle_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        // Input mode keys
        match self.input_mode {
            InputMode::SendKeys => return self.handle_send_keys_input(key),
            InputMode::Rename => return self.handle_rename_input(key),
            InputMode::NewSession => return self.handle_new_session_input(key),
            InputMode::None => {}
        }

        match key.code {
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                let max = self.total_visible_rows().saturating_sub(1);
                if self.tree_cursor < max {
                    self.tree_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_cursor = self.tree_cursor.saturating_sub(1);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.tree_cursor = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.tree_cursor = self.total_visible_rows().saturating_sub(1);
            }

            // Focus cycling
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Detail,
                    Focus::Detail => Focus::Tree,
                    Focus::SendKeys => Focus::Tree,
                };
            }

            // Context-sensitive actions
            KeyCode::Char('N') => {
                // New session
                self.input_mode = InputMode::NewSession;
                self.input_buf.clear();
                self.focus = Focus::SendKeys;
            }
            _ => {
                return self.handle_node_action(key);
            }
        }
        Cmd::None
    }

    fn handle_node_action(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        let node = match self.selected_node() {
            Some(n) => n,
            None => return Cmd::None,
        };

        match node {
            TmuxNodeKind::Session(name) => match key.code {
                KeyCode::Char('n') => {
                    return actions::new_window_cmd(name, None);
                }
                KeyCode::Char('r') => {
                    self.input_mode = InputMode::Rename;
                    self.input_buf.clear();
                    self.focus = Focus::SendKeys;
                }
                KeyCode::Char('k') => {
                    return self.two_press_confirm(&format!("kill-session {name}"), move || {
                        actions::kill_session_cmd(name)
                    });
                }
                KeyCode::Char('s') | KeyCode::Enter => {
                    return actions::switch_client_cmd(name);
                }
                KeyCode::Char('L') => {
                    let target = format!("{name}:");
                    return actions::select_layout_cmd(target, "tiled".to_string());
                }
                _ => {}
            },
            TmuxNodeKind::Window(session, idx) => {
                let target = format!("{session}:{idx}");
                match key.code {
                    KeyCode::Char('n') => {
                        return actions::split_pane_cmd(target, true);
                    }
                    KeyCode::Char('v') => {
                        return actions::split_pane_cmd(target, false);
                    }
                    KeyCode::Char('r') => {
                        self.input_mode = InputMode::Rename;
                        self.input_buf.clear();
                        self.focus = Focus::SendKeys;
                    }
                    KeyCode::Char('k') => {
                        return self.two_press_confirm(
                            &format!("kill-window {target}"),
                            move || actions::kill_window_cmd(target),
                        );
                    }
                    KeyCode::Enter => {
                        return actions::switch_client_cmd(session);
                    }
                    KeyCode::Char('L') => {
                        return actions::select_layout_cmd(target, "tiled".to_string());
                    }
                    _ => {}
                }
            }
            TmuxNodeKind::Pane(session, _idx, pane_id) => {
                let target = pane_id.clone();
                match key.code {
                    KeyCode::Enter => {
                        let sess = session;
                        return actions::switch_client_cmd(sess);
                    }
                    KeyCode::Char('k') => {
                        return self.two_press_confirm(
                            &format!("kill-pane {target}"),
                            move || actions::kill_pane_cmd(target),
                        );
                    }
                    KeyCode::Tab => {
                        self.input_mode = InputMode::SendKeys;
                        self.input_buf.clear();
                        self.focus = Focus::SendKeys;
                    }
                    _ => {}
                }
            }
        }
        Cmd::None
    }

    fn two_press_confirm(&mut self, label: &str, action: impl FnOnce() -> Cmd<Msg>) -> Cmd<Msg> {
        if let Some((ref pending, tick)) = self.pending_confirm {
            if pending == label && self.tick_count.saturating_sub(tick) < 30 {
                self.pending_confirm = None;
                return action();
            }
        }
        self.pending_confirm = Some((label.to_string(), self.tick_count));
        self.set_feedback(format!("press k again to confirm: {label}"));
        Cmd::None
    }

    fn handle_send_keys_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => {
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
            }
            KeyCode::Enter => {
                if let Some(TmuxNodeKind::Pane(_, _, ref pane_id)) = self.selected_node() {
                    let target = pane_id.clone();
                    let keys = self.input_buf.clone();
                    self.input_buf.clear();
                    self.input_mode = InputMode::None;
                    self.focus = Focus::Tree;
                    return actions::send_keys_cmd(target, keys);
                }
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_rename_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => {
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
            }
            KeyCode::Enter => {
                let new_name = self.input_buf.clone();
                self.input_buf.clear();
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
                if new_name.is_empty() {
                    return Cmd::None;
                }
                if let Some(node) = self.selected_node() {
                    return match node {
                        TmuxNodeKind::Session(old) => {
                            actions::rename_session_cmd(old, new_name)
                        }
                        TmuxNodeKind::Window(session, idx) => {
                            let target = format!("{session}:{idx}");
                            actions::rename_window_cmd(target, new_name)
                        }
                        _ => Cmd::None,
                    };
                }
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_new_session_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => {
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
            }
            KeyCode::Enter => {
                let name = self.input_buf.clone();
                self.input_buf.clear();
                self.input_mode = InputMode::None;
                self.focus = Focus::Tree;
                if !name.is_empty() {
                    return actions::create_session(name);
                }
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn handle_mouse(&mut self, _mouse: &MouseEvent) -> Cmd<Msg> {
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        if let Some((_, ttl)) = &self.feedback {
            if tick_count > *ttl {
                self.feedback = None;
            }
        }
    }

    // ----- Rendering -----

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        // Two-column layout: tree (40%) + detail (60%)
        let has_input = self.input_mode != InputMode::None;
        let constraints = if has_input {
            vec![
                Constraint::Min(6),   // content
                Constraint::Fixed(3), // input bar
            ]
        } else {
            vec![Constraint::Min(6)]
        };

        let outer = Flex::vertical().constraints(constraints).split(area);

        let columns = Flex::horizontal()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(60.0),
            ])
            .split(outer[0]);

        self.render_tree(frame, columns[0]);
        self.render_detail(frame, columns[1]);

        if has_input && outer.len() > 1 {
            self.render_input_bar(frame, outer[1]);
        }
    }

    fn render_tree(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == Focus::Tree;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        let block = Block::new()
            .title(" Tmux Commander ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(crate::theme::panel_bg())
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.width < 4 || inner.height < 1 {
            return;
        }

        if self.snapshot.sessions.is_empty() {
            Paragraph::new("No tmux sessions found.\nPress N to create one.")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
            return;
        }

        // Build tree and render with cursor highlight
        let tree_root = self.snapshot.to_tree_node();
        let tree = Tree::new(tree_root)
            .with_guides(TreeGuides::Rounded)
            .with_guide_style(Style::new().fg(theme::fg::MUTED))
            .with_label_style(Style::new().fg(theme::fg::PRIMARY))
            .with_root_style(Style::new().fg(theme::accent::INFO).bold())
            .with_show_root(true);

        // We render the tree, then overlay the cursor highlight
        tree.render(inner, frame);

        // Highlight cursor row
        if self.tree_cursor < inner.height as usize {
            let y = inner.y + self.tree_cursor as u16;
            for x in inner.x..inner.right() {
                if let Some(cell) = frame.buffer.get_mut(x, y) {
                    cell.bg = theme::accent::PRIMARY.into();
                    cell.fg = theme::bg::DEEP.into();
                }
            }
        }
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == Focus::Detail;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        let block = Block::new()
            .title(" Detail ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(crate::theme::panel_bg())
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.width < 4 || inner.height < 2 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        match self.selected_node() {
            Some(TmuxNodeKind::Session(ref name)) => {
                lines.push(Line::from_spans([
                    Span::styled("Session: ", Style::new().fg(theme::fg::MUTED)),
                    Span::styled(name, Style::new().fg(theme::accent::INFO).bold()),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Actions:",
                    Style::new().fg(theme::fg::SECONDARY),
                ));
                lines.push(action_hint("n", "new window"));
                lines.push(action_hint("r", "rename"));
                lines.push(action_hint("k", "kill (confirm)"));
                lines.push(action_hint("s/Enter", "switch client"));
                lines.push(action_hint("L", "set layout"));
            }
            Some(TmuxNodeKind::Window(ref session, idx)) => {
                lines.push(Line::from_spans([
                    Span::styled("Window: ", Style::new().fg(theme::fg::MUTED)),
                    Span::styled(
                        format!("{session}:{idx}"),
                        Style::new().fg(theme::accent::INFO).bold(),
                    ),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Actions:",
                    Style::new().fg(theme::fg::SECONDARY),
                ));
                lines.push(action_hint("n", "split horizontal"));
                lines.push(action_hint("v", "split vertical"));
                lines.push(action_hint("r", "rename"));
                lines.push(action_hint("k", "kill (confirm)"));
                lines.push(action_hint("Enter", "switch client"));
                lines.push(action_hint("L", "set layout"));
            }
            Some(TmuxNodeKind::Pane(ref _session, _idx, ref pane_id)) => {
                lines.push(Line::from_spans([
                    Span::styled("Pane: ", Style::new().fg(theme::fg::MUTED)),
                    Span::styled(pane_id, Style::new().fg(theme::accent::INFO).bold()),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Actions:",
                    Style::new().fg(theme::fg::SECONDARY),
                ));
                lines.push(action_hint("Enter", "switch client"));
                lines.push(action_hint("k", "kill (confirm)"));
                lines.push(action_hint("Tab", "send keys"));
            }
            None => {
                lines.push(Line::styled(
                    "Select a node to see details",
                    Style::new().fg(theme::fg::DISABLED),
                ));
                lines.push(Line::raw(""));
                lines.push(action_hint("N", "new session"));
                lines.push(action_hint("j/k", "navigate"));
            }
        }

        // Feedback
        if let Some((ref msg, _)) = self.feedback {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                msg.as_str(),
                Style::new().fg(theme::accent::WARNING),
            ));
        }

        // Confirm hint
        if let Some((ref label, _)) = self.pending_confirm {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("Press k again to confirm: {label}"),
                Style::new().fg(theme::accent::ERROR).bold(),
            ));
        }

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        let title = match self.input_mode {
            InputMode::SendKeys => " Send Keys: ",
            InputMode::Rename => " Rename: ",
            InputMode::NewSession => " New Session Name: ",
            InputMode::None => return,
        };

        let block = Block::new()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::new().fg(theme::accent::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        let display = format!("{}_", self.input_buf);
        Paragraph::new(display.as_str())
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }
}

fn action_hint(key: &str, desc: &str) -> Line {
    Line::from_spans([
        Span::styled(
            format!("  [{key}] "),
            Style::new().fg(theme::accent::PRIMARY).bold(),
        ),
        Span::styled(desc, Style::new().fg(theme::fg::SECONDARY)),
    ])
}
