//! Docs Browser screen — fuzzy search CLI reference with command execution.
//!
//! Loads the auto-generated CLI docs index (from Cobra docgen) and provides:
//! - Fuzzy search over all gt commands
//! - Full docs view (synopsis, flags, subcommands)
//! - Execute selected command directly

use std::cell::RefCell;
use std::process::Command as ProcessCommand;

use ftui_core::event::{KeyCode, KeyEvent, Modifiers};
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
use ftui_widgets::table::{Row, Table, TableState};
use ftui_widgets::{StatefulWidget, Widget};

use crate::data::{self, CliCommand};
use crate::msg::Msg;

// ---------------------------------------------------------------------------
// Fuzzy matching (simple substring/word-start scorer)
// ---------------------------------------------------------------------------

fn fuzzy_score(query: &str, haystack: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let q = query.to_lowercase();
    let h = haystack.to_lowercase();

    // Exact match in command path
    if h.starts_with(&q) {
        return Some(1000);
    }

    // Word-start match (e.g. "ms" matches "mail send")
    let q_words: Vec<&str> = q.split_whitespace().collect();
    let h_words: Vec<&str> = h.split_whitespace().collect();
    if q_words.len() <= h_words.len() {
        let mut all_match = true;
        for (qw, hw) in q_words.iter().zip(h_words.iter()) {
            if !hw.starts_with(qw) {
                all_match = false;
                break;
            }
        }
        if all_match {
            return Some(800);
        }
    }

    // Substring match
    if h.contains(&q) {
        return Some(500);
    }

    // Check if all query chars appear in order (fuzzy)
    let mut chars = q.chars();
    let mut current = chars.next();
    for hc in h.chars() {
        if let Some(qc) = current {
            if hc == qc {
                current = chars.next();
            }
        } else {
            break;
        }
    }
    if current.is_none() {
        Some(100)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Screen state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Search,
    Results,
    Builder,
}

/// Tracks one positional arg slot in the command builder.
struct BuilderArg {
    /// Name from usage (e.g. "bead-id").
    name: String,
    /// User-supplied value (empty until filled).
    value: String,
}

/// State for the step-by-step command builder panel.
struct CommandBuilder {
    /// Base command path (e.g. "gt sling").
    base_cmd: String,
    /// Positional args parsed from usage.
    args: Vec<BuilderArg>,
    /// Index of the arg currently being filled.
    current: usize,
    /// Set to true once all args are filled and command is ready.
    ready: bool,
}

impl CommandBuilder {
    fn from_command(cmd: &CliCommand) -> Self {
        let positional = data::parse_positional_args(&cmd.usage);
        let args = positional
            .into_iter()
            .map(|name| BuilderArg { name, value: String::new() })
            .collect();
        Self {
            base_cmd: cmd.cmd.clone(),
            args,
            current: 0,
            ready: false,
        }
    }

    /// Build the full command string with filled args.
    fn full_command(&self) -> String {
        let mut parts = vec![self.base_cmd.clone()];
        for arg in &self.args {
            if !arg.value.is_empty() {
                parts.push(arg.value.clone());
            }
        }
        parts.join(" ")
    }

    /// Current arg being edited.
    fn current_input(&self) -> &str {
        self.args.get(self.current).map(|a| a.value.as_str()).unwrap_or("")
    }

    /// Current arg being edited (mutable).
    fn current_input_mut(&mut self) -> Option<&mut String> {
        self.args.get_mut(self.current).map(|a| &mut a.value)
    }

    /// Check whether all args are filled and mark ready.
    fn check_ready(&mut self) {
        self.ready = self.args.iter().all(|a| !a.value.is_empty());
    }
}

pub struct DocsBrowserScreen {
    all_commands: Vec<CliCommand>,
    filtered: Vec<usize>, // indices into all_commands
    query: String,
    focus: Focus,
    table_state: RefCell<TableState>,
    selected_detail: Option<usize>, // index into filtered
    last_run_output: Option<String>,
    /// Command builder state (active when user hits Enter on a command with args).
    builder: Option<CommandBuilder>,
    tick_count: u64,
}

impl DocsBrowserScreen {
    pub fn new(commands: Vec<CliCommand>) -> Self {
        let filtered: Vec<usize> = (0..commands.len()).collect();
        let mut s = Self {
            all_commands: commands,
            filtered,
            query: String::new(),
            focus: Focus::Search,
            table_state: RefCell::new(TableState::default()),
            selected_detail: None,
            last_run_output: None,
            builder: None,
            tick_count: 0,
        };
        s.refilter();
        s
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            // Show leaf commands first, then parents
            let mut scored: Vec<(usize, u32)> = self
                .all_commands
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let base = if c.is_parent { 0 } else { 1 };
                    (i, base)
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        } else {
            let mut scored: Vec<(usize, u32)> = self
                .all_commands
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    // Score against cmd path, short desc, and synopsis
                    let cmd_score = fuzzy_score(&self.query, &c.cmd);
                    let short_score = fuzzy_score(&self.query, &c.short);
                    let synopsis_score =
                        fuzzy_score(&self.query, &c.synopsis).map(|s| s / 2);
                    let best = [cmd_score, short_score, synopsis_score]
                        .iter()
                        .filter_map(|s| *s)
                        .max()?;
                    Some((i, best))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

        // Reset selection
        self.table_state.borrow_mut().select(if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        });
        self.selected_detail = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn selected_command(&self) -> Option<&CliCommand> {
        let idx = self.selected_detail?;
        let cmd_idx = *self.filtered.get(idx)?;
        self.all_commands.get(cmd_idx)
    }

    /// Enter command builder mode for the selected command.
    fn enter_builder_mode(&mut self) {
        let cmd = match self.selected_command() {
            Some(c) => c.clone(),
            None => return,
        };
        if cmd.is_parent {
            // For parent commands, just show help
            self.run_command_with_args(&cmd.cmd, "--help");
            return;
        }

        let builder = CommandBuilder::from_command(&cmd);
        if builder.args.is_empty() {
            // No positional args — execute immediately
            return;
        }
        self.builder = Some(builder);
        self.focus = Focus::Builder;
    }

    /// Execute the selected command directly (no args needed).
    fn execute_no_args(&mut self) -> Cmd<Msg> {
        let cmd = match self.selected_command() {
            Some(c) => c.clone(),
            None => return Cmd::None,
        };
        if cmd.is_parent {
            return self.run_command_with_args(&cmd.cmd, "--help");
        }
        self.run_command_with_args(&cmd.cmd, "")
    }

    /// Execute a command with the given args string (async via Cmd::Task).
    fn run_command_with_args(&mut self, base_cmd: &str, args_str: &str) -> Cmd<Msg> {
        let parts: Vec<&str> = base_cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Cmd::None;
        }

        let program = parts[0].to_string();
        let mut args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        for arg in args_str.split_whitespace() {
            args.push(arg.to_string());
        }

        self.last_run_output = Some("Running...".to_string());
        self.focus = Focus::Results;

        Cmd::Task(
            Default::default(),
            Box::new(move || {
                let result = ProcessCommand::new(&program).args(&args).output();
                let output = match result {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let mut combined = stdout.to_string();
                        if !stderr.is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(&stderr);
                        }
                        if combined.is_empty() {
                            "(no output)".to_string()
                        } else {
                            combined
                        }
                    }
                    Err(e) => format!("Error: {}", e),
                };
                Msg::DocsOutput(output)
            }),
        )
    }

    pub fn set_last_output(&mut self, output: String) {
        self.last_run_output = Some(output);
    }

    pub fn consumes_text_input(&self) -> bool {
        matches!(self.focus, Focus::Search | Focus::Builder)
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match self.focus {
            Focus::Search => { self.handle_search_key(key); Cmd::None }
            Focus::Results => self.handle_results_key(key),
            Focus::Builder => self.handle_builder_key(key),
        }
    }

    fn handle_search_key(&mut self, key: &KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Escape, _) => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.refilter();
                }
            }
            (KeyCode::Tab, _) | (KeyCode::Down, _) => {
                self.focus = Focus::Results;
            }
            (KeyCode::Enter, _) => {
                // Switch to results and select first match
                self.focus = Focus::Results;
            }
            (KeyCode::Backspace, _) => {
                self.query.pop();
                self.refilter();
            }
            (KeyCode::Char('c'), m) if m.contains(Modifiers::CTRL) => {
                // Let global handler deal with Ctrl+C
            }
            (KeyCode::Char(c), _) => {
                self.query.push(c);
                self.refilter();
            }
            _ => {}
        }
    }

    fn handle_results_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        let count = self.filtered.len();
        match (key.code, key.modifiers) {
            (KeyCode::Escape, _) | (KeyCode::BackTab, _) => {
                self.focus = Focus::Search;
            }
            (KeyCode::Tab, _) => {
                self.focus = Focus::Search;
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                let mut state = self.table_state.borrow_mut();
                if let Some(sel) = state.selected {
                    if sel > 0 {
                        state.select(Some(sel - 1));
                        drop(state);
                        self.selected_detail = Some(sel - 1);
                        self.last_run_output = None;
                    }
                }
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                let mut state = self.table_state.borrow_mut();
                if let Some(sel) = state.selected {
                    if sel + 1 < count {
                        state.select(Some(sel + 1));
                        drop(state);
                        self.selected_detail = Some(sel + 1);
                        self.last_run_output = None;
                    }
                } else if count > 0 {
                    state.select(Some(0));
                    drop(state);
                    self.selected_detail = Some(0);
                    self.last_run_output = None;
                }
            }
            (KeyCode::Enter, _) => {
                // If command has positional args → open builder, else execute directly
                let has_args = self.selected_command()
                    .map(|c| !data::parse_positional_args(&c.usage).is_empty())
                    .unwrap_or(false);
                if has_args {
                    self.enter_builder_mode();
                } else {
                    return self.execute_no_args();
                }
            }
            (KeyCode::Char('/'), _) => {
                self.focus = Focus::Search;
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_builder_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        let builder = match self.builder.as_mut() {
            Some(b) => b,
            None => { self.focus = Focus::Results; return Cmd::None; }
        };

        match (key.code, key.modifiers) {
            (KeyCode::Escape, _) => {
                self.builder = None;
                self.focus = Focus::Results;
            }
            (KeyCode::Enter, _) => {
                if builder.ready {
                    // All args filled — execute
                    let full = builder.full_command();
                    self.builder = None;
                    return self.run_command_with_args(&full, "");
                }
                // Current arg entered — advance to next
                if let Some(input) = builder.current_input_mut() {
                    if !input.is_empty() {
                        builder.current += 1;
                        if builder.current >= builder.args.len() {
                            builder.current = builder.args.len() - 1;
                        }
                        builder.check_ready();
                    }
                }
            }
            (KeyCode::Tab, _) => {
                // Cycle to next arg
                if !builder.args.is_empty() {
                    builder.current = (builder.current + 1) % builder.args.len();
                }
            }
            (KeyCode::BackTab, _) => {
                // Cycle to previous arg
                if !builder.args.is_empty() {
                    if builder.current > 0 {
                        builder.current -= 1;
                    } else {
                        builder.current = builder.args.len() - 1;
                    }
                }
            }
            (KeyCode::Backspace, _) => {
                if let Some(input) = builder.current_input_mut() {
                    input.pop();
                    builder.check_ready();
                }
            }
            (KeyCode::Char('c'), m) if m.contains(Modifiers::CTRL) => {
                // Let global handler deal with Ctrl+C
            }
            (KeyCode::Char(c), _) => {
                if let Some(input) = builder.current_input_mut() {
                    input.push(c);
                    builder.check_ready();
                }
            }
            _ => {}
        }
        Cmd::None
    }

    pub fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
    }

    // --- Rendering ---

    fn render_search_bar(&self, frame: &mut Frame, area: Rect) {
        let is_active = self.focus == Focus::Search;

        let border_style = if is_active {
            Style::new().fg(theme::accent::PRIMARY)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let match_count = self.filtered.len();
        let total = self.all_commands.len();
        let title = format!(
            " Search ({}/{}) ",
            match_count, total
        );

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if is_active {
                BorderType::Double
            } else {
                BorderType::Rounded
            })
            .title(title.as_str())
            .title_alignment(Alignment::Left)
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let cursor = if is_active { "\u{2588}" } else { "" };
        let display = if self.query.is_empty() {
            if is_active {
                format!("Type to search gt commands...{}", cursor)
            } else {
                "Type to search gt commands...".to_string()
            }
        } else {
            format!("{}{}", self.query, cursor)
        };

        let style = if self.query.is_empty() {
            Style::new().fg(theme::fg::DISABLED)
        } else {
            Style::new().fg(theme::fg::PRIMARY).bold()
        };

        Paragraph::new(Text::styled(display, style)).render(inner, frame);
    }

    fn render_results_table(&self, frame: &mut Frame, area: Rect) {
        let is_active = self.focus == Focus::Results;

        let rows: Vec<Row> = self
            .filtered
            .iter()
            .take(200) // cap for performance
            .map(|&idx| {
                let cmd = &self.all_commands[idx];
                let type_label = if cmd.is_parent { "grp" } else { "cmd" };
                let type_color = if cmd.is_parent {
                    theme::fg::MUTED
                } else {
                    theme::accent::SUCCESS
                };
                Row::new([
                    Text::from(Line::from_spans([Span::styled(
                        &cmd.cmd,
                        Style::new().fg(theme::accent::INFO),
                    )])),
                    Text::raw(&cmd.short),
                    Text::from(Line::from_spans([Span::styled(
                        type_label,
                        Style::new().fg(type_color),
                    )])),
                ])
            })
            .collect();

        let widths = [
            Constraint::Fixed(24),
            Constraint::Fill,
            Constraint::Fixed(5),
        ];
        let header = Row::new([
            Text::raw("Command"),
            Text::raw("Description"),
            Text::raw("Type"),
        ])
        .style(Style::new().bold());

        let border_style = if is_active {
            Style::new()
                .fg(theme::accent::PRIMARY)
                .bg(theme::bg::DEEP)
        } else {
            crate::theme::content_border()
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(" Commands ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(if is_active {
                        BorderType::Double
                    } else {
                        BorderType::Rounded
                    })
                    .style(border_style),
            )
            .highlight_style(Style::new().bg(theme::bg::SURFACE).bold());

        let mut state = self.table_state.borrow_mut();
        StatefulWidget::render(&table, area, frame, &mut state);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let cmd = match self.selected_command() {
            Some(c) => c,
            None => {
                let block = Block::default()
                    .title(" Documentation ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(crate::theme::content_border());
                Paragraph::new("Select a command to view docs")
                    .block(block)
                    .style(crate::theme::muted())
                    .render(area, frame);
                return;
            }
        };

        let title = format!(" {} ", cmd.cmd);
        let block = Block::default()
            .title(title.as_str())
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::new().fg(theme::accent::PRIMARY));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        // Check if we should show run output
        if let Some(ref output) = self.last_run_output {
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from_spans([
                Span::styled(
                    "Output of: ",
                    Style::new().fg(theme::fg::MUTED),
                ),
                Span::styled(
                    format!("{} --help", cmd.cmd),
                    Style::new().fg(theme::accent::INFO).bold(),
                ),
            ]));
            lines.push(Line::raw(""));
            for line in output.lines() {
                lines.push(Line::styled(
                    line.to_string(),
                    Style::new().fg(theme::fg::PRIMARY),
                ));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "[Esc] back to docs",
                Style::new().fg(theme::fg::DISABLED),
            ));

            let max_lines = inner.height as usize;
            if lines.len() > max_lines {
                lines.truncate(max_lines);
            }

            Paragraph::new(Text::from_lines(lines)).render(inner, frame);
            return;
        }

        // Build doc content
        let mut lines: Vec<Line> = Vec::new();

        // Usage
        if !cmd.usage.is_empty() {
            lines.push(Line::styled(
                "USAGE:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            lines.push(Line::styled(
                format!("  {}", cmd.usage),
                Style::new().fg(theme::fg::PRIMARY),
            ));
            lines.push(Line::raw(""));
        }

        // Synopsis
        if !cmd.synopsis.is_empty() {
            lines.push(Line::styled(
                "SYNOPSIS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for line in cmd.synopsis.lines().take(15) {
                lines.push(Line::styled(
                    format!("  {}", line),
                    Style::new().fg(theme::fg::SECONDARY),
                ));
            }
            if cmd.synopsis.lines().count() > 15 {
                lines.push(Line::styled(
                    "  ...(truncated)",
                    Style::new().fg(theme::fg::DISABLED),
                ));
            }
            lines.push(Line::raw(""));
        }

        // Options
        if !cmd.options.is_empty() {
            lines.push(Line::styled(
                "OPTIONS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for line in cmd.options.lines().take(20) {
                let style = if line.trim_start().starts_with('-') {
                    Style::new().fg(theme::accent::INFO)
                } else {
                    Style::new().fg(theme::fg::SECONDARY)
                };
                lines.push(Line::styled(format!("  {}", line), style));
            }
            if cmd.options.lines().count() > 20 {
                lines.push(Line::styled(
                    "  ...(truncated)",
                    Style::new().fg(theme::fg::DISABLED),
                ));
            }
            lines.push(Line::raw(""));
        }

        // Subcommands
        if !cmd.subcommands.is_empty() {
            lines.push(Line::styled(
                "SUBCOMMANDS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for sub in &cmd.subcommands {
                lines.push(Line::from_spans([
                    Span::styled(
                        format!("  {:<24}", sub.cmd),
                        Style::new().fg(theme::accent::INFO),
                    ),
                    Span::styled(
                        &sub.desc,
                        Style::new().fg(theme::fg::SECONDARY),
                    ),
                ]));
            }
            lines.push(Line::raw(""));
        }

        // Help hint — varies based on whether command has args
        let has_args = !data::parse_positional_args(&cmd.usage).is_empty();
        let enter_hint = if has_args { "build command" } else { "run" };
        lines.push(Line::from_spans([
            Span::styled(
                "[Enter] ",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ),
            Span::styled(
                enter_hint,
                Style::new().fg(theme::fg::MUTED),
            ),
            Span::styled(
                "  [/] ",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ),
            Span::styled(
                "search",
                Style::new().fg(theme::fg::MUTED),
            ),
            Span::styled(
                "  [Tab] ",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ),
            Span::styled(
                "switch focus",
                Style::new().fg(theme::fg::MUTED),
            ),
        ]));

        let max_lines = inner.height as usize;
        if lines.len() > max_lines {
            lines.truncate(max_lines);
        }

        Paragraph::new(Text::from_lines(lines)).render(inner, frame);
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let has_builder = self.builder.is_some();
        let constraints = if has_builder {
            vec![
                Constraint::Fixed(3),  // Search bar
                Constraint::Fill,      // Content
                Constraint::Fixed(7),  // Command builder
            ]
        } else {
            vec![
                Constraint::Fixed(3),  // Search bar
                Constraint::Fill,      // Content
            ]
        };

        let main = Flex::vertical().constraints(constraints).split(area);

        self.render_search_bar(frame, main[0]);

        let content = Flex::horizontal()
            .constraints([
                Constraint::Percentage(40.0),
                Constraint::Percentage(60.0),
            ])
            .split(main[1]);

        self.render_results_table(frame, content[0]);
        self.render_detail(frame, content[1]);

        if has_builder && main.len() > 2 {
            self.render_builder(frame, main[2]);
        }
    }

    fn render_builder(&self, frame: &mut Frame, area: Rect) {
        let builder = match &self.builder {
            Some(b) => b,
            None => return,
        };

        let is_active = self.focus == Focus::Builder;
        let border_style = if is_active {
            Style::new().fg(theme::accent::PRIMARY).bg(theme::bg::DEEP)
        } else {
            Style::new().fg(theme::fg::MUTED)
        };

        let title = if builder.ready {
            " Command Builder \u{2714} Ready "
        } else {
            " Command Builder "
        };
        let block = Block::new()
            .title(title)
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(if is_active { BorderType::Double } else { BorderType::Rounded })
            .style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let accent = Style::new().fg(theme::accent::PRIMARY).bold();
        let muted = Style::new().fg(theme::fg::MUTED);
        let filled_style = Style::new().fg(theme::accent::SUCCESS).bold();
        let active_style = Style::new().fg(theme::accent::INFO).bold();
        let placeholder_style = Style::new().fg(theme::fg::DISABLED);

        let mut lines: Vec<Line> = Vec::new();

        // Line 1: Command preview — show full command being built
        let mut cmd_spans: Vec<Span> = vec![
            Span::styled("$ ", muted),
            Span::styled(&builder.base_cmd, accent),
        ];
        for (i, arg) in builder.args.iter().enumerate() {
            cmd_spans.push(Span::raw(" "));
            if !arg.value.is_empty() {
                cmd_spans.push(Span::styled(&arg.value, filled_style));
            } else if i == builder.current {
                cmd_spans.push(Span::styled(
                    format!("<{}>", arg.name),
                    active_style,
                ));
            } else {
                cmd_spans.push(Span::styled(
                    format!("<{}>", arg.name),
                    placeholder_style,
                ));
            }
        }
        lines.push(Line::from_spans(cmd_spans));

        // Line 2: blank
        lines.push(Line::raw(""));

        // Line 3: Arg slots with status indicators
        let mut slot_spans: Vec<Span> = vec![Span::styled("Args: ", muted)];
        for (i, arg) in builder.args.iter().enumerate() {
            if i > 0 {
                slot_spans.push(Span::styled("  ", muted));
            }
            let indicator = if !arg.value.is_empty() {
                "\u{2714}" // ✔
            } else if i == builder.current {
                "\u{25b6}" // ▶
            } else {
                "\u{25cb}" // ○
            };
            let style = if i == builder.current {
                active_style
            } else if !arg.value.is_empty() {
                filled_style
            } else {
                placeholder_style
            };
            slot_spans.push(Span::styled(
                format!("{indicator} {}", arg.name),
                style,
            ));
        }
        lines.push(Line::from_spans(slot_spans));

        // Line 4: Current input
        let current_label = builder.args.get(builder.current)
            .map(|a| a.name.as_str())
            .unwrap_or("?");
        let current_val = builder.current_input();
        let cursor = if is_active { "\u{2588}" } else { "" };
        lines.push(Line::from_spans([
            Span::styled(format!("{current_label}: "), active_style),
            Span::styled(format!("{current_val}{cursor}"), Style::new().fg(theme::fg::PRIMARY).bold()),
        ]));

        // Line 5: Hints
        let hint = if builder.ready {
            "[Enter] run  [Tab] edit args  [Esc] cancel"
        } else {
            "[Enter] next arg  [Tab/Shift+Tab] switch  [Esc] cancel"
        };
        lines.push(Line::styled(hint, muted));

        for (i, line) in lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let row = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
            Paragraph::new(line.clone()).render(row, frame);
        }
    }
}
