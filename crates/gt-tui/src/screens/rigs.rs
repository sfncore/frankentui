//! F7 — Rigs Management screen.
//!
//! Shows all rigs with status: name, polecat count, crew count,
//! witness/refinery status. Actions: start witness/refinery, start crew,
//! sling work, view polecats/beads.

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
use ftui_widgets::Widget;

use crate::data::{self, RigStatus, TownStatus};
use crate::msg::Msg;
use crate::screen::ActiveScreen;

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Detail,
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct RigsScreen {
    selected_rig: usize,
    focus: Focus,
    selected_action: usize,
    feedback: Option<(String, u64)>,
    tick_count: u64,
}

impl RigsScreen {
    pub fn new() -> Self {
        Self {
            selected_rig: 0,
            focus: Focus::List,
            selected_action: 0,
            feedback: None,
            tick_count: 0,
        }
    }

    fn set_feedback(&mut self, msg: impl Into<String>) {
        self.feedback = Some((msg.into(), self.tick_count + 30));
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        if let Some((_, ttl)) = &self.feedback {
            if tick_count > *ttl {
                self.feedback = None;
            }
        }
    }

    // =====================================================================
    // Key handling
    // =====================================================================

    pub fn handle_key(&mut self, key: &KeyEvent, status: &TownStatus) -> Cmd<Msg> {
        match self.focus {
            Focus::List => self.handle_list_key(key, status),
            Focus::Detail => self.handle_detail_key(key, status),
        }
    }

    fn handle_list_key(&mut self, key: &KeyEvent, status: &TownStatus) -> Cmd<Msg> {
        let rig_count = status.rigs.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if rig_count > 0 {
                    self.selected_rig = (self.selected_rig + 1).min(rig_count - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_rig = self.selected_rig.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if rig_count > 0 {
                    self.focus = Focus::Detail;
                    self.selected_action = 0;
                }
            }
            KeyCode::Tab => {
                if rig_count > 0 {
                    self.focus = Focus::Detail;
                    self.selected_action = 0;
                }
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_detail_key(&mut self, key: &KeyEvent, status: &TownStatus) -> Cmd<Msg> {
        let rig = match status.rigs.get(self.selected_rig) {
            Some(r) => r,
            None => {
                self.focus = Focus::List;
                return Cmd::None;
            }
        };

        let actions = rig_actions(rig);
        let action_count = actions.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if action_count > 0 {
                    self.selected_action = (self.selected_action + 1).min(action_count - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_action = self.selected_action.saturating_sub(1);
            }
            KeyCode::Escape | KeyCode::Char('h') | KeyCode::Left => {
                self.focus = Focus::List;
            }
            KeyCode::Tab => {
                self.focus = Focus::List;
            }
            KeyCode::Enter => {
                if let Some(action) = actions.get(self.selected_action) {
                    return self.execute_action(action, rig);
                }
            }
            // Quick keys for common actions
            KeyCode::Char('w') => {
                if rig.has_witness {
                    return self.run_command(&format!("gt crew stop {} witness", rig.name));
                } else {
                    return self.run_command(&format!("gt crew start {} witness", rig.name));
                }
            }
            KeyCode::Char('r') => {
                if rig.has_refinery {
                    return self.run_command(&format!("gt crew stop {} refinery", rig.name));
                } else {
                    return self.run_command(&format!("gt crew start {} refinery", rig.name));
                }
            }
            KeyCode::Char('c') => {
                return self.run_command(&format!("gt crew start {} --all", rig.name));
            }
            KeyCode::Char('p') => {
                return self.run_command(&format!("gt polecat list {}", rig.name));
            }
            KeyCode::Char('b') => {
                return Cmd::Msg(Msg::SwitchScreen(ActiveScreen::Beads));
            }
            KeyCode::Char('a') => {
                return Cmd::Msg(Msg::SwitchScreen(ActiveScreen::Agents));
            }
            _ => {}
        }
        Cmd::None
    }

    fn execute_action(&mut self, action: &RigAction, rig: &RigStatus) -> Cmd<Msg> {
        let cmd_str = match action.kind {
            ActionKind::StartWitness => {
                format!("gt crew start {} witness", rig.name)
            }
            ActionKind::StopWitness => {
                format!("gt crew stop {} witness", rig.name)
            }
            ActionKind::StartRefinery => {
                format!("gt crew start {} refinery", rig.name)
            }
            ActionKind::StopRefinery => {
                format!("gt crew stop {} refinery", rig.name)
            }
            ActionKind::StartAllCrew => {
                format!("gt crew start {} --all", rig.name)
            }
            ActionKind::ListPolecats => {
                format!("gt polecat list {}", rig.name)
            }
            ActionKind::NukeAllPolecats => {
                // Note: this is destructive — the feedback message shows the command
                format!("gt polecat nuke --rig {} --force", rig.name)
            }
            ActionKind::ViewAgents => {
                return Cmd::Msg(Msg::SwitchScreen(ActiveScreen::Agents));
            }
            ActionKind::ViewBeads => {
                return Cmd::Msg(Msg::SwitchScreen(ActiveScreen::Beads));
            }
        };
        self.run_command(&cmd_str)
    }

    fn run_command(&mut self, cmd: &str) -> Cmd<Msg> {
        let cmd_str = cmd.to_string();
        self.set_feedback(format!("$ {cmd_str}"));
        let cmd_owned = cmd_str.clone();
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                let output = data::run_cli_command(&cmd_owned);
                Msg::CommandOutput(cmd_owned, output)
            }),
        )
    }

