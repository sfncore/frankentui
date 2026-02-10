use ftui_core::geometry::Rect;
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::Widget;

use crate::app::TreeEntry;
use crate::data::TownStatus;
use crate::theme;

fn status_indicator(running: bool, state: &str) -> &'static str {
    if running {
        match state {
            "idle" => "",
            "working" | "active" => "",
            "stuck" | "error" => "",
            _ => "",
        }
    } else {
        ""
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    status: &TownStatus,
    focused: bool,
    tree_entries: &[TreeEntry],
    cursor: usize,
) {
    let border_style = if focused {
        theme::panel_border_focused()
    } else {
        theme::panel_border_style()
    };

    let block = Block::new()
        .title(" Agents (j/k Enter) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::panel_bg())
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, frame);

    if tree_entries.is_empty() {
        // Fallback: render plain text tree from status
        render_fallback(frame, inner, status);
        return;
    }

    // Render flat list with indentation and cursor highlight
    let max_rows = inner.height as usize;
    let start = if cursor >= max_rows {
        cursor - max_rows + 1
    } else {
        0
    };

    for (i, entry) in tree_entries.iter().skip(start).take(max_rows).enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }

        let indent = "  ".repeat(entry.depth as usize);
        let prefix = if entry.depth == 0 && entry.tmux_session.is_empty() {
            " "  // Rig header
        } else if !entry.tmux_session.is_empty() {
            "  "
        } else {
            "  "
        };

        let line = format!("{}{}{}", indent, prefix, entry.label);

        let is_selected = (start + i) == cursor;
        let row_area = Rect::new(inner.x, y, inner.width, 1);

        if is_selected && focused {
            // Highlight selected row
            frame.buffer.fill(
                row_area,
                Cell::default()
                    .with_bg(ftui_extras::theme::bg::HIGHLIGHT.into())
                    .with_fg(ftui_extras::theme::fg::PRIMARY.into()),
            );
        }

        // Draw the text
        let max_chars = inner.width as usize;
        let display: String = line.chars().take(max_chars).collect();
        for (j, ch) in display.chars().enumerate() {
            let x = inner.x + j as u16;
            if x < inner.x + inner.width {
                if let Some(cell) = frame.buffer.get_mut(x, y) {
                    let new_cell = if is_selected && focused {
                        Cell::from_char(ch)
                            .with_bg(ftui_extras::theme::bg::HIGHLIGHT.into())
                            .with_fg(ftui_extras::theme::fg::PRIMARY.into())
                    } else if !entry.tmux_session.is_empty() {
                        Cell::from_char(ch)
                            .with_fg(ftui_extras::theme::accent::INFO.into())
                    } else if entry.depth == 0 {
                        Cell::from_char(ch)
                            .with_fg(ftui_extras::theme::accent::PRIMARY.into())
                    } else {
                        Cell::from_char(ch)
                    };
                    *cell = new_cell;
                }
            }
        }
    }
}

/// Fallback tree rendering before status data arrives.
fn render_fallback(frame: &mut Frame, area: Rect, status: &TownStatus) {
    let lines: Vec<String> = if status.rigs.is_empty() {
        vec!["  Loading...".to_string()]
    } else {
        let mut out = Vec::new();
        for agent in &status.agents {
            let ind = status_indicator(agent.running, &agent.state);
            out.push(format!("{} {} [{}]", ind, agent.name, agent.session));
        }
        for rig in &status.rigs {
            out.push(format!(" {}", rig.name));
            for agent in &rig.agents {
                let ind = status_indicator(agent.running, &agent.state);
                out.push(format!("  {} {} [{}]", ind, agent.name, agent.session));
            }
        }
        out
    };

    for (i, line) in lines.iter().take(area.height as usize).enumerate() {
        let y = area.y + i as u16;
        let max_chars = area.width as usize;
        let display: String = line.chars().take(max_chars).collect();
        for (j, ch) in display.chars().enumerate() {
            let x = area.x + j as u16;
            if x < area.x + area.width {
                if let Some(cell) = frame.buffer.get_mut(x, y) {
                    *cell = Cell::from_char(ch);
                }
            }
        }
    }
}
