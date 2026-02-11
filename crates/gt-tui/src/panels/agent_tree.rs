use ftui_core::geometry::Rect;
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::Widget;

use crate::screens::dashboard::TreeEntry;
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
        .title(" Agents (j/k Enter=switch) ")
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

    // Reserve last 2 rows for the selected-entry footer
    let footer_rows: u16 = if inner.height > 4 { 2 } else { 0 };
    let list_height = (inner.height - footer_rows) as usize;

    // Render flat list with indentation and cursor highlight
    let max_rows = list_height;
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
        } else if entry.running {
            " \u{25cf} "  // ● online
        } else {
            " \u{25cb} "  // ○ offline
        };

        let line = if entry.running && !entry.tmux_session.is_empty() {
            format!("{}{}{} [{}]", indent, prefix, entry.label, entry.tmux_session)
        } else {
            format!("{}{}{}", indent, prefix, entry.label)
        };

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
                    } else if entry.running {
                        Cell::from_char(ch)
                            .with_fg(ftui_extras::theme::accent::INFO.into())
                    } else if entry.depth == 0 && entry.tmux_session.is_empty() {
                        // Rig header
                        Cell::from_char(ch)
                            .with_fg(ftui_extras::theme::accent::PRIMARY.into())
                    } else {
                        // Offline agent — dimmed
                        Cell::from_char(ch)
                            .with_fg(ftui_extras::theme::fg::MUTED.into())
                    };
                    *cell = new_cell;
                }
            }
        }
    }

    // Footer: show the exact popup command for the selected entry
    if footer_rows > 0 {
        if let Some(entry) = tree_entries.get(cursor) {
            let footer_y = inner.y + inner.height - footer_rows;
            let w = inner.width as usize;

            let (line1, line2) = if entry.running && !entry.tmux_session.is_empty() {
                (
                    format!(" Enter: switch pane -> {}", entry.tmux_session),
                    format!(" respawn-pane -k … attach -t {}", entry.tmux_session),
                )
            } else if entry.tmux_session.is_empty() && entry.depth == 0 {
                (" (rig header)".to_string(), String::new())
            } else {
                (" (offline — no session)".to_string(), String::new())
            };

            // Draw footer lines
            let footer_color = ftui_extras::theme::fg::DISABLED;
            for (row, text) in [line1, line2].iter().enumerate() {
                let y = footer_y + row as u16;
                if y >= inner.y + inner.height {
                    break;
                }
                let display: String = text.chars().take(w).collect();
                for (j, ch) in display.chars().enumerate() {
                    let x = inner.x + j as u16;
                    if x < inner.x + inner.width {
                        if let Some(cell) = frame.buffer.get_mut(x, y) {
                            *cell = Cell::from_char(ch).with_fg(footer_color.into());
                        }
                    }
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
