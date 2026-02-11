#![forbid(unsafe_code)]

//! Agent Detail screen — modal-style panel showing agent info and actions.
//!
//! Demonstrates a detail view for a Gas Town agent, showing identity info,
//! running state, hooked work, mail count, recent events, and action buttons.
//! The panel uses a two-column layout: left shows agent metadata in a Block,
//! right shows recent events filtered to the selected agent.

use std::cell::Cell;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::{Line, Span, Text};
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;

use super::{HelpEntry, Screen};
use crate::theme;

// ---------------------------------------------------------------------------
// Mock data types
// ---------------------------------------------------------------------------

/// Running state for a mock agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentState {
    Running,
    Idle,
    Blocked,
    Done,
}

impl AgentState {
    fn label(self) -> &'static str {
        match self {
            AgentState::Running => "RUNNING",
            AgentState::Idle => "IDLE",
            AgentState::Blocked => "BLOCKED",
            AgentState::Done => "DONE",
        }
    }

    fn color(self) -> theme::ColorToken {
        match self {
            AgentState::Running => theme::accent::SUCCESS,
            AgentState::Idle => theme::fg::MUTED,
            AgentState::Blocked => theme::accent::WARNING,
            AgentState::Done => theme::accent::INFO,
        }
    }
}

/// A mock agent for demonstration purposes.
#[derive(Debug, Clone)]
struct MockAgent {
    name: &'static str,
    role: &'static str,
    address: &'static str,
    tmux_session: &'static str,
    state: AgentState,
    hooked_work_title: &'static str,
    hooked_work_id: &'static str,
    unread_mail: u32,
}

/// A mock event for the agent's recent activity.
#[derive(Debug, Clone)]
struct AgentEvent {
    tick: u64,
    kind: &'static str,
    message: &'static str,
}

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

/// Agent detail panel screen state.
pub struct AgentDetail {
    /// Index of the currently selected agent in the mock list.
    selected_agent: usize,
    /// Mock agent data for demonstration.
    agents: Vec<MockAgent>,
    /// Recent events for the selected agent.
    events: Vec<Vec<AgentEvent>>,
    /// Which action button is focused (0-3).
    focused_action: usize,
    /// Global tick counter.
    tick_count: u64,
    /// Last action invoked (displayed as feedback).
    last_action: Option<String>,
    /// Ticks remaining for action feedback display.
    action_feedback_ttl: u64,
    /// Cached detail panel area for mouse hit-testing.
    layout_detail: Cell<Rect>,
    /// Cached events panel area for mouse hit-testing.
    layout_events: Cell<Rect>,
    /// Cached action bar area for mouse hit-testing.
    layout_actions: Cell<Rect>,
}

impl Default for AgentDetail {
    fn default() -> Self {
        Self::new()
    }
}

const ACTION_LABELS: [(&str, &str); 4] = [
    ("n", "nudge"),
    ("m", "mail"),
    ("p", "peek"),
    ("a", "attach"),
];

