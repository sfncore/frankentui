use std::cell::RefCell;
use std::time::{Duration, Instant};

use ftui_core::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, Modifiers, MouseButton, MouseEventKind,
};
use ftui_core::geometry::Rect;
use ftui_extras::theme;
use ftui_layout::{Constraint, Flex};
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_runtime::{Cmd, Every, Model, Subscription};
use ftui_widgets::command_palette::{ActionItem, CommandPalette, CompletionItem, PaletteAction};
use ftui_widgets::log_viewer::{LogViewer, LogViewerState};
use ftui_widgets::spinner::SpinnerState;
use ftui_widgets::status_line::{StatusItem, StatusLine};
use ftui_widgets::Widget;

use crate::data::{self, AgentInfo, BeadsSnapshot, CliCommand, ConvoyItem, FormulaItem, TownStatus};
use crate::msg::Msg;
use crate::panels;
use crate::screen::ActiveScreen;
use crate::screens::agent_detail::AgentDetailScreen;
use crate::screens::beads_overview::BeadsOverviewScreen;
use crate::screens::convoy_panel::ConvoyPanelScreen;
use crate::screens::docs_browser::DocsBrowserScreen;
use crate::screens::dashboard::DashboardScreen;
use crate::screens::event_feed::EventFeedScreen;
use crate::screens::layout_manager::LayoutManagerScreen;
use crate::screens::mail_inbox::MailInboxScreen;
use crate::screens::rigs::RigsScreen;
use crate::screens::tmux_commander::TmuxCommanderScreen;
use crate::screens::workflows::WorkflowsScreen;

// ---------------------------------------------------------------------------
// Arg completion types
// ---------------------------------------------------------------------------

/// Defines a positional argument for a CLI command.
struct ArgDef {
    name: &'static str,
    source: CompletionSource,
}

/// Where to pull completion candidates from.
enum CompletionSource {
    /// `self.beads.ready`
    ReadyBeads,
    /// `self.beads.in_progress`
    InProgressBeads,
    /// `self.status.rigs`
    Rigs,
    /// `self.all_agents`
    Agents,
    /// `self.formulas`
    Formulas,
    /// Rig polecats (agents with role "polecat")
    Polecats,
    /// No completions — user types freely.
    FreeText,
}

/// Tracks the multi-step arg-fill process for a command.
struct ArgFillState {
    /// Base command (e.g. "gt sling").
    command: String,
    /// Argument definitions.
    args: Vec<ArgDef>,
    /// Values filled so far.
    filled: Vec<String>,
    /// Index of the arg currently being filled.
    current: usize,
}

/// Map command IDs to their arg definitions (if any).
fn command_args(id: &str) -> Option<Vec<ArgDef>> {
    match id {
        "gt sling" => Some(vec![
            ArgDef { name: "bead", source: CompletionSource::ReadyBeads },
            ArgDef { name: "rig", source: CompletionSource::Rigs },
        ]),
        "gt crew start" => Some(vec![
            ArgDef { name: "rig", source: CompletionSource::Rigs },
            ArgDef { name: "agent", source: CompletionSource::Agents },
        ]),
        "gt crew stop" => Some(vec![
            ArgDef { name: "rig", source: CompletionSource::Rigs },
            ArgDef { name: "agent", source: CompletionSource::Agents },
        ]),
        "gt polecat nuke" => Some(vec![
            ArgDef { name: "target", source: CompletionSource::Polecats },
        ]),
        "gt nudge" => Some(vec![
            ArgDef { name: "target", source: CompletionSource::Agents },
        ]),
        "bd close" => Some(vec![
            ArgDef { name: "id", source: CompletionSource::InProgressBeads },
        ]),
        "gt formula run" => Some(vec![
            ArgDef { name: "name", source: CompletionSource::Formulas },
        ]),
        _ => None,
    }
}

