//! Agent Detail screen — shows agent info from `gt status --json`.

use std::cell::Cell;

use ftui_core::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
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
use ftui_widgets::Widget;

use crate::data::AgentInfo;
use crate::msg::Msg;
use crate::tmux_pane::{ActivateResult, TmuxPaneControl};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn state_color(state: &str) -> theme::ColorToken {
    match state.to_lowercase().as_str() {
        "running" => theme::accent::SUCCESS,
        "idle" | "waiting" => theme::fg::MUTED,
        "blocked" => theme::accent::WARNING,
        "done" | "complete" => theme::accent::INFO,
        _ => theme::fg::SECONDARY,
    }
}

const ACTION_LABELS: [(&str, &str); 4] = [
    ("n", "nudge"),
    ("m", "mail"),
    ("p", "peek"),
    ("a", "attach"),
];

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct AgentDetailScreen {
    selected_agent: usize,
    focused_action: usize,
    tick_count: u64,
    last_action: Option<String>,
    action_feedback_ttl: u64,
    layout_actions: Cell<Rect>,
    pub tmux_pane: TmuxPaneControl,
}

impl AgentDetailScreen {
    pub fn new() -> Self {
        Self {
            selected_agent: 0,
            focused_action: 0,
            tick_count: 0,
            last_action: None,
            action_feedback_ttl: 0,
            layout_actions: Cell::new(Rect::default()),
            tmux_pane: TmuxPaneControl::new(),
        }
    }

    /// Re-scan tmux pane layout (called on status refresh).
    pub fn rescan_tmux(&mut self) {
        self.tmux_pane.scan();
    }

    fn execute_action(&mut self, agents: &[AgentInfo]) {
        if agents.is_empty() {
            return;
        }
        let idx = self.selected_agent.min(agents.len() - 1);
        let agent = &agents[idx];
        let action = ACTION_LABELS[self.focused_action].1;

        match action {
            "attach" => self.attach_agent(agent),
            _ => {
                self.last_action = Some(format!("{} -> {}", action, agent.name));
                self.action_feedback_ttl = 30;
            }
        }
    }

    fn attach_agent(&mut self, agent: &AgentInfo) {
        if !agent.running || agent.session.is_empty() {
            self.last_action = Some(format!("{} is offline", agent.name));
            self.action_feedback_ttl = 30;
            return;
        }
        match self.tmux_pane.link_session(&agent.session) {
            ActivateResult::Linked | ActivateResult::AlreadyLinked => {
                self.last_action = Some(format!("linked: {}", agent.session));
                self.action_feedback_ttl = 40;
            }
            ActivateResult::SameSession => {
                self.last_action = Some(format!("{} (this session)", agent.session));
                self.action_feedback_ttl = 20;
            }
            ActivateResult::NoTmux => {
                self.last_action = Some("not in tmux".to_string());
                self.action_feedback_ttl = 30;
            }
            ActivateResult::SessionNotFound => {
                self.last_action = Some(format!("{} not found", agent.session));
                self.action_feedback_ttl = 30;
            }
            _ => {}
        }
    }

    // --- Rendering ---

    fn render_agent_selector(
        &self,
        frame: &mut Frame,
        area: Rect,
        agents: &[AgentInfo],
    ) {
        if area.width < 4 || area.height < 1 {
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            " Agent: ",
            Style::new().fg(theme::fg::MUTED),
        ));

        for (i, agent) in agents.iter().enumerate() {
            let is_selected = i == self.selected_agent;
            let sc = state_color(&agent.state);

            if is_selected {
                spans.push(Span::styled(
                    format!(" {} ", agent.name),
                    Style::new().fg(theme::bg::DEEP).bg(sc).bold(),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", agent.name),
                    Style::new().fg(sc),
                ));
            }
        }

        if agents.is_empty() {
            spans.push(Span::styled(
                " (no agents) ",
                Style::new().fg(theme::fg::DISABLED),
            ));
        } else {
            spans.push(Span::styled(
                "  (j/k to switch)",
                Style::new().fg(theme::fg::DISABLED),
            ));
        }

