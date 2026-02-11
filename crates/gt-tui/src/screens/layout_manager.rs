//! F9 — Formulas screen.
//!
//! Formulas are tmuxrs layout configs (~/.config/tmuxrs/*.yml).
//! Visual layout builder: pick a formula, preview its geometry, fill slots
//! with sessions/agents, hit Go to create the tmux session.
//! Supports: create, delete, duplicate, apply.

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

use crate::data::AgentInfo;
use crate::msg::Msg;
use crate::tmux::client as tmux_client;
use crate::tmuxrs;
use crate::tmuxrs::model::{Layout, PaneConfig, TmuxrsConfig};

// ---------------------------------------------------------------------------
// Layout presets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutPreset {
    EvenHorizontal,
    EvenVertical,
    MainHorizontal,
    MainVertical,
    Tiled,
}

const ALL_PRESETS: [LayoutPreset; 5] = [
    LayoutPreset::EvenHorizontal,
    LayoutPreset::EvenVertical,
    LayoutPreset::MainHorizontal,
    LayoutPreset::MainVertical,
    LayoutPreset::Tiled,
];

impl LayoutPreset {
    fn label(self) -> &'static str {
        match self {
            Self::EvenHorizontal => "even-horizontal",
            Self::EvenVertical => "even-vertical",
            Self::MainHorizontal => "main-horizontal",
            Self::MainVertical => "main-vertical",
            Self::Tiled => "tiled",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "even-horizontal" => Self::EvenHorizontal,
            "even-vertical" => Self::EvenVertical,
            "main-horizontal" => Self::MainHorizontal,
            "main-vertical" | "main-vert" => Self::MainVertical,
            "tiled" => Self::Tiled,
            _ => Self::Tiled,
        }
    }

    fn next(self) -> Self {
        let idx = ALL_PRESETS.iter().position(|p| *p == self).unwrap_or(0);
        ALL_PRESETS[(idx + 1) % ALL_PRESETS.len()]
    }

    fn prev(self) -> Self {
        let idx = ALL_PRESETS.iter().position(|p| *p == self).unwrap_or(0);
        ALL_PRESETS[(idx + ALL_PRESETS.len() - 1) % ALL_PRESETS.len()]
    }

    /// Compute rects for N slots within the given area.
    fn slot_rects(self, count: usize, area: Rect) -> Vec<Rect> {
        if count == 0 || area.width < 2 || area.height < 2 {
            return Vec::new();
        }
        match self {
            Self::EvenHorizontal => split_h_equal(count, area),
            Self::EvenVertical => split_v_equal(count, area),
            Self::MainHorizontal => {
                if count == 1 {
                    return vec![area];
                }
                let rows = Flex::vertical()
                    .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
                    .split(area);
                let mut rects = vec![rows[0]];
                rects.extend(split_h_equal(count - 1, rows[1]));
                rects
            }
            Self::MainVertical => {
                if count == 1 {
                    return vec![area];
                }
                let cols = Flex::horizontal()
                    .constraints([Constraint::Percentage(60.0), Constraint::Percentage(40.0)])
                    .split(area);
                let mut rects = vec![cols[0]];
                rects.extend(split_v_equal(count - 1, cols[1]));
                rects
            }
            Self::Tiled => {
                let cols = (count as f64).sqrt().ceil() as usize;
                let rows = (count + cols - 1) / cols;
                let row_rects = split_v_equal(rows, area);
                let mut rects = Vec::with_capacity(count);
                let mut remaining = count;
                for row_rect in row_rects {
                    let n = remaining.min(cols);
                    rects.extend(split_h_equal(n, row_rect));
                    remaining -= n;
                }
                rects
            }
        }
    }
}

fn split_h_equal(n: usize, area: Rect) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n as u32)).collect();
    Flex::horizontal().constraints(constraints).split(area).to_vec()
}

fn split_v_equal(n: usize, area: Rect) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n as u32)).collect();
    Flex::vertical().constraints(constraints).split(area).to_vec()
}

// ---------------------------------------------------------------------------
// Focus / Input
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Configs,
    Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    NewConfig,
    SessionName,
    Root,
}

// ---------------------------------------------------------------------------
// Slot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Slot {
    /// Display label (from config window name, or "slot N").
    label: String,
    /// Assigned tmux session name.
    session: Option<String>,
    /// GT address for `gt session start`.
    gt_address: Option<String>,
    /// Pane configs from template (commands, session links, etc.).
    pane_configs: Vec<PaneConfig>,
}

