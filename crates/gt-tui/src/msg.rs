use ftui_core::event::{Event, KeyEvent, MouseEvent};

use crate::data::{BeadsSnapshot, ConvoyItem, GtEvent, TownStatus};
use crate::screen::ActiveScreen;
use crate::tmux::TmuxSnapshot;
use crate::tmuxrs::TmuxrsConfig;

#[derive(Debug)]
pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEvent),
    #[allow(dead_code)]
    Resize { width: u16, height: u16 },
    StatusRefresh(TownStatus),
    ConvoyRefresh(Vec<ConvoyItem>),
    BeadsRefresh(BeadsSnapshot),
    NewEvent(GtEvent),
    SwitchScreen(ActiveScreen),
    CommandOutput(String, String),
    /// Full tmux server snapshot (sessions -> windows -> panes).
    TmuxSnapshot(TmuxSnapshot),
    /// Result of a tmux action (action_name, result).
    TmuxActionResult(String, Result<(), String>),
    /// List of tmuxrs configs from ~/.config/tmuxrs/.
    TmuxrsConfigList(Vec<TmuxrsConfig>),
    /// Result of a tmuxrs action (action_name, result).
    TmuxrsActionResult(String, Result<String, String>),
    /// Live tmux session names for layout manager.
    TmuxSessionList(Vec<String>),
    Tick,
    Noop,
}

impl From<Event> for Msg {
    fn from(event: Event) -> Self {
        match event {
            Event::Key(key) => Msg::Key(key),
            Event::Mouse(mouse) => Msg::Mouse(mouse),
            Event::Resize { width, height } => Msg::Resize { width, height },
            _ => Msg::Noop,
        }
    }
}
