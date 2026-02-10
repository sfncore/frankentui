use ftui_core::geometry::Rect;
use ftui_render::frame::Frame;
use ftui_widgets::status_line::{StatusItem, StatusLine};
use ftui_widgets::Widget;

use crate::data::TownStatus;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, status: &TownStatus, tick: u32) {
    let rig_count = format!("{} rigs", status.summary.rig_count);
    let polecats = format!("{} polecats", status.summary.polecat_count);
    let mail = format!("{} mail", status.overseer.unread_mail);

    let town_name = if status.name.is_empty() {
        "Gas Town"
    } else {
        &status.name
    };

    let now = chrono_lite_now();

    let bar = StatusLine::new()
        .style(theme::status_bar_style())
        .separator("  ")
        .left(StatusItem::text(town_name))
        .left(StatusItem::Spinner(tick as usize))
        .center(StatusItem::text(&rig_count))
        .center(StatusItem::text(&polecats))
        .center(StatusItem::text(&mail))
        .right(StatusItem::text(&now));

    bar.render(area, frame);
}

fn chrono_lite_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
}