/// Build the Gas Town action items for the command palette.
///
/// Screen navigation items come first, then all runnable (leaf) CLI commands
/// from the docs index.
fn gas_town_actions(cli_docs: &[CliCommand]) -> Vec<ActionItem> {
    let mut items = vec![
        // Navigation — always present
        ActionItem::new("screen-dashboard", "Dashboard")
            .with_description("Switch to Dashboard (F1)")
            .with_tags(&["screen", "dashboard", "home", "overview"])
            .with_category("Navigation"),
        ActionItem::new("screen-events", "Event Feed")
            .with_description("Switch to Event Feed (F2)")
            .with_tags(&["screen", "events", "feed", "log"])
            .with_category("Navigation"),
        ActionItem::new("screen-convoys", "Convoy Panel")
            .with_description("Switch to Convoy Panel (F3)")
            .with_tags(&["screen", "convoys", "progress"])
            .with_category("Navigation"),
        ActionItem::new("screen-agents", "Agent Detail")
            .with_description("Switch to Agent Detail (F4)")
            .with_tags(&["screen", "agents", "detail"])
            .with_category("Navigation"),
        ActionItem::new("screen-mail", "Mail Inbox")
            .with_description("Switch to Mail Inbox (F5)")
            .with_tags(&["screen", "mail", "inbox"])
            .with_category("Navigation"),
        ActionItem::new("screen-beads", "Beads Overview")
            .with_description("Switch to Beads Overview (F6)")
            .with_tags(&["screen", "beads", "issues"])
            .with_category("Navigation"),
        ActionItem::new("screen-rigs", "Rigs Management")
            .with_description("Manage rigs: start/stop witness/refinery (F7)")
            .with_tags(&["screen", "rigs", "management", "witness", "refinery"])
            .with_category("Navigation"),
        ActionItem::new("screen-tmux", "Tmux Commander")
            .with_description("Full tmux session/window/pane control (F8)")
            .with_tags(&["screen", "tmux", "sessions", "windows", "panes"])
            .with_category("Navigation"),
        ActionItem::new("screen-formulas", "Layouts")
            .with_description("Manage tmuxrs layouts (F9)")
            .with_tags(&["screen", "formulas", "layouts", "tmuxrs", "config"])
            .with_category("Navigation"),
        ActionItem::new("screen-workflows", "Workflows")
            .with_description("Browse GT workflow formulas")
            .with_tags(&["screen", "workflows", "formulas", "molecules"])
            .with_category("Navigation"),
        ActionItem::new("screen-docs", "CLI Docs Browser")
            .with_description("Search and browse gt CLI reference (F10/0)")
            .with_tags(&["screen", "docs", "help", "reference", "commands"])
            .with_category("Navigation"),
    ];

    // Quick-action commands — pre-built entries for common GT operations.
    // These pre-fill the palette with the command prefix so users can add args.
    let quick_actions = [
        ("gt sling", "<bead> <rig> \u{2014} Sling work to a polecat", &["sling", "dispatch", "polecat", "work"][..], "Quick Actions"),
        ("gt crew start", "<rig> <agent> \u{2014} Start a crew member", &["crew", "start", "agent"], "Quick Actions"),
        ("gt crew stop", "<rig> <agent> \u{2014} Stop a crew member", &["crew", "stop", "agent"], "Quick Actions"),
        ("gt polecat nuke", "<target> \u{2014} Kill polecat session + remove worktree", &["polecat", "nuke", "kill"], "Quick Actions"),
        ("gt nudge", "<target> \u{2014} Send message to agent session", &["nudge", "message", "agent"], "Quick Actions"),
        ("gt mail send", "Send mail to an agent", &["mail", "send", "message"], "Quick Actions"),
        ("bd create --title=", "Create a new bead/issue", &["bead", "create", "issue", "task"], "Quick Actions"),
        ("bd close", "<id> \u{2014} Close a bead/issue", &["bead", "close", "done"], "Quick Actions"),
        ("bd ready", "Show issues ready to work", &["bead", "ready", "available"], "Quick Actions"),
        ("bd list --status=open", "List all open issues", &["bead", "list", "open"], "Quick Actions"),
        ("gt status", "Refresh town status", &["status", "refresh", "overview"], "Quick Actions"),
        ("gt rig list", "List all rigs", &["rig", "list"], "Quick Actions"),
        ("gt convoy list", "List active convoys", &["convoy", "list", "batch"], "Quick Actions"),
        ("gt formula run", "<name> \u{2014} Run a formula", &["formula", "run", "workflow"], "Quick Actions"),
    ];
    for (cmd, desc, tags, category) in quick_actions {
        items.push(
            ActionItem::new(cmd, cmd)
                .with_description(desc)
                .with_tags(tags)
                .with_category(category),
        );
    }

    // Add all runnable CLI commands from docs
    for cmd in cli_docs {
        if cmd.is_parent || cmd.cmd.is_empty() {
            continue;
        }

        // Derive category from second word: "gt mail send" → "mail"
        let category = cmd.cmd.split_whitespace().nth(1).unwrap_or("gt");

        // Use all words as tags for fuzzy matching
        let words: Vec<&str> = cmd.cmd.split_whitespace().collect();

        items.push(
            ActionItem::new(&cmd.cmd, &cmd.cmd)
                .with_description(&cmd.short)
                .with_tags(&words)
                .with_category(category),
        );
    }

    items
}