impl AgentDetail {
    pub fn new() -> Self {
        let agents = vec![
            MockAgent {
                name: "obsidian",
                role: "polecat",
                address: "frankentui/polecats/obsidian",
                tmux_session: "polecat-obsidian-7a41",
                state: AgentState::Running,
                hooked_work_title: "Agent Detail: Modal panel with info and actions",
                hooked_work_id: "bd-ydjj",
                unread_mail: 0,
            },
            MockAgent {
                name: "witness",
                role: "witness",
                address: "frankentui/witness",
                tmux_session: "witness-frankentui-3b2e",
                state: AgentState::Running,
                hooked_work_title: "Monitor polecat health and progress",
                hooked_work_id: "bd-w1tn",
                unread_mail: 3,
            },
            MockAgent {
                name: "refinery",
                role: "refinery",
                address: "frankentui/refinery",
                tmux_session: "refinery-frankentui-8f1c",
                state: AgentState::Idle,
                hooked_work_title: "(idle -- waiting for merge queue)",
                hooked_work_id: "",
                unread_mail: 1,
            },
            MockAgent {
                name: "cobalt",
                role: "polecat",
                address: "frankentui/polecats/cobalt",
                tmux_session: "polecat-cobalt-5d9a",
                state: AgentState::Blocked,
                hooked_work_title: "Focus Management: Replace Panel enum",
                hooked_work_id: "bd-280t",
                unread_mail: 2,
            },
            MockAgent {
                name: "mayor",
                role: "mayor",
                address: "mayor/",
                tmux_session: "mayor-main-0001",
                state: AgentState::Running,
                hooked_work_title: "Coordinate rig dispatches",
                hooked_work_id: "hq-m4yr",
                unread_mail: 5,
            },
        ];

        let events = vec![
            vec![
                AgentEvent { tick: 142, kind: "HOOK", message: "Hooked bd-ydjj via molecule bd-wisp-gp4t" },
                AgentEvent { tick: 143, kind: "STEP", message: "Started bd-wisp-n2xn: Load context" },
                AgentEvent { tick: 148, kind: "STEP", message: "Closed bd-wisp-n2xn" },
                AgentEvent { tick: 149, kind: "STEP", message: "Started bd-wisp-kmve: Set up branch" },
                AgentEvent { tick: 151, kind: "STEP", message: "Closed bd-wisp-kmve" },
                AgentEvent { tick: 152, kind: "BUILD", message: "cargo check passed" },
                AgentEvent { tick: 153, kind: "STEP", message: "Started bd-wisp-vpc2: Implement" },
            ],
            vec![
                AgentEvent { tick: 140, kind: "SPAWN", message: "Spawned polecat obsidian" },
                AgentEvent { tick: 141, kind: "ATTACH", message: "Attached molecule bd-wisp-gp4t" },
                AgentEvent { tick: 145, kind: "HEALTH", message: "Polecat obsidian: heartbeat OK" },
                AgentEvent { tick: 150, kind: "HEALTH", message: "Polecat cobalt: BLOCKED detected" },
                AgentEvent { tick: 155, kind: "NUDGE", message: "Nudged cobalt: check blockers" },
            ],
            vec![
                AgentEvent { tick: 120, kind: "MERGE", message: "Merged polecat/jade/bd-y8lc to main" },
                AgentEvent { tick: 125, kind: "CLOSE", message: "Closed bd-y8lc after merge" },
                AgentEvent { tick: 130, kind: "IDLE", message: "Queue empty, waiting for submissions" },
            ],
            vec![
                AgentEvent { tick: 135, kind: "HOOK", message: "Hooked bd-280t via molecule" },
                AgentEvent { tick: 136, kind: "STEP", message: "Started: Load context" },
                AgentEvent { tick: 138, kind: "BLOCKED", message: "Dependency bd-ydjj not yet complete" },
                AgentEvent { tick: 150, kind: "NUDGE", message: "Received nudge from witness" },
            ],
            vec![
                AgentEvent { tick: 100, kind: "DISPATCH", message: "Dispatched bd-ydjj to obsidian" },
                AgentEvent { tick: 105, kind: "DISPATCH", message: "Dispatched bd-280t to cobalt" },
                AgentEvent { tick: 110, kind: "MAIL", message: "Sent memory constraints to polecats" },
                AgentEvent { tick: 130, kind: "REVIEW", message: "Reviewed rig progress: 40% complete" },
                AgentEvent { tick: 155, kind: "PLAN", message: "Planning next batch of dispatches" },
            ],
        ];

        Self {
            selected_agent: 0,
            agents,
            events,
            focused_action: 0,
            tick_count: 0,
            last_action: None,
            action_feedback_ttl: 0,
            layout_detail: Cell::new(Rect::default()),
            layout_events: Cell::new(Rect::default()),
            layout_actions: Cell::new(Rect::default()),
        }
    }

    fn agent(&self) -> &MockAgent {
        &self.agents[self.selected_agent]
    }

    fn agent_events(&self) -> &[AgentEvent] {
        &self.events[self.selected_agent]
    }

    fn execute_action(&mut self) {
        let agent = &self.agents[self.selected_agent];
        let action = ACTION_LABELS[self.focused_action].1;
        self.last_action = Some(format!("{} -> {}", action, agent.name));
        self.action_feedback_ttl = 30;
    }

    // --- Rendering helpers ---

