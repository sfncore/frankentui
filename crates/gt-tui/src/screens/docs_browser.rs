//! Docs Browser screen — fuzzy search CLI reference with command execution.
//!
//! Loads the auto-generated CLI docs index (from Cobra docgen) and provides:
//! - Fuzzy search over all gt commands
//! - Filter chips (leaf-only, has-args, has-completions)
//! - Full docs view (synopsis, flags, subcommands, carapace completion info)
//! - Command builder panel (side pane for filling positional args)
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

use crate::data::{self, carapace_complete, CliCommand};
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
// Completion source inference (mirrors app.rs logic for display)
// ---------------------------------------------------------------------------

/// Human-readable description of where completions come from for an arg.
fn completion_source_label(arg_name: &str, cmd: &str) -> &'static str {
    match arg_name {
        "rig" | "target-prefix" => return "rigs",
        "polecat" | "rig/polecat" => return "polecats",
        "agent" | "agent-bead" | "member" | "role" => return "agents",
        "convoy-id" => return "convoys",
        "bead-or-formula" => return "ready beads",
        _ => {}
    }
    if arg_name.contains("bead") || arg_name.contains("issue")
        || arg_name.contains("epic") || arg_name.contains("mr-id")
        || arg_name == "id"
    {
        if cmd.contains("close") || cmd.contains("ack") {
            return "in-progress beads";
        }
        return "all beads";
    }
    if arg_name == "name" || arg_name == "name..." {
        if cmd.contains("formula") { return "formulas"; }
        if cmd.contains("crew") || cmd.contains("dog") || cmd.contains("agent") {
            return "agents";
        }
    }
    if arg_name == "target" {
        if cmd.contains("polecat") { return "polecats"; }
        if cmd.contains("nudge") || cmd.contains("crew") { return "agents"; }
        if cmd.contains("sling") { return "rigs"; }
    }
    if arg_name.contains("message-id") || arg_name == "mail-id" {
        return "mail messages";
    }
    if arg_name == "thread-id" {
        return "mail threads";
    }
    "free text"
}

/// Whether an arg has dynamic completions (not free text).
fn has_dynamic_completions(arg_name: &str, cmd: &str) -> bool {
    completion_source_label(arg_name, cmd) != "free text"
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

/// Filter toggles for the command list.
#[derive(Debug, Clone, Copy)]
struct Filters {
    /// Only show leaf commands (not groups).
    leaf_only: bool,
    /// Only show commands with positional args.
    with_args: bool,
    /// Only show commands with carapace dynamic completions.
    with_completions: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            leaf_only: false,
            with_args: false,
            with_completions: false,
        }
    }
}

/// Tracks one positional arg slot in the command builder.
struct BuilderArg {
    /// Name from usage (e.g. "bead-id").
    name: String,
    /// User-supplied value (empty until filled).
    value: String,
    /// Human-readable completion source.
    source_label: &'static str,
    /// Carapace suggestions loaded async.
    suggestions: Vec<(String, String)>, // (value, description)
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
            .map(|name| {
                let source_label = completion_source_label(&name, &cmd.cmd);
                BuilderArg {
                    name,
                    value: String::new(),
                    source_label,
                    suggestions: Vec::new(),
                }
            })
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