/// Compare old and new TownStatus to generate synthetic events for changes.
fn status_delta_events(
    old: &TownStatus,
    new: &TownStatus,
) -> Vec<data::GtEvent> {
    use std::collections::HashMap;
    let mut events = Vec::new();

    // Build lookup of old rig state
    let old_rigs: HashMap<&str, &data::RigStatus> =
        old.rigs.iter().map(|r| (r.name.as_str(), r)).collect();
    let new_rigs: HashMap<&str, &data::RigStatus> =
        new.rigs.iter().map(|r| (r.name.as_str(), r)).collect();

    // Detect new rigs
    for (name, _rig) in &new_rigs {
        if !old_rigs.contains_key(name) {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "created".to_string(),
                actor: name.to_string(),
                message: format!("rig '{}' appeared", name),
            });
        }
    }

    // Detect removed rigs
    for (name, _rig) in &old_rigs {
        if !new_rigs.contains_key(name) {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "removed".to_string(),
                actor: name.to_string(),
                message: format!("rig '{}' removed", name),
            });
        }
    }

    // Compare each rig that exists in both
    for (name, new_rig) in &new_rigs {
        let Some(old_rig) = old_rigs.get(name) else {
            continue;
        };

        // Witness state change
        if !old_rig.has_witness && new_rig.has_witness {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "created".to_string(),
                actor: name.to_string(),
                message: format!("{}/witness started", name),
            });
        } else if old_rig.has_witness && !new_rig.has_witness {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "removed".to_string(),
                actor: name.to_string(),
                message: format!("{}/witness stopped", name),
            });
        }

        // Refinery state change
        if !old_rig.has_refinery && new_rig.has_refinery {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "created".to_string(),
                actor: name.to_string(),
                message: format!("{}/refinery started", name),
            });
        } else if old_rig.has_refinery && !new_rig.has_refinery {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "removed".to_string(),
                actor: name.to_string(),
                message: format!("{}/refinery stopped", name),
            });
        }

        // Polecat count change
        if new_rig.polecat_count > old_rig.polecat_count {
            let diff = new_rig.polecat_count - old_rig.polecat_count;
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "created".to_string(),
                actor: name.to_string(),
                message: format!(
                    "{} polecat{} spawned on {} (now {})",
                    diff,
                    if diff == 1 { "" } else { "s" },
                    name,
                    new_rig.polecat_count,
                ),
            });
        } else if new_rig.polecat_count < old_rig.polecat_count {
            let diff = old_rig.polecat_count - new_rig.polecat_count;
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "removed".to_string(),
                actor: name.to_string(),
                message: format!(
                    "{} polecat{} removed from {} (now {})",
                    diff,
                    if diff == 1 { "" } else { "s" },
                    name,
                    new_rig.polecat_count,
                ),
            });
        }

        // Crew count change
        if new_rig.crew_count != old_rig.crew_count {
            events.push(data::GtEvent {
                timestamp: String::new(),
                event_type: "update".to_string(),
                actor: name.to_string(),
                message: format!(
                    "{} crew count: {} -> {}",
                    name, old_rig.crew_count, new_rig.crew_count,
                ),
            });
        }

        // Agent state changes (running → stopped, stopped → running)
        let old_agents: HashMap<&str, &data::AgentInfo> =
            old_rig.agents.iter().map(|a| (a.name.as_str(), a)).collect();
        for agent in &new_rig.agents {
            if let Some(old_agent) = old_agents.get(agent.name.as_str()) {
                if !old_agent.running && agent.running {
                    events.push(data::GtEvent {
                        timestamp: String::new(),
                        event_type: "created".to_string(),
                        actor: format!("{}/{}", name, agent.name),
                        message: format!("{}/{} came online", name, agent.name),
                    });
                } else if old_agent.running && !agent.running {
                    events.push(data::GtEvent {
                        timestamp: String::new(),
                        event_type: "error".to_string(),
                        actor: format!("{}/{}", name, agent.name),
                        message: format!("{}/{} went offline", name, agent.name),
                    });
                }
            }
        }
    }

    // Unread mail count change
    if new.overseer.unread_mail != old.overseer.unread_mail
        && new.overseer.unread_mail > old.overseer.unread_mail
    {
        let diff = new.overseer.unread_mail - old.overseer.unread_mail;
        events.push(data::GtEvent {
            timestamp: String::new(),
            event_type: "update".to_string(),
            actor: "mail".to_string(),
            message: format!(
                "{} new mail message{}",
                diff,
                if diff == 1 { "" } else { "s" },
            ),
        });
    }

    events
}

pub struct GtApp {
    pub active_screen: ActiveScreen,
    // Shared data
    pub status: TownStatus,
    prev_status: Option<TownStatus>,
    pub convoys: Vec<ConvoyItem>,
    pub beads: BeadsSnapshot,
    pub all_agents: Vec<AgentInfo>,
    pub event_viewer: LogViewer,
    pub event_state: RefCell<LogViewerState>,
    // Per-screen state
    pub dashboard: DashboardScreen,
    pub event_feed_screen: EventFeedScreen,
    pub convoy_screen: ConvoyPanelScreen,
    pub agent_screen: AgentDetailScreen,
    pub mail_screen: MailInboxScreen,
    pub beads_screen: BeadsOverviewScreen,
    pub docs_screen: DocsBrowserScreen,
    pub rigs_screen: RigsScreen,
    pub tmux_commander: TmuxCommanderScreen,
    pub layout_manager: LayoutManagerScreen,
    pub workflows_screen: WorkflowsScreen,
    pub formulas: Vec<FormulaItem>,
    // Arg-fill state for command palette completions
    arg_fill: Option<ArgFillState>,
    // Global UI
    pub spinner_state: SpinnerState,
    pub spinner_tick: u32,
    pub last_refresh: Instant,
    pub palette_btn_area: RefCell<Rect>,
    pub tab_areas: RefCell<Vec<(Rect, ActiveScreen)>>,
    pub palette: CommandPalette,
}

impl GtApp {
    pub fn new() -> Self {
        let mut event_viewer = LogViewer::new(5_000);
        event_viewer.push("Gas Town TUI starting...");
        event_viewer.push("F1-F6 to switch screens, Tab to switch panels");
        event_viewer.push("Ctrl+K / Ctrl+P or : to open command palette");
        event_viewer.push("Click agent names to jump to their tmux session");
        event_viewer.push("");

        let cli_docs = data::load_cli_docs();

        let mut palette = CommandPalette::new().with_max_visible(9);
        palette.replace_actions(gas_town_actions(&cli_docs));

        let mut layout_manager = LayoutManagerScreen::new();
        layout_manager.tmuxrs_available = crate::tmuxrs::tmuxrs_available();

        Self {
            active_screen: ActiveScreen::Dashboard,
            status: TownStatus {
                name: "Gas Town".to_string(),
                ..Default::default()
            },
            prev_status: None,
            convoys: Vec::new(),
            beads: BeadsSnapshot::default(),
            all_agents: Vec::new(),
            event_viewer,
            event_state: RefCell::new(LogViewerState::default()),
            dashboard: DashboardScreen::new(),
            event_feed_screen: EventFeedScreen::new(),
            convoy_screen: ConvoyPanelScreen::new(),
            agent_screen: AgentDetailScreen::new(),
            mail_screen: MailInboxScreen::new(),
            beads_screen: BeadsOverviewScreen::new(),
            docs_screen: DocsBrowserScreen::new(cli_docs),
            rigs_screen: RigsScreen::new(),
            tmux_commander: TmuxCommanderScreen::new(),
            layout_manager,
            workflows_screen: WorkflowsScreen::new(),
            formulas: Vec::new(),
            arg_fill: None,
            spinner_state: SpinnerState::default(),
            spinner_tick: 0,
            last_refresh: Instant::now(),
            palette_btn_area: RefCell::new(Rect::default()),
            tab_areas: RefCell::new(Vec::new()),
            palette,
        }
    }

