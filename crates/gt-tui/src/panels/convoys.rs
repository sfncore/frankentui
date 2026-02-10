use ftui_core::geometry::Rect;
use ftui_render::frame::Frame;
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::list::{List, ListItem, ListState};
use ftui_widgets::{StatefulWidget, Widget};

use crate::data::ConvoyItem;
use crate::theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    convoys: &[ConvoyItem],
    focused: bool,
    list_state: &mut ListState,
) {
    let border_style = if focused {
        theme::panel_border_focused()
    } else {
        theme::panel_border_style()
    };

    let block = Block::new()
        .title(" Convoys ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::panel_bg())
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, frame);

    if convoys.is_empty() {
        let items: Vec<ListItem> = vec![ListItem::new("  No active convoys")];
        let list = List::new(items)
            .highlight_style(theme::convoy_active());
        StatefulWidget::render(&list, inner, frame, list_state);
        return;
    }

    let labels: Vec<String> = convoys
        .iter()
        .map(|c| {
            let status_icon = if c.landed {
                ""
            } else {
                match c.status.as_str() {
                    "active" | "running" => "",
                    "paused" => "",
                    "failed" => "",
                    _ => "",
                }
            };

            let progress = if c.total > 0 {
                format!(" ({}/{})", c.done, c.total)
            } else if !c.progress.is_empty() {
                format!(" ({})", c.progress)
            } else {
                String::new()
            };

            format!("  {} {}{}", status_icon, c.name, progress)
        })
        .collect();

    let items: Vec<ListItem> = labels.iter().map(|s| ListItem::new(s.as_str())).collect();
    let list = List::new(items)
        .highlight_style(theme::convoy_active());

    StatefulWidget::render(&list, inner, frame, list_state);
}