    /// Load carapace suggestions for the current arg position.
    fn load_carapace_suggestions(&mut self) {
        let mut words: Vec<String> = self.base_cmd.split_whitespace().map(String::from).collect();
        for (i, arg) in self.args.iter().enumerate() {
            if i < self.current {
                words.push(arg.value.clone());
            }
        }
        words.push(String::new()); // empty = next position

        let word_refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
        let completions = carapace_complete(&word_refs);
        if let Some(arg) = self.args.get_mut(self.current) {
            arg.suggestions = completions
                .into_iter()
                .filter(|c| {
                    !c.value.starts_with('/')
                        && !c.value.starts_with('.')
                        && !c.value.starts_with('~')
                })
                .take(15)
                .map(|c| (c.value.trim().to_string(), c.description))
                .collect();
        }
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
    /// Filter toggles.
    filters: Filters,
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
            filters: Filters::default(),
            tick_count: 0,
        };
        s.refilter();
        s
    }

    fn refilter(&mut self) {
        let mut scored: Vec<(usize, u32)> = self
            .all_commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                // Apply filters
                if self.filters.leaf_only && c.is_parent {
                    return None;
                }
                if self.filters.with_args {
                    let args = data::parse_positional_args(&c.usage);
                    if args.is_empty() {
                        return None;
                    }
                }
                if self.filters.with_completions {
                    let args = data::parse_positional_args(&c.usage);
                    let has_dynamic = args.iter().any(|a| has_dynamic_completions(a, &c.cmd));
                    if !has_dynamic {
                        return None;
                    }
                }

                if self.query.is_empty() {
                    let base = if c.is_parent { 0 } else { 1 };
                    Some((i, base))
                } else {
                    let cmd_score = fuzzy_score(&self.query, &c.cmd);
                    let short_score = fuzzy_score(&self.query, &c.short);
                    let synopsis_score =
                        fuzzy_score(&self.query, &c.synopsis).map(|s| s / 2);
                    let best = [cmd_score, short_score, synopsis_score]
                        .iter()
                        .filter_map(|s| *s)
                        .max()?;
                    Some((i, best))
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();

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

        let mut builder = CommandBuilder::from_command(&cmd);
        if builder.args.is_empty() {
            // No positional args — execute immediately
            return;
        }
        // Pre-load carapace suggestions for the first arg
        builder.load_carapace_suggestions();
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
        // Global filter toggles: Ctrl+1/2/3
        match (key.code, key.modifiers) {
            (KeyCode::Char('1'), m) if m.contains(Modifiers::ALT) => {
                self.filters.leaf_only = !self.filters.leaf_only;
                self.refilter();
                return Cmd::None;
            }
            (KeyCode::Char('2'), m) if m.contains(Modifiers::ALT) => {
                self.filters.with_args = !self.filters.with_args;
                self.refilter();
                return Cmd::None;
            }
            (KeyCode::Char('3'), m) if m.contains(Modifiers::ALT) => {
                self.filters.with_completions = !self.filters.with_completions;
                self.refilter();
                return Cmd::None;
            }
            _ => {}
        }

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
                if self.builder.is_some() {
                    self.focus = Focus::Builder;
                } else {
                    self.focus = Focus::Search;
                }
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
                        // Load suggestions for the new arg
                        if !builder.ready {
                            builder.load_carapace_suggestions();
                        }
                    }
                }
            }
            (KeyCode::Tab, _) => {
                // Accept first suggestion if input is empty
                let should_accept = builder.args.get(builder.current)
                    .map(|a| a.value.is_empty() && !a.suggestions.is_empty())
                    .unwrap_or(false);
                if should_accept {
                    let suggestion = builder.args[builder.current].suggestions[0].0.clone();
                    if let Some(input) = builder.current_input_mut() {
                        *input = suggestion;
                    }
                    builder.check_ready();
                } else {
                    // Cycle to next arg
                    if !builder.args.is_empty() {
                        builder.current = (builder.current + 1) % builder.args.len();
                        builder.load_carapace_suggestions();
                    }
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
                    builder.load_carapace_suggestions();
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

    fn render_filter_bar(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(" Filters: ", Style::new().fg(theme::fg::MUTED)));

        let on = Style::new().fg(theme::bg::DEEP).bg(theme::accent::PRIMARY).bold();
        let off = Style::new().fg(theme::fg::MUTED);

        // Alt+1: Leaf only
        spans.push(Span::styled(
            " Alt+1 ",
            Style::new().fg(theme::accent::INFO),
        ));
        spans.push(Span::styled(
            if self.filters.leaf_only { " Leaf Only " } else { " Leaf Only " },
            if self.filters.leaf_only { on } else { off },
        ));
        spans.push(Span::raw("  "));

        // Alt+2: Has Args
        spans.push(Span::styled(
            " Alt+2 ",
            Style::new().fg(theme::accent::INFO),
        ));
        spans.push(Span::styled(
            if self.filters.with_args { " Has Args " } else { " Has Args " },
            if self.filters.with_args { on } else { off },
        ));
        spans.push(Span::raw("  "));

        // Alt+3: Has Completions
        spans.push(Span::styled(
            " Alt+3 ",
            Style::new().fg(theme::accent::INFO),
        ));
        spans.push(Span::styled(
            if self.filters.with_completions { " Completions " } else { " Completions " },
            if self.filters.with_completions { on } else { off },
        ));

        Paragraph::new(Line::from_spans(spans))
            .style(Style::new().bg(theme::bg::DEEP))
            .render(area, frame);
    }

    fn render_results_table(&self, frame: &mut Frame, area: Rect) {
        let is_active = self.focus == Focus::Results;

        let rows: Vec<Row> = self
            .filtered
            .iter()
            .take(200) // cap for performance
            .map(|&idx| {
                let cmd = &self.all_commands[idx];
                let args = data::parse_positional_args(&cmd.usage);
                let has_dynamic = args.iter().any(|a| has_dynamic_completions(a, &cmd.cmd));

                // Type badge
                let badge = if cmd.is_parent {
                    ("grp", theme::fg::MUTED)
                } else if has_dynamic {
                    ("\u{25b6}", theme::accent::SUCCESS)  // ▶ guided
                } else if !args.is_empty() {
                    ("arg", theme::accent::WARNING)
                } else {
                    ("cmd", theme::accent::INFO)
                };

                Row::new([
                    Text::from(Line::from_spans([Span::styled(
                        &cmd.cmd,
                        Style::new().fg(theme::accent::INFO),
                    )])),
                    Text::raw(&cmd.short),
                    Text::from(Line::from_spans([Span::styled(
                        badge.0,
                        Style::new().fg(badge.1),
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

        // Positional args with completion info
        let pos_args = data::parse_positional_args(&cmd.usage);
        if !pos_args.is_empty() {
            lines.push(Line::styled(
                "ARGUMENTS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for arg_name in &pos_args {
                let source = completion_source_label(arg_name, &cmd.cmd);
                let has_dynamic = source != "free text";
                let icon = if has_dynamic { "\u{25b6}" } else { "\u{25cb}" };
                let source_style = if has_dynamic {
                    Style::new().fg(theme::accent::SUCCESS)
                } else {
                    Style::new().fg(theme::fg::DISABLED)
                };
                lines.push(Line::from_spans([
                    Span::styled(
                        format!("  {icon} <{arg_name}>"),
                        Style::new().fg(theme::accent::INFO),
                    ),
                    Span::styled(
                        format!("  \u{2192} {source}"),
                        source_style,
                    ),
                ]));
            }
            lines.push(Line::raw(""));
        }

        // Synopsis
        if !cmd.synopsis.is_empty() {
            lines.push(Line::styled(
                "SYNOPSIS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for line in cmd.synopsis.lines().take(10) {
                lines.push(Line::styled(
                    format!("  {}", line),
                    Style::new().fg(theme::fg::SECONDARY),
                ));
            }
            if cmd.synopsis.lines().count() > 10 {
                lines.push(Line::styled(
                    "  ...(truncated)",
                    Style::new().fg(theme::fg::DISABLED),
                ));
            }
            lines.push(Line::raw(""));
        }

        // Options (compact)
        if !cmd.options.is_empty() {
            lines.push(Line::styled(
                "OPTIONS:",
                Style::new().fg(theme::accent::PRIMARY).bold(),
            ));
            for line in cmd.options.lines().take(15) {
                let style = if line.trim_start().starts_with('-') {
                    Style::new().fg(theme::accent::INFO)
                } else {
                    Style::new().fg(theme::fg::SECONDARY)
                };
                lines.push(Line::styled(format!("  {}", line), style));
            }
            if cmd.options.lines().count() > 15 {
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
            for sub in cmd.subcommands.iter().take(15) {
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
            if cmd.subcommands.len() > 15 {
                lines.push(Line::styled(
                    format!("  ...({} more)", cmd.subcommands.len() - 15),
                    Style::new().fg(theme::fg::DISABLED),
                ));
            }
            lines.push(Line::raw(""));
        }

        // Help hint — varies based on whether command has args
        let has_args = !pos_args.is_empty();
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

        // Top: search bar + filter bar.  Middle: content.
        let main = Flex::vertical()
            .constraints([
                Constraint::Fixed(3),  // Search bar
                Constraint::Fixed(1),  // Filter bar
                Constraint::Fill,      // Content
            ])
            .split(area);

        self.render_search_bar(frame, main[0]);
        self.render_filter_bar(frame, main[1]);

        // Content: commands list on left, detail + builder on right
        let has_builder = self.builder.is_some();

        if has_builder {
            // Three columns: commands | detail | builder
            let content = Flex::horizontal()
                .constraints([
                    Constraint::Percentage(30.0),
                    Constraint::Percentage(35.0),
                    Constraint::Percentage(35.0),
                ])
                .split(main[2]);

            self.render_results_table(frame, content[0]);
            self.render_detail(frame, content[1]);
            self.render_builder(frame, content[2]);
        } else {
            // Two columns: commands | detail
            let content = Flex::horizontal()
                .constraints([
                    Constraint::Percentage(40.0),
                    Constraint::Percentage(60.0),
                ])
                .split(main[2]);

            self.render_results_table(frame, content[0]);
            self.render_detail(frame, content[1]);
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
            " Builder \u{2714} Ready "
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
        lines.push(Line::raw(""));

        // Arg slots with status indicators
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

        // Current input
        let current_arg = builder.args.get(builder.current);
        let current_label = current_arg.map(|a| a.name.as_str()).unwrap_or("?");
        let source_label = current_arg.map(|a| a.source_label).unwrap_or("?");
        let current_val = builder.current_input();
        let cursor = if is_active { "\u{2588}" } else { "" };
        lines.push(Line::from_spans([
            Span::styled(format!("{current_label}: "), active_style),
            Span::styled(
                format!("{current_val}{cursor}"),
                Style::new().fg(theme::fg::PRIMARY).bold(),
            ),
            Span::styled(
                format!("  ({source_label})"),
                Style::new().fg(theme::fg::DISABLED),
            ),
        ]));
        lines.push(Line::raw(""));

        // Carapace suggestions
        if let Some(arg) = current_arg {
            if !arg.suggestions.is_empty() {
                lines.push(Line::styled(
                    "Suggestions:",
                    Style::new().fg(theme::accent::PRIMARY).bold(),
                ));
                for (i, (val, desc)) in arg.suggestions.iter().enumerate() {
                    if lines.len() as u16 >= inner.height.saturating_sub(2) {
                        let remaining = arg.suggestions.len() - i;
                        lines.push(Line::styled(
                            format!("  ...({remaining} more)"),
                            Style::new().fg(theme::fg::DISABLED),
                        ));
                        break;
                    }
                    let mut spans = vec![
                        Span::styled(
                            format!("  {val}"),
                            Style::new().fg(theme::accent::INFO),
                        ),
                    ];
                    if !desc.is_empty() {
                        spans.push(Span::styled(
                            format!("  {desc}"),
                            Style::new().fg(theme::fg::MUTED),
                        ));
                    }
                    lines.push(Line::from_spans(spans));
                }
            } else if arg.source_label != "free text" {
                lines.push(Line::styled(
                    "Loading suggestions...",
                    Style::new().fg(theme::fg::DISABLED),
                ));
            }
        }

        // Pad to fill, then hints at bottom
        while (lines.len() as u16) < inner.height.saturating_sub(1) {
            lines.push(Line::raw(""));
        }

        let hint = if builder.ready {
            "[Enter] run  [Tab] accept/cycle  [Esc] cancel"
        } else {
            "[Enter] next  [Tab] accept/cycle  [Esc] cancel"
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