    /// Execute a command palette action by ID.
    fn execute_palette_action(&mut self, id: &str) -> Cmd<Msg> {
        // Screen navigation
        let screen = match id {
            "screen-dashboard" => Some(ActiveScreen::Dashboard),
            "screen-events" => Some(ActiveScreen::EventFeed),
            "screen-convoys" => Some(ActiveScreen::Convoys),
            "screen-agents" => Some(ActiveScreen::Agents),
            "screen-mail" => Some(ActiveScreen::Mail),
            "screen-beads" => Some(ActiveScreen::Beads),
            "screen-rigs" => Some(ActiveScreen::Rigs),
            "screen-tmux" => Some(ActiveScreen::TmuxCommander),
            "screen-formulas" => Some(ActiveScreen::Formulas),
            "screen-workflows" => Some(ActiveScreen::Workflows),
            "screen-docs" => Some(ActiveScreen::Docs),
            _ => None,
        };
        if let Some(s) = screen {
            self.active_screen = s;
            self.event_viewer
                .push(format!("Screen: {}", s.label()));
            return Cmd::None;
        }

        // Special: gt status triggers the full refresh cycle
        if id == "gt status" {
            self.last_refresh = Instant::now();
            self.event_viewer.push("Refreshing status...");
            return Cmd::Batch(vec![
                Cmd::Task(
                    Default::default(),
                    Box::new(|| Msg::StatusRefresh(data::fetch_status())),
                ),
                Cmd::Task(
                    Default::default(),
                    Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
                ),
            ]);
        }

        // CLI command selected — try arg-fill mode if we know the args.
        if id.starts_with("gt ") || id.starts_with("bd ") {
            if let Some(args) = command_args(id) {
                // Enter structured arg-fill mode
                let first_arg = &args[0];
                let prompt = format!("Select {}: (1/{})", first_arg.name, args.len());
                let items = self.generate_completions(&first_arg.source);
                let item_count = items.len();
                self.event_viewer.push(format!(
                    "Arg fill: {} ({} candidates)",
                    prompt, item_count
                ));
                self.arg_fill = Some(ArgFillState {
                    command: id.to_string(),
                    args,
                    filled: Vec::new(),
                    current: 0,
                });
                self.palette.enter_completion_mode(&prompt, items);
                return Cmd::None;
            }
            // No arg defs — fall back to pre-fill behavior.
            // open() first (resets state + makes visible), THEN set_query
            // (open() clears the query, so set_query must come after).
            let prefill = format!("{} ", id);
            self.palette.open();
            self.palette.set_query(&prefill);
            return Cmd::None;
        }

        self.event_viewer
            .push(format!("Unknown action: {id}"));
        Cmd::None
    }

