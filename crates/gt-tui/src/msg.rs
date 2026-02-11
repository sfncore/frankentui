use ftui_core::event::{Event, KeyEvent, MouseEvent};

use crate::data::{BeadsSnapshot, ConvoyItem, GtEvent, TownStatus};
use crate::screen::ActiveScreen;

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