    fn render_info(&self, frame: &mut Frame, area: Rect) {
        let agent = self.agent();

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

        let state_color = agent.state.color();
        let mut lines = vec![
            Line::from_spans([
                Span::styled("  Role: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(agent.role, Style::new().fg(theme::fg::PRIMARY)),
            ]),
            Line::from_spans([
                Span::styled("  Addr: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(agent.address, Style::new().fg(theme::fg::SECONDARY)),
            ]),
            Line::from_spans([
                Span::styled("  Tmux: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(agent.tmux_session, Style::new().fg(theme::fg::SECONDARY)),
            ]),
            Line::from_spans([
                Span::styled(" State: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(agent.state.label(), Style::new().fg(state_color).bold()),
            ]),
            Line::raw(""),
            Line::from_spans([
                Span::styled("  Hook: ", Style::new().fg(theme::fg::MUTED)),
                Span::styled(
                    agent.hooked_work_title,
                    Style::new().fg(theme::accent::PRIMARY),
                ),
            ]),
        ];

        if !agent.hooked_work_id.is_empty() {
            lines.push(Line::from_spans([
                Span::raw("        "),
                Span::styled(
                    format!("({})", agent.hooked_work_id),
                    Style::new().fg(theme::fg::DISABLED),
                ),
            ]));
        }

        lines.push(Line::raw(""));

        let mail_color = if agent.unread_mail > 0 {
            theme::accent::WARNING
        } else {
            theme::fg::MUTED
        };
        lines.push(Line::from_spans([
            Span::styled("  Mail: ", Style::new().fg(theme::fg::MUTED)),
            Span::styled(
                format!("{} unread", agent.unread_mail),
                Style::new().fg(mail_color),
            ),
        ]));

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_events(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Recent Events ")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.height < 1 || inner.width < 4 {
            return;
        }

        let events = self.agent_events();
        let max_visible = inner.height as usize;
        let start = events.len().saturating_sub(max_visible);
        let visible = &events[start..];

        let lines: Vec<Line> = visible
            .iter()
            .map(|ev| {
                let kind_color = match ev.kind {
                    "HOOK" | "ATTACH" => theme::accent::PRIMARY,
                    "STEP" | "BUILD" => theme::accent::SUCCESS,
                    "HEALTH" | "IDLE" => theme::fg::MUTED,
                    "BLOCKED" => theme::accent::WARNING,
                    "NUDGE" | "MAIL" => theme::accent::INFO,
                    "MERGE" | "CLOSE" => theme::accent::SUCCESS,
                    "DISPATCH" | "PLAN" | "REVIEW" => theme::accent::PRIMARY,
                    "SPAWN" => theme::accent::INFO,
                    _ => theme::fg::SECONDARY,
                };
                Line::from_spans([
                    Span::styled(
                        format!(" t{:<4} ", ev.tick),
                        Style::new().fg(theme::fg::DISABLED),
                    ),
                    Span::styled(
                        format!("{:<8} ", ev.kind),
                        Style::new().fg(kind_color).bold(),
                    ),
                    Span::styled(ev.message, Style::new().fg(theme::fg::SECONDARY)),
                ])
            })
            .collect();

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

    fn render_agent_selector(&self, frame: &mut Frame, area: Rect) {
        if area.width < 4 || area.height < 1 {
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(" Agent: ", Style::new().fg(theme::fg::MUTED)));

        for (i, agent) in self.agents.iter().enumerate() {
            let is_selected = i == self.selected_agent;

            if is_selected {
                spans.push(Span::styled(
                    format!(" {} ", agent.name),
                    Style::new()
                        .fg(theme::bg::DEEP)
                        .bg(agent.state.color())
                        .bold(),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", agent.name),
                    Style::new().fg(agent.state.color()),
                ));
            }
        }

        spans.push(Span::styled(
            "  (j/k or Up/Down to switch)",
            Style::new().fg(theme::fg::DISABLED),
        ));

        Paragraph::new(Line::from_spans(spans))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(area, frame);
    }
}

impl Screen for AgentDetail {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_agent > 0 {
                        self.selected_agent -= 1;
                    } else {
                        self.selected_agent = self.agents.len() - 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected_agent = (self.selected_agent + 1) % self.agents.len();
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
                    self.execute_action();
                }
                KeyCode::Char('m') => {
                    self.focused_action = 1;
                    self.execute_action();
                }
                KeyCode::Char('p') => {
                    self.focused_action = 2;
                    self.execute_action();
                }
                KeyCode::Char('a') => {
                    self.focused_action = 3;
                    self.execute_action();
                }
                KeyCode::Enter => {
                    self.execute_action();
                }
                _ => {}
            }
        }

        if let Event::Mouse(mouse) = event {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let pos_x = mouse.x;
                let pos_y = mouse.y;
                let actions_area = self.layout_actions.get();
                if actions_area.contains(pos_x, pos_y) {
                    let relative_x = pos_x.saturating_sub(actions_area.x);
                    let btn_width = actions_area.width / 4;
                    if btn_width > 0 {
                        let btn_idx = (relative_x / btn_width) as usize;
                        if btn_idx < ACTION_LABELS.len() {
                            self.focused_action = btn_idx;
                            self.execute_action();
                        }
                    }
                }
            }
        }

        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
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
        self.render_agent_selector(frame, selector_area);

        let columns = Flex::horizontal()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(60.0),
            ])
            .split(content_area);

        self.layout_detail.set(columns[0]);
        self.layout_events.set(columns[1]);

        self.render_info(frame, columns[0]);
        self.render_events(frame, columns[1]);
        self.render_actions(frame, action_area);
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        if self.action_feedback_ttl > 0 {
            self.action_feedback_ttl -= 1;
        }
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "j/k",
                action: "Select agent",
            },
            HelpEntry {
                key: "h/l",
                action: "Focus action button",
            },
            HelpEntry {
                key: "Enter",
                action: "Execute focused action",
            },
            HelpEntry {
                key: "n",
                action: "Nudge agent",
            },
            HelpEntry {
                key: "m",
                action: "Mail agent",
            },
            HelpEntry {
                key: "p",
                action: "Peek (last 20 lines of tmux)",
            },
            HelpEntry {
                key: "a",
                action: "Attach (switch tmux session)",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Agent Detail"
    }

    fn tab_label(&self) -> &'static str {
        "Agent"
    }
}