    /// Execute a raw command string from the palette input (with user-supplied args).
    fn execute_raw_command(&mut self, cmd: &str) -> Cmd<Msg> {
        let cmd = cmd.trim().to_string();
        if cmd.is_empty() {
            return Cmd::None;
        }

        // Special: gt status triggers the full refresh cycle
        if cmd == "gt status" || cmd == "gt status --json" {
            self.last_refresh = Instant::now();
            self.event_viewer.push("Refreshing status...");
            return Cmd::Batch(vec![
                Cmd::Task(
                    Default::default(),
                    Box::new(|| Msg::StatusRefresh(data::fetch_status())),
                ),
                Cmd::Task(
                    Default::default(),
                    Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
                ),
            ]);
        }

        self.event_viewer.push(format!("$ {cmd}"));
        let cmd_owned = cmd.clone();
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                let output = data::run_cli_command(&cmd_owned);
                Msg::CommandOutput(cmd_owned, output)
            }),
        )
    }

    /// Collect all agents from top-level and per-rig into a flat list.
    fn collect_agents(status: &TownStatus) -> Vec<AgentInfo> {
        let mut agents = status.agents.clone();
        for rig in &status.rigs {
            agents.extend(rig.agents.iter().cloned());
        }
        agents
    }

    /// Generate completion items from a data source.
    fn generate_completions(&self, source: &CompletionSource) -> Vec<CompletionItem> {
        match source {
            CompletionSource::ReadyBeads => {
                self.beads.ready.iter().map(|b| CompletionItem {
                    value: b.id.clone(),
                    label: b.id.clone(),
                    description: b.title.clone(),
                }).collect()
            }
            CompletionSource::InProgressBeads => {
                self.beads.in_progress.iter().map(|b| CompletionItem {
                    value: b.id.clone(),
                    label: b.id.clone(),
                    description: b.title.clone(),
                }).collect()
            }
            CompletionSource::Rigs => {
                self.status.rigs.iter().map(|r| CompletionItem {
                    value: r.name.clone(),
                    label: r.name.clone(),
                    description: format!("{} polecats, {} crew", r.polecat_count, r.crew_count),
                }).collect()
            }
            CompletionSource::Agents => {
                self.all_agents.iter().map(|a| CompletionItem {
                    value: a.address.clone(),
                    label: if a.address.is_empty() { a.name.clone() } else { a.address.clone() },
                    description: format!("{} ({})", a.role, if a.running { "running" } else { "stopped" }),
                }).collect()
            }
            CompletionSource::Formulas => {
                self.formulas.iter().map(|f| CompletionItem {
                    value: f.name.clone(),
                    label: f.name.clone(),
                    description: f.description.clone(),
                }).collect()
            }
            CompletionSource::Polecats => {
                let mut items = Vec::new();
                for rig in &self.status.rigs {
                    for agent in &rig.agents {
                        if agent.role == "polecat" {
                            let target = format!("{}/{}", rig.name, agent.name);
                            items.push(CompletionItem {
                                value: target.clone(),
                                label: target,
                                description: if agent.running { "running".to_string() } else { "stopped".to_string() },
                            });
                        }
                    }
                }
                items
            }
            CompletionSource::FreeText => Vec::new(),
        }
    }

    /// Handle a completion value being selected in arg-fill mode.
    fn handle_arg_complete(&mut self, value: String) -> Cmd<Msg> {
        if self.arg_fill.is_none() {
            return Cmd::None;
        }

        {
            let state = self.arg_fill.as_mut().unwrap();
            state.filled.push(value);
            state.current += 1;
        }

        let state = self.arg_fill.as_ref().unwrap();
        if state.current < state.args.len() {
            // More args to fill — show next completion
            let prompt = format!("Select {}: ({}/{})", state.args[state.current].name, state.current + 1, state.args.len());
            let items = self.generate_completions(&state.args[state.current].source);
            self.palette.enter_completion_mode(&prompt, items);
            Cmd::None
        } else {
            // All args filled — build and execute the command
            let state = self.arg_fill.take().unwrap();
            let mut parts = vec![state.command];
            parts.extend(state.filled);
            let cmd = parts.join(" ");
            self.palette.close();
            self.execute_raw_command(&cmd)
        }
    }

    /// Handle backspace in arg-fill mode — go back to previous arg or exit.
    fn handle_arg_back(&mut self) -> Cmd<Msg> {
        if self.arg_fill.is_none() {
            return Cmd::None;
        }

        let go_back = {
            let state = self.arg_fill.as_ref().unwrap();
            state.current > 0
        };

        if go_back {
            {
                let state = self.arg_fill.as_mut().unwrap();
                state.current -= 1;
                state.filled.pop();
            }
            let state = self.arg_fill.as_ref().unwrap();
            let prompt = format!("Select {}: ({}/{})", state.args[state.current].name, state.current + 1, state.args.len());
            let items = self.generate_completions(&state.args[state.current].source);
            self.palette.enter_completion_mode(&prompt, items);
        } else {
            // At first arg — exit arg-fill, return to normal palette
            self.arg_fill = None;
            self.palette.exit_completion_mode();
            self.palette.open();
        }
        Cmd::None
    }

    /// Returns true if the active screen is in a text-input mode (e.g. search bar).
    fn consumes_text_input(&self) -> bool {
        match self.active_screen {
            ActiveScreen::EventFeed => self.event_feed_screen.consumes_text_input(),
            ActiveScreen::Docs => self.docs_screen.consumes_text_input(),
            ActiveScreen::TmuxCommander => self.tmux_commander.consumes_text_input(),
            ActiveScreen::Formulas => self.layout_manager.consumes_text_input(),
            ActiveScreen::Workflows => false,
            _ => false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Cmd<Msg> {
        if key.kind != KeyEventKind::Press {
            return Cmd::None;
        }

        // When palette is open, route all input through it first
        if self.palette.is_visible() {
            // Intercept Enter when query is a raw command (user typed args)
            // but NOT in completion mode (completion mode handles its own Enter)
            if key.code == KeyCode::Enter && !self.palette.is_in_completion_mode() {
                let q = self.palette.query().to_string();
                if q.starts_with("gt ") || q.starts_with("bd ") {
                    self.palette.close();
                    return self.execute_raw_command(&q);
                }
            }

            let event = Event::Key(key);
            if let Some(action) = self.palette.handle_event(&event) {
                return match action {
                    PaletteAction::Execute(id) => self.execute_palette_action(&id),
                    PaletteAction::Complete(value) => self.handle_arg_complete(value),
                    PaletteAction::CompleteBack => self.handle_arg_back(),
                    PaletteAction::Dismiss => {
                        self.arg_fill = None;
                        Cmd::None
                    }
                };
            }
            // Palette consumed the event
            return Cmd::None;
        }

        // Check if active screen is consuming text input (e.g. search bar)
        let text_input = self.consumes_text_input();

        // Global keys (palette closed)
        // Ctrl-modified keys and F-keys always work; single-char keys are
        // suppressed when a screen text-input mode is active.
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('C')
                if key.modifiers.contains(Modifiers::CTRL) =>
            {
                return Cmd::Quit;
            }
            KeyCode::Char('p') | KeyCode::Char('k')
                if key.modifiers.contains(Modifiers::CTRL) =>
            {
                self.palette.open();
                return Cmd::None;
            }
            // F-keys always switch screens
            KeyCode::F(n) => {
                if let Some(screen) = ActiveScreen::from_f_key(n) {
                    self.active_screen = screen;
                    self.event_viewer
                        .push(format!("Screen: {}", screen.label()));
                    return Cmd::None;
                }
            }
            // Shift+H / Shift+L — cycle screens (vim-style)
            KeyCode::Char('H') if key.modifiers.contains(Modifiers::SHIFT) => {
                self.active_screen = self.active_screen.prev();
                self.event_viewer
                    .push(format!("Screen: {}", self.active_screen.label()));
                return Cmd::None;
            }
            KeyCode::Char('L') if key.modifiers.contains(Modifiers::SHIFT) => {
                self.active_screen = self.active_screen.next();
                self.event_viewer
                    .push(format!("Screen: {}", self.active_screen.label()));
                return Cmd::None;
            }
            // Number keys 1-6 — direct screen access (suppressed during text input)
            KeyCode::Char(ch @ '1'..='9') if !text_input && key.modifiers == Modifiers::NONE => {
                if let Some(screen) = ActiveScreen::from_number_key(ch) {
                    self.active_screen = screen;
                    self.event_viewer
                        .push(format!("Screen: {}", screen.label()));
                    return Cmd::None;
                }
            }
            // Single-char global keys: only when no text input active
            KeyCode::Char('q') if !text_input && !key.modifiers.contains(Modifiers::CTRL) => {
                return Cmd::Quit;
            }
            KeyCode::Char(':') if !text_input => {
                self.palette.open();
                return Cmd::None;
            }
            KeyCode::Char('r') if !text_input => {
                self.last_refresh = Instant::now();
                self.event_viewer.push("Refreshing...");
                return Cmd::Batch(vec![
                    Cmd::Task(
                        Default::default(),
                        Box::new(|| Msg::StatusRefresh(data::fetch_status())),
                    ),
                    Cmd::Task(
                        Default::default(),
                        Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
                    ),
                ]);
            }
            _ => {}
        }

        // Delegate to active screen
        match self.active_screen {
            ActiveScreen::Dashboard => self.dashboard.handle_key(
                key,
                &mut self.event_viewer,
                &self.event_state,
                &self.convoys,
            ),
            ActiveScreen::EventFeed => self.event_feed_screen.handle_key(&key),
            ActiveScreen::Convoys => self.convoy_screen.handle_key(&key, &self.convoys),
            ActiveScreen::Agents => self.agent_screen.handle_key(&key, &self.all_agents),
            ActiveScreen::Mail => self.mail_screen.handle_key(&key),
            ActiveScreen::Beads => self.beads_screen.handle_key(&key, &self.beads),
            ActiveScreen::Rigs => self.rigs_screen.handle_key(&key, &self.status),
            ActiveScreen::TmuxCommander => self.tmux_commander.handle_key(&key),
            ActiveScreen::Formulas => self.layout_manager.handle_key(&key),
            ActiveScreen::Workflows => self.workflows_screen.handle_key(&key, &self.formulas),
            ActiveScreen::Docs => self.docs_screen.handle_key(&key),
        }
    }

    fn handle_mouse(&mut self, mouse: ftui_core::event::MouseEvent) -> Cmd<Msg> {
        // When palette is open, handle mouse on the palette
        if self.palette.is_visible() {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        modifiers: Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    if let Some(PaletteAction::Execute(id)) = self.palette.handle_event(&synth) {
                        return self.execute_palette_action(&id);
                    }
                }
                MouseEventKind::ScrollUp => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Up,
                        modifiers: Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    for _ in 0..3 {
                        let _ = self.palette.handle_event(&synth);
                    }
                }
                MouseEventKind::ScrollDown => {
                    let synth = Event::Key(KeyEvent {
                        code: KeyCode::Down,
                        modifiers: Modifiers::NONE,
                        kind: KeyEventKind::Press,
                    });
                    for _ in 0..3 {
                        let _ = self.palette.handle_event(&synth);
                    }
                }
                _ => {}
            }
            return Cmd::None;
        }

        // Left clicks: check global elements first (tab bar, command button),
        // then delegate to screen. Scroll events go straight to screen.
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            // Check if click is on a tab bar item
            for (tab_rect, screen) in self.tab_areas.borrow().iter() {
                if tab_rect.contains(mouse.x, mouse.y) {
                    self.active_screen = *screen;
                    self.event_viewer
                        .push(format!("Screen: {}", screen.label()));
                    return Cmd::None;
                }
            }

            // Check if click is on the Commands button
            let btn_area = *self.palette_btn_area.borrow();
            if btn_area.contains(mouse.x, mouse.y) {
                self.palette.open();
                return Cmd::None;
            }
        }

        // Delegate to active screen (clicks, scroll, etc.)
        match self.active_screen {
            ActiveScreen::Dashboard => {
                self.dashboard
                    .handle_mouse(mouse, &mut self.event_viewer)
            }
            ActiveScreen::EventFeed => self.event_feed_screen.handle_mouse(&mouse),
            ActiveScreen::Convoys => self.convoy_screen.handle_mouse(&mouse, &self.convoys),
            ActiveScreen::Agents => self.agent_screen.handle_mouse(&mouse, &self.all_agents),
            ActiveScreen::Mail => self.mail_screen.handle_mouse(&mouse),
            ActiveScreen::Beads => self.beads_screen.handle_mouse(&mouse, &self.beads),
            ActiveScreen::Rigs => self.rigs_screen.handle_mouse(&mouse, &self.status),
            ActiveScreen::TmuxCommander => self.tmux_commander.handle_mouse(&mouse),
            ActiveScreen::Formulas => self.layout_manager.handle_mouse(&mouse),
            ActiveScreen::Workflows => self.workflows_screen.handle_mouse(&mouse, &self.formulas),
            ActiveScreen::Docs => Cmd::None,
        }
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let bg = ftui_render::cell::PackedRgba::rgb(30, 30, 45);
        frame.buffer.fill(area, Cell::default().with_bg(bg));

        let mut tabs = Vec::new();
        let mut x = area.x;
        for screen in ActiveScreen::ALL {
            let n = screen.f_key();
            // Screens with f_key > 10 have no physical key binding
            let label = if n <= 10 {
                let num_key = if n == 10 { 0 } else { n };
                format!(" {}/F{}\u{00b7}{} ", num_key, n, screen.label())
            } else {
                format!(" \u{00b7}{} ", screen.label())
            };
            let is_active = *screen == self.active_screen;
            let fg = if is_active {
                theme::bg::DEEP.into()
            } else {
                theme::fg::SECONDARY.into()
            };
            let cell_bg = if is_active {
                theme::accent::PRIMARY.into()
            } else {
                bg
            };

            let tab_start = x;
            for ch in label.chars() {
                if x >= area.right() {
                    break;
                }
                if let Some(cell) = frame.buffer.get_mut(x, area.y) {
                    *cell = Cell::from_char(ch).with_fg(fg).with_bg(cell_bg);
                }
                x += 1;
            }
            let tab_width = x - tab_start;
            if tab_width > 0 {
                tabs.push((Rect::new(tab_start, area.y, tab_width, 1), *screen));
            }
        }
        *self.tab_areas.borrow_mut() = tabs;
    }

}

