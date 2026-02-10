use std::cell::RefCell;

use ftui_core::geometry::Rect;
use ftui_render::frame::Frame;
use ftui_text::Text;
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::log_viewer::{LogViewer, LogViewerState};
use ftui_widgets::{StatefulWidget, Widget};

use crate::data::GtEvent;
use crate::theme;

pub fn push_event(viewer: &mut LogViewer, event: &GtEvent) {
    let style = match event.event_type.as_str() {
        "create" | "created" => theme::event_create(),
        "error" | "fail" | "failed" => theme::event_error(),
        "update" | "updated" | "close" | "closed" => theme::event_update(),
        _ => theme::event_default(),
    };

    let actor = if event.actor.is_empty() {
        String::new()
    } else {
        format!("[{}] ", event.actor)
    };

    let ts = if event.timestamp.is_empty() {
        String::new()
    } else {
        // Show just HH:MM:SS if possible
        let ts = &event.timestamp;
        if ts.len() > 19 {
            format!("{} ", &ts[11..19])
        } else {
            format!("{} ", ts)
        }
    };

    let line = format!("{}{}{}", ts, actor, event.message);
    viewer.push(Text::styled(line, style));
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    viewer: &LogViewer,
    state: &RefCell<LogViewerState>,
    focused: bool,
) {
    let border_style = if focused {
        theme::panel_border_focused()
    } else {
        theme::panel_border_style()
    };

    let block = Block::new()
        .title(" Events ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::panel_bg())
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, frame);

    let mut log_state = state.borrow_mut();
    viewer.render(inner, frame, &mut log_state);
}
