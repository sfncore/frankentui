use ftui_core::geometry::Rect;
use ftui_render::frame::Frame;
use ftui_widgets::block::Block;
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::tree::{Tree, TreeGuides, TreeNode};
use ftui_widgets::Widget;

use crate::data::TownStatus;
use crate::theme;

/// Build the agent tree from town status data.
pub fn build_tree(status: &TownStatus) -> TreeNode {
    let town_label = if status.name.is_empty() {
        "Gas Town".to_string()
    } else {
        status.name.clone()
    };

    let mut root = TreeNode::new(format!(" {}", town_label));

    // Town-level agents (mayor, deacon)
    for agent in &status.agents {
        let indicator = status_indicator(agent.running, &agent.state);
        let session_hint = if agent.session.is_empty() {
            String::new()
        } else {
            format!(" [{}]", agent.session)
        };
        let mail_hint = if agent.unread_mail > 0 {
            format!(" ({})", agent.unread_mail)
        } else {
            String::new()
        };
        root = root.child(TreeNode::new(format!(
            "{} {}{}{}", indicator, agent.name, session_hint, mail_hint
        )));
    }

    if status.rigs.is_empty() {
        root = root.child(TreeNode::new("  (no rigs)"));
        return root;
    }

    for rig in &status.rigs {
        let mut rig_node = TreeNode::new(format!(" {}", rig.name));

        for agent in &rig.agents {
            let indicator = status_indicator(agent.running, &agent.state);
            let session_hint = if agent.session.is_empty() {
                String::new()
            } else {
                format!(" [{}]", agent.session)
            };
            let work_hint = if agent.has_work { " " } else { "" };
            let mail_hint = if agent.unread_mail > 0 {
                format!(" ({})", agent.unread_mail)
            } else {
                String::new()
            };
            rig_node = rig_node.child(TreeNode::new(format!(
                "{} {}{}{}{}", indicator, agent.name, work_hint, session_hint, mail_hint
            )));
        }

        if rig.polecat_count > 0 {
            rig_node = rig_node.child(TreeNode::new(format!(
                "  {} polecats", rig.polecat_count
            )));
        }

        root = root.child(rig_node);
    }

    root
}

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

pub fn render(frame: &mut Frame, area: Rect, status: &TownStatus, focused: bool) {
    let border_style = if focused {
        theme::panel_border_focused()
    } else {
        theme::panel_border_style()
    };

    let block = Block::new()
        .title(" Agents ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(theme::panel_bg())
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, frame);

    let tree_root = build_tree(status);
    let tree = Tree::new(tree_root)
        .with_guides(TreeGuides::Rounded)
        .with_guide_style(theme::panel_border_style())
        .with_label_style(theme::event_default());

    tree.render(inner, frame);
}