impl Model for GtApp {
    type Message = Msg;

    fn init(&mut self) -> Cmd<Self::Message> {
        Cmd::Batch(vec![
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::StatusRefresh(data::fetch_status())),
            ),
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::ConvoyRefresh(data::fetch_convoys())),
            ),
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::BeadsRefresh(data::fetch_beads())),
            ),
            Cmd::Task(
                Default::default(),
                Box::new(|| Msg::FormulasRefresh(data::fetch_formulas())),
            ),
        ])
    }

    fn update(&mut self, msg: Self::Message) -> Cmd<Self::Message> {
        match msg {
            Msg::Key(key) => self.handle_key(key),
            Msg::Mouse(mouse) => self.handle_mouse(mouse),
            Msg::Resize { .. } => Cmd::None,
            Msg::StatusRefresh(status) => {
                // Generate events from status deltas
                if let Some(prev) = &self.prev_status {
                    for event in status_delta_events(prev, &status) {
                        panels::event_feed::push_event(&mut self.event_viewer, &event);
                        self.event_feed_screen.push_real_event(&event);
                    }
                }
                self.prev_status = Some(status.clone());
                self.status = status;
                self.all_agents = Self::collect_agents(&self.status);
                self.last_refresh = Instant::now();
                self.dashboard.rebuild_tree_entries(&self.status);
                self.agent_screen.rescan_tmux();
                self.layout_manager.set_agents(self.all_agents.clone());
                Cmd::None
            }
            Msg::ConvoyRefresh(convoys) => {
                self.convoys = convoys;
                Cmd::None
            }
            Msg::BeadsRefresh(snapshot) => {
                self.beads = snapshot;
                Cmd::None
            }
            Msg::MailRefresh(messages) => {
                self.mail_screen.set_messages(messages);
                Cmd::None
            }
            Msg::NewEvent(event) => {
                panels::event_feed::push_event(&mut self.event_viewer, &event);
                self.event_feed_screen.push_real_event(&event);
                Cmd::None
            }
            Msg::CommandOutput(cmd, output) => {
                // Push to dashboard event viewer
                for line in output.lines().take(50) {
                    self.event_viewer.push(line);
                }
                if output.lines().count() > 50 {
                    self.event_viewer
                        .push(format!("... ({} lines total)", output.lines().count()));
                }
                self.event_viewer
                    .push(format!("--- {} done ---", cmd));

                // Also push to Event Feed screen as a real event
                let truncated = if output.len() > 200 {
                    format!("{}...", &output[..200])
                } else {
                    output.clone()
                };
                let event = data::GtEvent {
                    timestamp: String::new(),
                    event_type: "command".to_string(),
                    actor: "gt-tui".to_string(),
                    message: format!("$ {} → {}", cmd, truncated.replace('\n', " ")),
                };
                self.event_feed_screen.push_real_event(&event);
                Cmd::None
            }
            Msg::SwitchScreen(screen) => {
                self.active_screen = screen;
                self.event_viewer
                    .push(format!("Screen: {}", screen.label()));
                Cmd::None
            }
            Msg::TmuxSnapshot(snapshot) => {
                self.tmux_commander.set_snapshot(snapshot);
                Cmd::None
            }
            Msg::TmuxActionResult(action, result) => {
                let msg = match &result {
                    Ok(()) => format!("tmux: {action} ok"),
                    Err(e) => format!("tmux: {action} failed: {e}"),
                };
                self.event_viewer.push(msg.clone());
                let event = data::GtEvent {
                    timestamp: String::new(),
                    event_type: if result.is_ok() { "status" } else { "error" }.to_string(),
                    actor: "tmux".to_string(),
                    message: msg,
                };
                self.event_feed_screen.push_real_event(&event);
                // Trigger refresh after action
                crate::tmux::actions::fetch_snapshot()
            }
            Msg::TmuxSessionList(sessions) => {
                self.layout_manager.set_tmux_sessions(sessions);
                Cmd::None
            }
            Msg::TmuxrsConfigList(configs) => {
                self.layout_manager.set_configs(configs);
                Cmd::None
            }
            Msg::TmuxrsActionResult(action, result) => {
                match &result {
                    Ok(msg) => self.event_viewer.push(format!("tmuxrs: {msg}")),
                    Err(e) => self.event_viewer.push(format!("tmuxrs: {action} failed: {e}")),
                }
                // Refresh config list + session list after action
                Cmd::Task(
                    Default::default(),
                    Box::new(|| {
                        let configs = crate::tmuxrs::cli::list_configs().unwrap_or_default();
                        Msg::TmuxrsConfigList(configs)
                    }),
                )
            }
            Msg::FormulasRefresh(f) => {
                self.formulas = f;
                Cmd::None
            }
            Msg::FormulaDetailLoaded(d) => {
                self.workflows_screen.set_detail(d);
                Cmd::None
            }
            Msg::DocsOutput(output) => {
                self.docs_screen.set_last_output(output);
                Cmd::None
            }
            Msg::Tick => {
                self.spinner_state.tick();
                self.spinner_tick = self.spinner_tick.wrapping_add(1);
                let tick = self.spinner_tick as u64;
                self.event_feed_screen.tick(tick);
                self.convoy_screen.tick(tick);
                self.agent_screen.tick(tick);
                self.mail_screen.tick(tick);
                self.beads_screen.tick(tick);
                self.docs_screen.tick(tick);
                self.rigs_screen.tick(tick);
                self.tmux_commander.tick(tick);
                self.layout_manager.tick(tick);
                self.workflows_screen.tick(tick);
                Cmd::None
            }
            Msg::Noop => Cmd::None,
        }
    }

    fn view(&self, frame: &mut Frame) {
        let area = Rect::from_size(frame.buffer.width(), frame.buffer.height());

        // Fill background
        frame.buffer.fill(
            area,
            Cell::default()
                .with_bg(theme::bg::DEEP.into())
                .with_fg(theme::fg::PRIMARY.into()),
        );

        // Main layout: status bar (1), tab bar (1), content (fill), keybinds (1)
        let outer = Flex::vertical()
            .constraints([
                Constraint::Fixed(1), // Status bar
                Constraint::Fixed(1), // Tab bar
                Constraint::Min(6),   // Content
                Constraint::Fixed(1), // Keybinds
            ])
            .split(area);

        // --- Status Bar ---
        panels::status_bar::render(frame, outer[0], &self.status, self.spinner_tick);

        // --- Tab Bar ---
        self.render_tab_bar(frame, outer[1]);

        // --- Content (dispatched to active screen) ---
        match self.active_screen {
            ActiveScreen::Dashboard => {
                self.dashboard.view(
                    frame,
                    outer[2],
                    &self.status,
                    &self.convoys,
                    &self.event_viewer,
                    &self.event_state,
                );
            }
            ActiveScreen::EventFeed => {
                self.event_feed_screen.view(frame, outer[2]);
            }
            ActiveScreen::Convoys => {
                self.convoy_screen.view(frame, outer[2], &self.convoys);
            }
            ActiveScreen::Agents => {
                self.agent_screen.view(frame, outer[2], &self.all_agents);
            }
            ActiveScreen::Mail => {
                self.mail_screen.view(frame, outer[2]);
            }
            ActiveScreen::Beads => {
                self.beads_screen.view(frame, outer[2], &self.beads);
            }
            ActiveScreen::Rigs => {
                self.rigs_screen.view(frame, outer[2], &self.status);
            }
            ActiveScreen::TmuxCommander => {
                self.tmux_commander.view(frame, outer[2]);
            }
            ActiveScreen::Formulas => {
                self.layout_manager.view(frame, outer[2]);
            }
            ActiveScreen::Workflows => {
                self.workflows_screen.view(frame, outer[2], &self.formulas);
            }
            ActiveScreen::Docs => {
                self.docs_screen.view(frame, outer[2]);
            }
        }

        // --- Keybind Help Line ---
        let btn_label = " \u{25b8} Commands ";
        let btn_width = btn_label.len() as u16;
        let bottom = Flex::horizontal()
            .constraints([
                Constraint::Min(20),          // Keybind hints
                Constraint::Fixed(btn_width), // Commands button
            ])
            .split(outer[3]);

        let screen_label = format!("[{}]", self.active_screen.label());
        let keybind_bar = StatusLine::new()
            .style(crate::theme::status_bar_style())
            .separator("  ")
            .left(StatusItem::key_hint("0-9/F1-F10", "Screen"))
            .left(StatusItem::key_hint("Ctrl+K", "Palette"))
            .center(StatusItem::text(&screen_label))
            .right(StatusItem::key_hint("r", "Refresh"))
            .right(StatusItem::key_hint("q", "Quit"));

        keybind_bar.render(bottom[0], frame);

        // Clickable Commands button
        *self.palette_btn_area.borrow_mut() = bottom[1];
        let btn_bg = ftui_render::cell::PackedRgba::rgb(60, 60, 90);
        let btn_fg = ftui_render::cell::PackedRgba::rgb(220, 220, 240);
        for (i, ch) in btn_label.chars().enumerate() {
            let x = bottom[1].x + i as u16;
            if x >= bottom[1].right() {
                break;
            }
            if let Some(cell) = frame.buffer.get_mut(x, bottom[1].y) {
                cell.content = ftui_render::cell::CellContent::from_char(ch);
                cell.fg = btn_fg;
                cell.bg = btn_bg;
            }
        }

        // --- Command Palette Overlay (rendered last, on top) ---
        if self.palette.is_visible() {
            self.palette.render(area, frame);
        }
    }

    fn subscriptions(&self) -> Vec<Box<dyn Subscription<Self::Message>>> {
        let mut subs: Vec<Box<dyn Subscription<Self::Message>>> = vec![
            Box::new(Every::new(Duration::from_millis(100), || Msg::Tick)),
            Box::new(data::StatusPoller),
            Box::new(data::ConvoyPoller),
            Box::new(data::BeadsPoller),
            Box::new(data::MailPoller),
            Box::new(data::EventTailer),
        ];

        // Conditional pollers — only active on their screens
        if self.active_screen == ActiveScreen::Workflows {
            subs.push(Box::new(data::FormulaPoller));
        }
        if self.active_screen == ActiveScreen::TmuxCommander {
            subs.push(Box::new(data::TmuxPoller));
        }
        if self.active_screen == ActiveScreen::Formulas {
            subs.push(Box::new(data::TmuxrsConfigPoller));
        }

        subs
    }
}