/// Merged entry from agents + live tmux sessions.
#[derive(Debug, Clone)]
struct AssignableSession {
    session: String,
    address: Option<String>,
    running: bool,
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

pub struct LayoutManagerScreen {
    pub configs: Vec<TmuxrsConfig>,
    pub tmuxrs_available: bool,
    agents: Vec<AgentInfo>,
    tmux_sessions: Vec<String>,

    // Config list
    selected_config: usize,

    // Visual layout
    preset: LayoutPreset,
    slots: Vec<Slot>,
    selected_slot: usize,

    root_override: Option<String>,
    focus: Focus,
    input_mode: InputMode,
    input_buf: String,
    pending_confirm: Option<(String, u64)>,
    feedback: Option<(String, u64)>,
    tick_count: u64,
}

impl LayoutManagerScreen {
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            tmuxrs_available: false,
            agents: Vec::new(),
            tmux_sessions: Vec::new(),
            selected_config: 0,
            preset: LayoutPreset::Tiled,
            slots: vec![
                Slot { label: "slot 1".into(), session: None, gt_address: None, pane_configs: Vec::new() },
                Slot { label: "slot 2".into(), session: None, gt_address: None, pane_configs: Vec::new() },
            ],
            selected_slot: 0,
            root_override: None,
            focus: Focus::Layout,
            input_mode: InputMode::None,
            input_buf: String::new(),
            pending_confirm: None,
            feedback: None,
            tick_count: 0,
        }
    }

    pub fn set_configs(&mut self, configs: Vec<TmuxrsConfig>) {
        let prev_name = self.configs.get(self.selected_config).map(|c| c.name.clone());
        self.configs = configs;
        if !self.configs.is_empty() && self.selected_config >= self.configs.len() {
            self.selected_config = self.configs.len() - 1;
        }
        if let Some(name) = prev_name {
            if let Some(idx) = self.configs.iter().position(|c| c.name == name) {
                self.selected_config = idx;
            }
        }
    }

    pub fn set_agents(&mut self, agents: Vec<AgentInfo>) {
        self.agents = agents;
    }

    pub fn set_tmux_sessions(&mut self, sessions: Vec<String>) {
        self.tmux_sessions = sessions;
    }

    /// Load a config template into the visual layout.
    /// Converts the raw TmuxrsConfig into a pure-geometry Layout, stripping
    /// session references. Sessions are assigned at runtime via the UI.
    fn load_config(&mut self, idx: usize) {
        let config = match self.configs.get(idx) {
            Some(c) => c,
            None => return,
        };

        // Convert to pure-geometry layout
        let layout = Layout::from_config(config);

        // Set preset from first slot (or tiled)
        self.preset = layout
            .slots
            .first()
            .and_then(|s| s.preset.as_deref())
            .map(LayoutPreset::from_str)
            .unwrap_or(LayoutPreset::Tiled);

        // Build slots from layout geometry
        let old_slots = std::mem::take(&mut self.slots);
        self.slots = layout
            .slots
            .iter()
            .map(|ls| {
                let prev = old_slots.iter().find(|s| s.label == ls.label);
                // Build pane configs from default commands (pure geometry, no sessions)
                let pane_configs: Vec<PaneConfig> = if !ls.default_commands.is_empty() {
                    ls.default_commands.iter().map(|c| PaneConfig::from_command(c)).collect()
                } else {
                    (0..ls.pane_count).map(|_| PaneConfig::default()).collect()
                };
                Slot {
                    label: ls.label.clone(),
                    session: prev.and_then(|s| s.session.clone()),
                    gt_address: prev.and_then(|s| s.gt_address.clone()),
                    pane_configs,
                }
            })
            .collect();

        self.root_override = layout.root;
        self.selected_slot = 0;
        self.set_feedback(format!("Loaded: {}", layout.name));
    }

    fn assignable_sessions(&self) -> Vec<AssignableSession> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for agent in &self.agents {
            if agent.session.is_empty() {
                continue;
            }
            let running = self.tmux_sessions.iter().any(|s| s == &agent.session);
            seen.insert(agent.session.clone());
            result.push(AssignableSession {
                session: agent.session.clone(),
                address: if agent.address.is_empty() { None } else { Some(agent.address.clone()) },
                running,
            });
        }

        for session in &self.tmux_sessions {
            if !seen.contains(session) {
                result.push(AssignableSession {
                    session: session.clone(),
                    address: None,
                    running: true,
                });
            }
        }

        result
    }

    fn set_feedback(&mut self, msg: impl Into<String>) {
        self.feedback = Some((msg.into(), self.tick_count + 30));
    }

    pub fn consumes_text_input(&self) -> bool {
        self.input_mode != InputMode::None
    }

    // =====================================================================
    // Key handling
    // =====================================================================

    pub fn handle_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match self.input_mode {
            InputMode::NewConfig => return self.handle_new_config_input(key),
            InputMode::SessionName => return self.handle_session_name_input(key),
            InputMode::Root => return self.handle_root_input(key),
            InputMode::None => {}
        }

        match self.focus {
            Focus::Configs => self.handle_configs_key(key),
            Focus::Layout => self.handle_layout_key(key),
        }
    }

    fn handle_configs_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.configs.is_empty() {
                    self.selected_config =
                        (self.selected_config + 1).min(self.configs.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_config = self.selected_config.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.load_config(self.selected_config);
                self.focus = Focus::Layout;
            }
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
                self.focus = Focus::Layout;
            }
            KeyCode::Char('n') => {
                self.input_mode = InputMode::NewConfig;
                self.input_buf.clear();
            }
            KeyCode::Char('d') => {
                return self.delete_selected_config();
            }
            KeyCode::Char('D') => {
                return self.duplicate_selected_config();
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_layout_key(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            // Navigate slots
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.slots.is_empty() {
                    self.selected_slot = (self.selected_slot + 1) % self.slots.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.slots.is_empty() {
                    self.selected_slot =
                        (self.selected_slot + self.slots.len() - 1) % self.slots.len();
                }
            }
            // Back to config list
            KeyCode::Tab | KeyCode::Char('h') | KeyCode::Left => {
                self.focus = Focus::Configs;
            }
            KeyCode::Escape => {
                self.focus = Focus::Configs;
            }
            // Cycle layout preset
            KeyCode::Char('L') => {
                self.preset = self.preset.next();
                self.set_feedback(format!("Layout: {}", self.preset.label()));
            }
            KeyCode::Char('H') => {
                self.preset = self.preset.prev();
                self.set_feedback(format!("Layout: {}", self.preset.label()));
            }
            // Add / remove slots
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let n = self.slots.len() + 1;
                self.slots.push(Slot {
                    label: format!("slot {n}"),
                    session: None,
                    gt_address: None,
                    pane_configs: Vec::new(),
                });
                self.selected_slot = self.slots.len() - 1;
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if self.slots.len() > 1 {
                    self.slots.remove(self.selected_slot);
                    if self.selected_slot >= self.slots.len() {
                        self.selected_slot = self.slots.len() - 1;
                    }
                }
            }
            // Assign session
            KeyCode::Char('a') | KeyCode::Enter => {
                self.cycle_session_forward();
            }
            KeyCode::Char('A') | KeyCode::Backspace => {
                self.cycle_session_backward();
            }
            // Clear
            KeyCode::Char('x') => {
                if let Some(slot) = self.slots.get_mut(self.selected_slot) {
                    let name = slot.label.clone();
                    slot.session = None;
                    slot.gt_address = None;
                    self.set_feedback(format!("{name}: cleared"));
                }
            }
            // Start session in selected slot
            KeyCode::Char('s') => {
                return self.start_slot_session();
            }
            // Stop session in selected slot
            KeyCode::Char('S') => {
                return self.stop_slot_session();
            }
            // Root override
            KeyCode::Char('R') => {
                self.input_mode = InputMode::Root;
                self.input_buf = self.root_override.clone().unwrap_or_default();
            }
            // Go
            KeyCode::Char('G') => {
                self.input_mode = InputMode::SessionName;
                self.input_buf = self
                    .configs
                    .get(self.selected_config)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "layout".into());
            }
            _ => {}
        }
        Cmd::None
    }

    fn cycle_session_forward(&mut self) {
        let assignable = self.assignable_sessions();
        if assignable.is_empty() {
            self.set_feedback("No sessions available");
            return;
        }

        let slot = match self.slots.get_mut(self.selected_slot) {
            Some(s) => s,
            None => return,
        };

        let current_idx = slot
            .session
            .as_ref()
            .and_then(|s| assignable.iter().position(|a| &a.session == s));

        let next_idx = match current_idx {
            Some(i) if i + 1 < assignable.len() => Some(i + 1),
            Some(_) => None,
            None => Some(0),
        };

        let slot_label = slot.label.clone();
        if let Some(idx) = next_idx {
            let a = &assignable[idx];
            let status = if a.running { "" } else { " (will start)" };
            slot.session = Some(a.session.clone());
            slot.gt_address = a.address.clone();
            self.set_feedback(format!("{slot_label}: {}{status}", a.session));
        } else {
            slot.session = None;
            slot.gt_address = None;
            self.set_feedback(format!("{slot_label}: (none)"));
        }
    }

    fn cycle_session_backward(&mut self) {
        let assignable = self.assignable_sessions();
        if assignable.is_empty() {
            self.set_feedback("No sessions available");
            return;
        }

        let slot = match self.slots.get_mut(self.selected_slot) {
            Some(s) => s,
            None => return,
        };

        let current_idx = slot
            .session
            .as_ref()
            .and_then(|s| assignable.iter().position(|a| &a.session == s));

        let next_idx = match current_idx {
            Some(0) => None,
            Some(i) => Some(i - 1),
            None => Some(assignable.len() - 1),
        };

        let slot_label = slot.label.clone();
        if let Some(idx) = next_idx {
            let a = &assignable[idx];
            let status = if a.running { "" } else { " (will start)" };
            slot.session = Some(a.session.clone());
            slot.gt_address = a.address.clone();
            self.set_feedback(format!("{slot_label}: {}{status}", a.session));
        } else {
            slot.session = None;
            slot.gt_address = None;
            self.set_feedback(format!("{slot_label}: (none)"));
        }
    }

    // ----- Apply -----

    fn apply_layout(&self, session_name: String) -> Cmd<Msg> {
        let slots = self.slots.clone();
        let preset = self.preset;
        let effective_root = self.root_override.clone();

        Cmd::Task(
            Default::default(),
            Box::new(move || {
                let result = do_apply_layout(
                    &session_name,
                    &slots,
                    preset,
                    effective_root.as_deref(),
                );
                Msg::TmuxrsActionResult(format!("apply {session_name}"), result)
            }),
        )
    }

    // ----- Input handlers -----

    fn handle_new_config_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => { self.input_mode = InputMode::None; }
            KeyCode::Enter => {
                let name = self.input_buf.clone();
                self.input_buf.clear();
                self.input_mode = InputMode::None;
                if !name.is_empty() {
                    let yaml = TmuxrsConfig::skeleton_yaml(&name);
                    let name_clone = name.clone();
                    return Cmd::Task(
                        Default::default(),
                        Box::new(move || {
                            let result = tmuxrs::cli::write_config(&name_clone, &yaml)
                                .map(|_| format!("Created config: {name_clone}"));
                            match result {
                                Ok(msg) => Msg::TmuxrsActionResult("new-config".into(), Ok(msg)),
                                Err(e) => Msg::TmuxrsActionResult("new-config".into(), Err(e)),
                            }
                        }),
                    );
                }
            }
            KeyCode::Backspace => { self.input_buf.pop(); }
            KeyCode::Char(c) => { self.input_buf.push(c); }
            _ => {}
        }
        Cmd::None
    }

    fn handle_session_name_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => { self.input_mode = InputMode::None; }
            KeyCode::Enter => {
                let name = self.input_buf.clone();
                self.input_buf.clear();
                self.input_mode = InputMode::None;
                if !name.is_empty() {
                    return self.apply_layout(name);
                }
            }
            KeyCode::Backspace => { self.input_buf.pop(); }
            KeyCode::Char(c) => { self.input_buf.push(c); }
            _ => {}
        }
        Cmd::None
    }

    fn handle_root_input(&mut self, key: &KeyEvent) -> Cmd<Msg> {
        match key.code {
            KeyCode::Escape => { self.input_mode = InputMode::None; }
            KeyCode::Enter => {
                let val = self.input_buf.clone();
                self.input_buf.clear();
                self.input_mode = InputMode::None;
                if val.is_empty() {
                    self.root_override = None;
                    self.set_feedback("Root: cleared");
                } else {
                    self.set_feedback(format!("Root: {val}"));
                    self.root_override = Some(val);
                }
            }
            KeyCode::Backspace => { self.input_buf.pop(); }
            KeyCode::Char(c) => { self.input_buf.push(c); }
            _ => {}
        }
        Cmd::None
    }

    fn delete_selected_config(&mut self) -> Cmd<Msg> {
        let config = match self.configs.get(self.selected_config) {
            Some(c) => c.name.clone(),
            None => return Cmd::None,
        };
        let label = format!("delete {config}");
        if let Some((ref pending, tick)) = self.pending_confirm {
            if pending == &label && self.tick_count.saturating_sub(tick) < 30 {
                self.pending_confirm = None;
                return Cmd::Task(
                    Default::default(),
                    Box::new(move || {
                        let result = tmuxrs::cli::delete_config(&config)
                            .map(|_| format!("Deleted: {config}"));
                        match result {
                            Ok(msg) => Msg::TmuxrsActionResult("delete".into(), Ok(msg)),
                            Err(e) => Msg::TmuxrsActionResult("delete".into(), Err(e)),
                        }
                    }),
                );
            }
        }
        self.pending_confirm = Some((label.clone(), self.tick_count));
        self.set_feedback(format!("Press d again to confirm: {label}"));
        Cmd::None
    }

    fn start_slot_session(&mut self) -> Cmd<Msg> {
        let slot = match self.slots.get(self.selected_slot) {
            Some(s) => s,
            None => return Cmd::None,
        };
        let address = match &slot.gt_address {
            Some(a) => a.clone(),
            None => {
                self.set_feedback("No GT address for this slot");
                return Cmd::None;
            }
        };
        let label = slot.label.clone();
        self.set_feedback(format!("Starting: {address}"));
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                match gt_session_start(&address) {
                    Ok(_) => Msg::TmuxrsActionResult(
                        format!("start {label}"),
                        Ok(format!("Started: {address}")),
                    ),
                    Err(e) => Msg::TmuxrsActionResult(
                        format!("start {label}"),
                        Err(e),
                    ),
                }
            }),
        )
    }

    fn stop_slot_session(&mut self) -> Cmd<Msg> {
        let slot = match self.slots.get(self.selected_slot) {
            Some(s) => s,
            None => return Cmd::None,
        };
        let session = match &slot.session {
            Some(s) => s.clone(),
            None => {
                self.set_feedback("No session assigned");
                return Cmd::None;
            }
        };
        let label = slot.label.clone();
        self.set_feedback(format!("Stopping: {session}"));
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                match tmux_client::kill_session(&session) {
                    Ok(_) => Msg::TmuxrsActionResult(
                        format!("stop {label}"),
                        Ok(format!("Stopped: {session}")),
                    ),
                    Err(e) => Msg::TmuxrsActionResult(
                        format!("stop {label}"),
                        Err(format!("stop {session}: {e}")),
                    ),
                }
            }),
        )
    }

    fn duplicate_selected_config(&mut self) -> Cmd<Msg> {
        let config = match self.configs.get(self.selected_config) {
            Some(c) => c,
            None => return Cmd::None,
        };
        let src_name = config.name.clone();
        let new_name = format!("{}-copy", src_name);
        // Read the source config, write it with a new name
        let new_name_clone = new_name.clone();
        let src_name_clone = src_name.clone();
        Cmd::Task(
            Default::default(),
            Box::new(move || {
                match tmuxrs::cli::read_config(&src_name_clone) {
                    Ok(content) => {
                        // Replace the name line
                        let new_content = content.replacen(
                            &format!("name: {src_name_clone}"),
                            &format!("name: {new_name_clone}"),
                            1,
                        );
                        match tmuxrs::cli::write_config(&new_name_clone, &new_content) {
                            Ok(_) => Msg::TmuxrsActionResult(
                                "duplicate".into(),
                                Ok(format!("Duplicated: {src_name_clone} → {new_name_clone}")),
                            ),
                            Err(e) => Msg::TmuxrsActionResult("duplicate".into(), Err(e)),
                        }
                    }
                    Err(e) => Msg::TmuxrsActionResult(
                        "duplicate".into(),
                        Err(format!("read {src_name_clone}: {e}")),
                    ),
                }
            }),
        )
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

    // =====================================================================
    // Rendering
    // =====================================================================

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let has_input = self.input_mode != InputMode::None;

        // Top-level: [main area] [optional input bar]
        let outer_constraints = if has_input {
            vec![Constraint::Min(6), Constraint::Fixed(3)]
        } else {
            vec![Constraint::Min(6)]
        };
        let outer = Flex::vertical().constraints(outer_constraints).split(area);

        // Main area: [config list 25%] [visual layout 75%]
        let columns = Flex::horizontal()
            .constraints([Constraint::Percentage(25.0), Constraint::Percentage(75.0)])
            .split(outer[0]);

        self.render_config_list(frame, columns[0]);
        self.render_visual_layout(frame, columns[1]);

        if has_input && outer.len() > 1 {
            self.render_input_bar(frame, outer[1]);
        }
    }

    fn render_config_list(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == Focus::Configs;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        let block = Block::new()
            .title(" Formulas ")
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

        if self.configs.is_empty() {
            lines.push(Line::styled("No configs", Style::new().fg(theme::fg::DISABLED)));
            lines.push(Line::styled("[n] create", Style::new().fg(theme::fg::MUTED)));
        } else {
            for (i, config) in self.configs.iter().enumerate() {
                let is_sel = i == self.selected_config;
                let win_count = config.windows.len();
                let label = format!(
                    " {} ({win_count}w)",
                    config.name,
                );

                if is_sel && focused {
                    lines.push(Line::styled(
                        label,
                        Style::new().fg(theme::bg::DEEP).bg(theme::accent::PRIMARY).bold(),
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

        lines.push(Line::raw(""));
        if focused {
            lines.push(Line::from_spans([
                Span::styled("[n]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("ew ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[d]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("el ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[D]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("up", Style::new().fg(theme::fg::MUTED)),
            ]));
            lines.push(Line::from_spans([
                Span::styled("Enter", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled(" load", Style::new().fg(theme::fg::MUTED)),
            ]));
        }

        // Geometry preview of selected config
        if let Some(config) = self.configs.get(self.selected_config) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "Windows:",
                Style::new().fg(theme::fg::SECONDARY).bold(),
            ));
            for win in &config.windows {
                let layout_hint = win.layout.as_deref().unwrap_or("tiled");
                let pane_count = win.panes.len();
                lines.push(Line::from_spans([
                    Span::styled(
                        format!("  {} ", win.name),
                        Style::new().fg(theme::accent::INFO),
                    ),
                    Span::styled(
                        format!("{}p {layout_hint}", pane_count),
                        Style::new().fg(theme::fg::MUTED),
                    ),
                ]));
            }
        }

        // Feedback
        if let Some((ref msg, _)) = self.feedback {
            lines.push(Line::raw(""));
            lines.push(Line::styled(msg.as_str(), Style::new().fg(theme::accent::WARNING)));
        }
        if let Some((ref label, _)) = self.pending_confirm {
            lines.push(Line::styled(
                format!("Press d: {label}"),
                Style::new().fg(theme::accent::ERROR).bold(),
            ));
        }

        Paragraph::new(Text::from_lines(lines))
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(inner, frame);
    }

    fn render_visual_layout(&self, frame: &mut Frame, area: Rect) {
        let focused = self.focus == Focus::Layout;
        let border_style = if focused {
            crate::theme::panel_border_focused()
        } else {
            crate::theme::panel_border_style()
        };

        // Header with layout name and root
        let preset_label = self.preset.label();
        let root_label = self.root_override.as_deref().unwrap_or("(no root)");
        let header = format!(" {preset_label}  root: {root_label} ");

        let block = Block::new()
            .title(&*header)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(crate::theme::panel_bg())
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.width < 6 || inner.height < 4 {
            return;
        }

        // Reserve bottom 2 rows for hints
        let hint_height = if focused { 2u16 } else { 0 };
        let layout_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(hint_height),
        };
        let hint_area = Rect {
            x: inner.x,
            y: inner.y + layout_area.height,
            width: inner.width,
            height: hint_height.min(inner.height),
        };

        if self.slots.is_empty() {
            Paragraph::new("Press + to add a slot")
                .style(Style::new().fg(theme::fg::DISABLED))
                .render(layout_area, frame);
        } else {
            // Compute rects for each slot
            let rects = self.preset.slot_rects(self.slots.len(), layout_area);

            for (i, slot) in self.slots.iter().enumerate() {
                let rect = match rects.get(i) {
                    Some(r) => *r,
                    None => continue,
                };
                if rect.width < 3 || rect.height < 2 {
                    continue;
                }

                let is_selected = focused && i == self.selected_slot;

                // Slot border
                let slot_border = if is_selected {
                    Style::new().fg(theme::accent::PRIMARY).bold()
                } else if slot.session.is_some() {
                    Style::new().fg(theme::accent::SUCCESS)
                } else {
                    Style::new().fg(theme::fg::MUTED)
                };

                let slot_block = Block::new()
                    .borders(Borders::ALL)
                    .border_type(if is_selected { BorderType::Double } else { BorderType::Rounded })
                    .border_style(slot_border)
                    .style(crate::theme::panel_bg());

                let slot_inner = slot_block.inner(rect);
                slot_block.render(rect, frame);

                if slot_inner.width < 2 || slot_inner.height < 1 {
                    continue;
                }

                // Content inside the box
                let mut lines: Vec<Line> = Vec::new();

                // Slot label
                lines.push(Line::styled(
                    &slot.label,
                    Style::new().fg(theme::fg::MUTED).italic(),
                ));

                // Session or empty
                match &slot.session {
                    Some(name) => {
                        let running = self.tmux_sessions.iter().any(|s| s == name);
                        let has_addr = slot.gt_address.is_some();
                        if running {
                            lines.push(Line::styled(
                                name.as_str(),
                                Style::new().fg(theme::accent::SUCCESS).bold(),
                            ));
                        } else if has_addr {
                            lines.push(Line::styled(
                                name.as_str(),
                                Style::new().fg(theme::accent::WARNING).bold(),
                            ));
                            lines.push(Line::styled(
                                "will start",
                                Style::new().fg(theme::accent::WARNING),
                            ));
                        } else {
                            lines.push(Line::styled(
                                name.as_str(),
                                Style::new().fg(theme::accent::ERROR),
                            ));
                            lines.push(Line::styled(
                                "not running",
                                Style::new().fg(theme::accent::ERROR),
                            ));
                        }
                    }
                    None => {
                        let labels: Vec<String> = slot.pane_configs.iter()
                            .map(|p| p.label())
                            .filter(|l| l != "(empty)" && l != "(shell)")
                            .collect();
                        if !labels.is_empty() {
                            for lbl in &labels {
                                let display = if lbl.len() > slot_inner.width as usize - 1 {
                                    format!("{}...", &lbl[..slot_inner.width as usize - 4])
                                } else {
                                    lbl.clone()
                                };
                                lines.push(Line::styled(
                                    display,
                                    Style::new().fg(theme::fg::SECONDARY),
                                ));
                            }
                        } else {
                            lines.push(Line::styled(
                                "(empty)",
                                Style::new().fg(theme::fg::DISABLED),
                            ));
                        }
                    }
                }

                // Truncate lines to fit
                lines.truncate(slot_inner.height as usize);

                Paragraph::new(Text::from_lines(lines))
                    .style(Style::new().fg(theme::fg::PRIMARY))
                    .render(slot_inner, frame);
            }
        }

        // Hints bar
        if focused && hint_area.height > 0 {
            let hints = Line::from_spans([
                Span::styled("[a]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("ssign ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[x]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("clear ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[+]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("add ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[-]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("rm ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[L]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("ayout ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[s]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("tart ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[S]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("top ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[R]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("oot ", Style::new().fg(theme::fg::MUTED)),
                Span::styled("[G]", Style::new().fg(theme::accent::PRIMARY).bold()),
                Span::styled("o", Style::new().fg(theme::fg::MUTED)),
            ]);

            Paragraph::new(Text::from_lines(vec![hints]))
                .style(Style::new().fg(theme::fg::MUTED))
                .render(hint_area, frame);
        }
    }

    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        let title = match self.input_mode {
            InputMode::NewConfig => " New Config Name: ",
            InputMode::SessionName => " Session Name: ",
            InputMode::Root => " Root Directory: ",
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

// =========================================================================
// Apply logic — runs in a Cmd::Task (blocking, off main thread)
// =========================================================================

fn do_apply_layout(
    session_name: &str,
    slots: &[Slot],
    preset: LayoutPreset,
    root: Option<&str>,
) -> Result<String, String> {
    let expanded_root = root.map(|r| {
        if r.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ubuntu".into());
            format!("{}{}", home, &r[1..])
        } else {
            r.to_string()
        }
    });

    // Nesting detection: warn if we're already inside tmux
    let nested = std::env::var("TMUX").is_ok();

    // Kill existing session
    if tmux_client::has_session(session_name) {
        tmux_client::kill_session(session_name)
            .map_err(|e| format!("kill old session: {e}"))?;
    }

    // Create new session
    if let Some(ref dir) = expanded_root {
        new_session_with_dir(session_name, dir)?;
    } else {
        tmux_client::new_session(session_name, true)
            .map_err(|e| format!("new-session: {e}"))?;
    }

    let mut linked_count = 0u32;
    let mut window_count = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for (i, slot) in slots.iter().enumerate() {
        if i > 0 {
            if let Some(ref dir) = expanded_root {
                new_window_with_dir(session_name, &slot.label, dir)
                    .map_err(|e| format!("new-window {}: {e}", slot.label))?;
            } else {
                tmux_client::new_window(session_name, Some(&slot.label))
                    .map_err(|e| format!("new-window {}: {e}", slot.label))?;
            }
        } else {
            // Rename the initial window (always index 0 for fresh sessions)
            let target = format!("{session_name}:0");
            let _ = tmux_client::rename_window(&target, &slot.label);
        }
        window_count += 1;

        // Find the actual window index we just created (don't assume sequential)
        let win_target = find_window_by_name(session_name, &slot.label)
            .unwrap_or_else(|| format!("{session_name}:{i}"));

        if let Some(ref src_session) = slot.session {
            // Start the session if it's not running
            if !tmux_client::has_session(src_session) {
                if let Some(ref address) = slot.gt_address {
                    let _ = gt_session_start(address);
                    // Wait up to 10s for session to appear
                    for _ in 0..20 {
                        if tmux_client::has_session(src_session) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }

            // Link the session's window into our layout
            if tmux_client::has_session(src_session) {
                if nested {
                    errors.push(format!("{src_session}: nested tmux, link may fail"));
                }
                let source = format!("{src_session}:0");
                match tmux_client::link_window(&source, session_name) {
                    Ok(idx) => {
                        let linked_target = format!("{session_name}:{idx}");
                        let _ = tmux_client::rename_window(&linked_target, src_session);
                        // Kill the placeholder window we created
                        let _ = tmux_client::kill_window(&win_target);
                        linked_count += 1;
                    }
                    Err(e) => {
                        errors.push(format!("link {src_session}: {e}"));
                    }
                }
            } else {
                let msg = if slot.gt_address.is_some() {
                    format!("Failed to start: {src_session}")
                } else {
                    format!("Not found: {src_session}")
                };
                errors.push(msg);
            }
        } else {
            // Plain window — apply layout preset and run pane commands
            let _ = tmux_client::select_layout(&win_target, preset.label());

            for (p, pane) in slot.pane_configs.iter().enumerate() {
                if p > 0 {
                    let horizontal = pane.direction.as_deref() == Some("horizontal");
                    let _ = tmux_client::split_pane(&win_target, horizontal);
                    let _ = tmux_client::select_layout(&win_target, preset.label());
                }
                if let Some(ref cmd) = pane.command {
                    if !cmd.is_empty() {
                        let _ = tmux_client::send_keys(&win_target, cmd);
                    }
                }
            }
        }
    }

    let mut summary = format!(
        "Created '{session_name}': {window_count} windows, {linked_count} linked"
    );
    if nested {
        summary.push_str(" (nested tmux)");
    }
    if !errors.is_empty() {
        summary.push_str(&format!("\nWarnings: {}", errors.join("; ")));
    }
    Ok(summary)
}

/// Find a window by name, returning its proper target string.
fn find_window_by_name(session: &str, name: &str) -> Option<String> {
    if let Ok(windows) = tmux_client::list_windows(session) {
        for win in &windows {
            if win.name == name {
                return Some(format!("{session}:{}", win.index));
            }
        }
    }
    None
}

fn new_session_with_dir(name: &str, dir: &str) -> Result<(), String> {
    let output = std::process::Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c", dir])
        .output()
        .map_err(|e| format!("new-session: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("new-session: {}", stderr.trim()))
    }
}

fn gt_session_start(address: &str) -> Result<(), String> {
    let output = std::process::Command::new("gt")
        .args(["session", "start", address])
        .output()
        .map_err(|e| format!("gt session start: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("gt session start {address}: {}", stderr.trim()))
    }
}

fn new_window_with_dir(session: &str, name: &str, dir: &str) -> Result<(), String> {
    let target = format!("{session}:");
    let output = std::process::Command::new("tmux")
        .args(["new-window", "-t", &target, "-n", name, "-c", dir])
        .output()
        .map_err(|e| format!("new-window: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("new-window: {}", stderr.trim()))
    }
}