    pub fn handle_mouse(&mut self, _mouse: &MouseEvent) -> Cmd<Msg> {
        Cmd::None
    }

    // =====================================================================
    // Rendering
    // =====================================================================

    pub fn view(&self, frame: &mut Frame, area: Rect, status: &TownStatus) {
        let columns = Flex::horizontal()
            .constraints([Constraint::Percentage(35.0), Constraint::Percentage(65.0)])
            .split(area);

        self.render_rig_list(frame, columns[0], status);
        self.render_rig_detail(frame, columns[1], status);
    }

    fn render_rig_list(&self, frame: &mut Frame, area: Rect, status: &TownStatus) {
        let focused = self.focus == Focus::List;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        let block = Block::new()
            .title(" Rigs ")
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

        let mut lines: Vec<Line> = Vec::new();

        if status.rigs.is_empty() {
            lines.push(Line::styled("No rigs", Style::new().fg(theme::fg::DISABLED)));
            lines.push(Line::styled(
                "Run: gt rig list",
                Style::new().fg(theme::fg::MUTED),
            ));
        } else {
            for (i, rig) in status.rigs.iter().enumerate() {
                let is_sel = i == self.selected_rig;

                // Status indicators
                let witness_icon = if rig.has_witness { "W" } else { "-" };
                let refinery_icon = if rig.has_refinery { "R" } else { "-" };
                let polecat_str = format!("{}p", rig.polecat_count);
                let crew_str = format!("{}c", rig.crew_count);

                let label = format!(
                    " {} [{}{}] {}/{} ",
                    rig.name, witness_icon, refinery_icon, polecat_str, crew_str,
                );

                if is_sel && focused {
                    lines.push(Line::styled(
                        label,
                        Style::new()
                            .fg(theme::bg::DEEP)
                            .bg(theme::accent::PRIMARY)
                            .bold(),
                    ));
                } else if is_sel {
                    lines.push(Line::styled(
                        label,
                        Style::new().fg(theme::accent::PRIMARY).bold(),
                    ));
                } else {
                    lines.push(Line::styled(label, Style::new().fg(theme::fg::PRIMARY)));
                }
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

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_rig_detail(&self, frame: &mut Frame, area: Rect, status: &TownStatus) {
        let focused = self.focus == Focus::Detail;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        let rig = status.rigs.get(self.selected_rig);
        let title = rig
            .map(|r| format!(" {} ", r.name))
            .unwrap_or_else(|| " (none) ".to_string());

        let block = Block::new()
            .title(&*title)
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

        let Some(rig) = rig else {
            Paragraph::new("Select a rig")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(inner, frame);
            return;
        };

        let mut lines: Vec<Line> = Vec::new();

        // Status section
        lines.push(Line::from_spans([
            Span::styled("Witness:  ", Style::new().fg(theme::fg::MUTED)),
            if rig.has_witness {
                Span::styled("running", Style::new().fg(theme::accent::SUCCESS).bold())
            } else {
                Span::styled("stopped", Style::new().fg(theme::accent::ERROR))
            },
        ]));

        lines.push(Line::from_spans([
            Span::styled("Refinery: ", Style::new().fg(theme::fg::MUTED)),
            if rig.has_refinery {
                Span::styled("running", Style::new().fg(theme::accent::SUCCESS).bold())
            } else {
                Span::styled("stopped", Style::new().fg(theme::accent::ERROR))
            },
        ]));

        lines.push(Line::from_spans([
            Span::styled("Polecats: ", Style::new().fg(theme::fg::MUTED)),
            Span::styled(
                rig.polecat_count.to_string(),
                Style::new().fg(theme::fg::PRIMARY).bold(),
            ),
        ]));

        lines.push(Line::from_spans([
            Span::styled("Crew:     ", Style::new().fg(theme::fg::MUTED)),
            Span::styled(
                rig.crew_count.to_string(),
                Style::new().fg(theme::fg::PRIMARY).bold(),
            ),
        ]));

        // Agent list
        if !rig.agents.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Agents:",
                Style::new().fg(theme::fg::SECONDARY).bold(),
            ));
            for agent in &rig.agents {
                let status_icon = if agent.running { "+" } else { "-" };
                let work_icon = if agent.has_work { "*" } else { " " };
                lines.push(Line::from_spans([
                    Span::styled(
                        format!("  {status_icon}{work_icon} "),
                        Style::new().fg(if agent.running {
                            theme::accent::SUCCESS
                        } else {
                            theme::fg::DISABLED
                        }),
                    ),
                    Span::styled(&agent.name, Style::new().fg(theme::fg::PRIMARY)),
                    Span::styled(
                        format!(" ({})", agent.role),
                        Style::new().fg(theme::fg::MUTED),
                    ),
                ]));
            }
        }

        // Actions
        if focused {
            let actions = rig_actions(rig);
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Actions:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for (i, action) in actions.iter().enumerate() {
                let is_sel = i == self.selected_action;
                let prefix = if is_sel { "> " } else { "  " };
                let style = if is_sel {
                    Style::new()
                        .fg(theme::bg::DEEP)
                        .bg(theme::accent::PRIMARY)
                        .bold()
                } else {
                    Style::new().fg(theme::fg::PRIMARY)
                };
                lines.push(Line::styled(format!("{prefix}{}", action.label), style));
            }

            lines.push(Line::raw(""));
            lines.push(Line::from_spans([
                Span::styled("[w]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("itness ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[r]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("efinery ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[c]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("rew ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[p]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("olecats ", Style::new().fg(theme::fg::MUTED)),
            ]));
            lines.push(Line::from_spans([
                Span::styled("[b]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("eads ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[a]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("gents", Style::new().fg(theme::fg::MUTED)),
            ]));
        }

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum ActionKind {
    StartWitness,
    StopWitness,
    StartRefinery,
    StopRefinery,
    StartAllCrew,
    ListPolecats,
    NukeAllPolecats,
    ViewAgents,
    ViewBeads,
}

struct RigAction {
    label: String,
    kind: ActionKind,
}

fn rig_actions(rig: &RigStatus) -> Vec<RigAction> {
    let mut actions = Vec::new();

    // Witness: start or stop depending on current state
    if rig.has_witness {
        actions.push(RigAction {
            label: format!("Stop witness (kill {}/witness)", rig.name),
            kind: ActionKind::StopWitness,
        });
    } else {
        actions.push(RigAction {
            label: format!("Start witness (gt crew start {})", rig.name),
            kind: ActionKind::StartWitness,
        });
    }

    // Refinery: start or stop depending on current state
    if rig.has_refinery {
        actions.push(RigAction {
            label: format!("Stop refinery (kill {}/refinery)", rig.name),
            kind: ActionKind::StopRefinery,
        });
    } else {
        actions.push(RigAction {
            label: format!("Start refinery (gt crew start {})", rig.name),
            kind: ActionKind::StartRefinery,
        });
    }

    // Crew operations
    actions.push(RigAction {
        label: format!("Start all crew (gt crew start {} --all)", rig.name),
        kind: ActionKind::StartAllCrew,
    });

    // Polecat operations
    actions.push(RigAction {
        label: format!("List polecats (gt polecat list {})", rig.name),
        kind: ActionKind::ListPolecats,
    });

    if rig.polecat_count > 0 {
        actions.push(RigAction {
            label: format!("Nuke all polecats (gt polecat nuke {}/\\* --force)", rig.name),
            kind: ActionKind::NukeAllPolecats,
        });
    }

    // Navigation
    actions.push(RigAction {
        label: "View agents (F4)".to_string(),
        kind: ActionKind::ViewAgents,
    });

    actions.push(RigAction {
        label: "View beads (F6)".to_string(),
        kind: ActionKind::ViewBeads,
    });

    actions
}