        Paragraph::new(Line::from_spans(spans))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(area, frame);
    }

    fn render_info(&self, frame: &mut Frame, area: Rect, agents: &[AgentInfo]) {
        if agents.is_empty() {
            let block = Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" No Agents ")
                .style(crate::theme::content_border());
            Paragraph::new("No agent data available")
                .block(block)
                .style(crate::theme::muted())
                .render(area, frame);
            return;
        }

        let idx = self.selected_agent.min(agents.len() - 1);
        let agent = &agents[idx];
        let title = format!(" {} ", agent.name);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title.as_str())
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height < 2 || inner.width < 4 {
            return;
        }

        let scolor = state_color(&agent.state);
        let running_label = if agent.running { "Yes" } else { "No" };
        let work_label = if agent.has_work { "Yes" } else { "No" };
        let mail_color = if agent.unread_mail > 0 {
            theme::accent::WARNING
        } else {
            theme::fg::MUTED
        };

        let lines = vec![
            Line::from_spans([
                Span::styled("   Role: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(&agent.role, Style::new().fg(theme::fg::PRIMARY)),
            ]),
            Line::from_spans([
                Span::styled("   Addr: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    &agent.address,
                    Style::new().fg(theme::fg::SECONDARY),
                ),
            ]),
            Line::from_spans([
                Span::styled("   Tmux: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    &agent.session,
                    Style::new().fg(theme::fg::SECONDARY),
                ),
            ]),
            Line::from_spans([
                Span::styled("  State: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(&agent.state, Style::new().fg(scolor).bold()),
            ]),
            Line::raw(""),
            Line::from_spans([
                Span::styled("Running: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    running_label,
                    Style::new().fg(if agent.running {
                        theme::accent::SUCCESS
                    } else {
                        theme::fg::MUTED
                    }),
                ),
            ]),
            Line::from_spans([
                Span::styled("   Work: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    work_label,
                    Style::new().fg(if agent.has_work {
                        theme::accent::INFO
                    } else {
                        theme::fg::MUTED
                    }),
                ),
            ]),
            Line::raw(""),
            Line::from_spans([
                Span::styled("   Mail: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{} unread", agent.unread_mail),
                    Style::new().fg(mail_color),
                ),
            ]),
        ];

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_tmux_panel(&self, frame: &mut Frame, area: Rect, agents: &[AgentInfo]) {
        let icon = if self.tmux_pane.in_tmux() { "\u{21c4}" } else { "\u{2717}" };
        let title = format!(" {} Tmux ", icon);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title.as_str())
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height < 2 || inner.width < 4 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        let ctx = &self.tmux_pane.context;

        if !ctx.session_name.is_empty() {
            lines.push(Line::from_spans([
                Span::styled(" Session: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    &ctx.session_name,
                    Style::new().fg(theme::accent::INFO).bold(),
                ),
            ]));
            lines.push(Line::from_spans([
                Span::styled("  Window: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{} ({})", ctx.window_index, ctx.window_name),
                    Style::new().fg(theme::fg::PRIMARY),
                ),
            ]));
        } else {
            lines.push(Line::styled(
                " Not running inside tmux",
                Style::new().fg(theme::fg::DISABLED),
            ));
        }

        lines.push(Line::raw(""));

        // Linked windows count
        let n = self.tmux_pane.linked_count();
        if n > 0 {
            lines.push(Line::from_spans([
                Span::styled("  Linked: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    format!("{} window{}", n, if n == 1 { "" } else { "s" }),
                    Style::new().fg(theme::accent::SUCCESS),
                ),
            ]));
        }

        // Peek indicator
        if let Some(peek) = self.tmux_pane.peek_session_name() {
            lines.push(Line::from_spans([
                Span::styled("    Peek: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(peek, Style::new().fg(theme::accent::INFO)),
            ]));
        }

        lines.push(Line::raw(""));

        // Selected agent hint
        if !agents.is_empty() {
            let idx = self.selected_agent.min(agents.len() - 1);
            let agent = &agents[idx];

            if !agent.running || agent.session.is_empty() {
                lines.push(Line::styled(
                    "  Agent is offline",
                    Style::new().fg(theme::fg::DISABLED),
                ));
            } else {
                let linked = self.tmux_pane.is_linked(&agent.session);
                let status = if linked { " (linked)" } else { "" };
                lines.push(Line::from_spans([
                    Span::styled(
                        "  Enter ",
                        Style::new().fg(theme::accent::PRIMARY).bold(),
                    ),
                    Span::styled(
                        format!("link{status} \u{2192} "),
                        Style::new().fg(theme::fg::SECONDARY),
                    ),
                    Span::styled(
                        &agent.session,
                        Style::new().fg(theme::accent::INFO),
                    ),
                ]));
            }
        }

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_actions(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Actions ")
            .title_alignment(Alignment::Left)
            .style(Style::new().fg(theme::fg::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.width < 4 || inner.height < 1 {
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw("  "));

        for (i, (key, label)) in ACTION_LABELS.iter().enumerate() {
            let is_focused = i == self.focused_action;

            if is_focused {
                spans.push(Span::styled(
                    format!(" [{}] {} ", key, label),
                    Style::new()
                        .fg(theme::bg::DEEP)
                        .bg(theme::accent::PRIMARY)
                        .bold(),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" [{}] ", key),
                    Style::new().fg(theme::accent::PRIMARY).bold(),
                ));
                spans.push(Span::styled(
                    format!("{} ", label),
                    Style::new().fg(theme::fg::SECONDARY),
                ));
            }

            if i < ACTION_LABELS.len() - 1 {
                spans.push(Span::raw(" "));
            }
        }

        if self.action_feedback_ttl > 0 {
            if let Some(ref action) = self.last_action {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("-> {}", action),
                    Style::new().fg(theme::accent::SUCCESS).italic(),
                ));
            }
        }

        Paragraph::new(Line::from_spans(spans))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    // --- Input handling ---

    pub fn handle_key(
        &mut self,
        key: &KeyEvent,
        agents: &[AgentInfo],
    ) -> Cmd<Msg> {
        if agents.is_empty() {
            return Cmd::None;
        }
        let count = agents.len();

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_agent > 0 {
                    self.selected_agent -= 1;
                } else {
                    self.selected_agent = count - 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected_agent = (self.selected_agent + 1) % count;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.focused_action > 0 {
                    self.focused_action -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.focused_action < ACTION_LABELS.len() - 1 {
                    self.focused_action += 1;
                }
            }
            KeyCode::Char('n') => {
                self.focused_action = 0;
                self.execute_action(agents);
            }
            KeyCode::Char('m') => {
                self.focused_action = 1;
                self.execute_action(agents);
            }
            KeyCode::Char('p') => {
                self.focused_action = 2;
                self.execute_action(agents);
            }
            KeyCode::Char('a') | KeyCode::Enter => {
                // Enter and 'a' always attach to the selected agent's tmux session
                if !agents.is_empty() {
                    let idx = self.selected_agent.min(agents.len() - 1);
                    let agent = agents[idx].clone();
                    self.focused_action = 3;
                    self.attach_agent(&agent);
                }
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn handle_mouse(
        &mut self,
        mouse: &MouseEvent,
        agents: &[AgentInfo],
    ) -> Cmd<Msg> {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let actions_area = self.layout_actions.get();
            if actions_area.contains(mouse.x, mouse.y) {
                let relative_x = mouse.x.saturating_sub(actions_area.x);
                let btn_width = actions_area.width / 4;
                if btn_width > 0 {
                    let btn_idx = (relative_x / btn_width) as usize;
                    if btn_idx < ACTION_LABELS.len() {
                        self.focused_action = btn_idx;
                        self.execute_action(agents);
                    }
                }
            }
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        if self.action_feedback_ttl > 0 {
            self.action_feedback_ttl -= 1;
        }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect, agents: &[AgentInfo]) {
        let outer = Flex::vertical()
            .constraints([
                Constraint::Fixed(1),
                Constraint::Min(5),
                Constraint::Fixed(3),
            ])
            .split(area);

        let selector_area = outer[0];
        let content_area = outer[1];
        let action_area = outer[2];

        self.layout_actions.set(action_area);
        self.render_agent_selector(frame, selector_area, agents);

        let columns = Flex::horizontal()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(60.0),
            ])
            .split(content_area);

        self.render_info(frame, columns[0], agents);
        self.render_tmux_panel(frame, columns[1], agents);
        self.render_actions(frame, action_area);
    }
}
