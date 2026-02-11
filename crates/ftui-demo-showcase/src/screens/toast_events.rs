#![forbid(unsafe_code)]

//! Toast Events screen — real-time toast notifications for Gas Town events.
//!
//! Demonstrates toast notifications triggered by simulated Gas Town events:
//! new mail (info), polecat complete (success), polecat stuck/zombie (error),
//! escalation (warning), and convoy landed (success).

use std::cell::Cell;
use std::time::Duration;

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::notification_queue::{
    NotificationPriority, NotificationQueue, NotificationStack, QueueConfig,
};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::toast::{Toast, ToastIcon, ToastPosition, ToastStyle};

use super::{HelpEntry, Screen};
use crate::theme;

/// Toast events screen state.
pub struct ToastEventsScreen {
    /// The notification queue managing visible and pending toasts.
    queue: NotificationQueue,
    /// Global tick counter from the app.
    tick_count: u64,
    /// Counter for generating unique toast content.
    event_counter: u64,
    /// Whether auto-generation of events is enabled.
    auto_mode: bool,
    /// Cached info panel area for rendering.
    last_info_area: Cell<Rect>,
    /// Cached toast area for rendering.
    last_toast_area: Cell<Rect>,
}

impl Default for ToastEventsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastEventsScreen {
    /// Create a new toast events screen with BottomRight positioning.
    pub fn new() -> Self {
        Self {
            queue: NotificationQueue::new(
                QueueConfig::new()
                    .max_visible(4)
                    .max_queued(20)
                    .position(ToastPosition::BottomRight),
            ),
            tick_count: 0,
            event_counter: 0,
            auto_mode: false,
            last_info_area: Cell::new(Rect::default()),
            last_toast_area: Cell::new(Rect::default()),
        }
    }

    /// Push a "new mail" info toast.
    fn push_new_mail(&mut self) {
        self.event_counter += 1;
        let toast = Toast::new(format!("New mail #{} received", self.event_counter))
            .icon(ToastIcon::Info)
            .title("New Mail")
            .style_variant(ToastStyle::Info)
            .duration(Duration::from_secs(5));
        self.queue.push(toast, NotificationPriority::Normal);
    }

    /// Push a "polecat complete" success toast.
    fn push_polecat_complete(&mut self) {
        self.event_counter += 1;
        let toast = Toast::new(format!("Polecat #{} finished work", self.event_counter))
            .icon(ToastIcon::Success)
            .title("Polecat Complete")
            .style_variant(ToastStyle::Success)
            .duration(Duration::from_secs(5));
        self.queue.push(toast, NotificationPriority::Normal);
    }

    /// Push a "polecat stuck/zombie" error toast.
    fn push_polecat_stuck(&mut self) {
        self.event_counter += 1;
        let toast = Toast::new(format!("Polecat #{} unresponsive", self.event_counter))
            .icon(ToastIcon::Error)
            .title("Polecat Stuck")
            .style_variant(ToastStyle::Error)
            .duration(Duration::from_secs(5));
        self.queue.push(toast, NotificationPriority::High);
    }

    /// Push an "escalation" warning toast.
    fn push_escalation(&mut self) {
        self.event_counter += 1;
        let toast = Toast::new(format!("Escalation #{}: needs attention", self.event_counter))
            .icon(ToastIcon::Warning)
            .title("Escalation")
            .style_variant(ToastStyle::Warning)
            .duration(Duration::from_secs(5));
        self.queue.push(toast, NotificationPriority::High);
    }

    /// Push a "convoy landed" success toast.
    fn push_convoy_landed(&mut self) {
        self.event_counter += 1;
        let toast = Toast::new(format!("Convoy #{} merged to main", self.event_counter))
            .icon(ToastIcon::Success)
            .title("Convoy Landed")
            .style_variant(ToastStyle::Success)
            .duration(Duration::from_secs(5));
        self.queue.push(toast, NotificationPriority::Normal);
    }

    /// Render the info panel describing available event triggers.
    fn render_info_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Toast Events")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::fg::PRIMARY).bg(theme::bg::DEEP));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let auto_label = if self.auto_mode { "ON" } else { "OFF" };
        let lines = [
            "Gas Town event notifications:",
            "",
            "  m  New mail (info)",
            "  c  Polecat complete (success)",
            "  s  Polecat stuck/zombie (error)",
            "  e  Escalation (warning)",
            "  l  Convoy landed (success)",
            "",
            "  a  Toggle auto-events",
            "  d  Dismiss all",
            "",
            &format!(
                "Queue: {} visible, {} pending",
                self.queue.visible().len(),
                self.queue.pending_count(),
            ),
            &format!("Events fired: {}", self.event_counter),
            &format!("Auto-mode: {auto_label}"),
        ];

        for (i, line) in lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let row_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
            let style = if line.starts_with("  ") && line.len() > 3 {
                Style::new().fg(theme::accent::INFO)
            } else {
                Style::new().fg(theme::fg::MUTED)
            };
            Paragraph::new(*line).style(style).render(row_area, frame);
        }
    }
}

impl Screen for ToastEventsScreen {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match code {
                KeyCode::Char('m') => self.push_new_mail(),
                KeyCode::Char('c') => self.push_polecat_complete(),
                KeyCode::Char('s') => self.push_polecat_stuck(),
                KeyCode::Char('e') => self.push_escalation(),
                KeyCode::Char('l') => self.push_convoy_landed(),
                KeyCode::Char('a') => self.auto_mode = !self.auto_mode,
                KeyCode::Char('d') => self.queue.dismiss_all(),
                _ => {}
            }
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let chunks = Flex::horizontal()
            .constraints([Constraint::Percentage(40.0), Constraint::Min(1)])
            .split(area);

        self.last_info_area.set(chunks[0]);
        self.last_toast_area.set(chunks[1]);
        self.render_info_panel(frame, chunks[0]);

        // Render the notification stack overlay on the right portion
        NotificationStack::new(&self.queue)
            .margin(theme::spacing::INLINE)
            .render(chunks[1], frame);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "m",
                action: "New mail toast",
            },
            HelpEntry {
                key: "c",
                action: "Polecat complete toast",
            },
            HelpEntry {
                key: "s",
                action: "Polecat stuck toast",
            },
            HelpEntry {
                key: "e",
                action: "Escalation toast",
            },
            HelpEntry {
                key: "l",
                action: "Convoy landed toast",
            },
            HelpEntry {
                key: "a",
                action: "Toggle auto-events",
            },
            HelpEntry {
                key: "d",
                action: "Dismiss all",
            },
        ]
    }

    fn tick(&mut self, tick_count: u64) {
        self.tick_count = tick_count;
        // Process queue expiry and promotion
        let _actions = self.queue.tick(Duration::from_millis(100));

        // Auto-generate events every ~3 seconds (30 ticks) when enabled
        if self.auto_mode && tick_count % 30 == 0 {
            match (tick_count / 30) % 5 {
                0 => self.push_new_mail(),
                1 => self.push_polecat_complete(),
                2 => self.push_polecat_stuck(),
                3 => self.push_escalation(),
                4 => self.push_convoy_landed(),
                _ => {}
            }
        }
    }

    fn title(&self) -> &'static str {
        "Toast Events"
    }

    fn tab_label(&self) -> &'static str {
        "Toasts"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn default_state() {
        let screen = ToastEventsScreen::new();
        assert_eq!(screen.tick_count, 0);
        assert_eq!(screen.event_counter, 0);
        assert!(!screen.auto_mode);
        assert!(screen.queue.visible().is_empty());
    }

    #[test]
    fn push_all_event_types() {
        let mut screen = ToastEventsScreen::new();
        screen.push_new_mail();
        screen.push_polecat_complete();
        screen.push_polecat_stuck();
        screen.push_escalation();
        screen.push_convoy_landed();
        assert_eq!(screen.event_counter, 5);
        assert_eq!(screen.queue.stats().total_pushed, 5);
        screen.tick(1);
        assert_eq!(screen.queue.visible().len(), 4);
        assert_eq!(screen.queue.pending_count(), 1);
    }

    #[test]
    fn key_m_triggers_mail() {
        use super::Screen;
        let mut screen = ToastEventsScreen::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('m'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        screen.update(&event);
        assert_eq!(screen.event_counter, 1);
    }

    #[test]
    fn key_a_toggles_auto() {
        use super::Screen;
        let mut screen = ToastEventsScreen::new();
        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: ftui_core::event::Modifiers::NONE,
            kind: KeyEventKind::Press,
        });
        assert!(!screen.auto_mode);
        screen.update(&event);
        assert!(screen.auto_mode);
        screen.update(&event);
        assert!(!screen.auto_mode);
    }

    #[test]
    fn auto_mode_generates_events() {
        use super::Screen;
        let mut screen = ToastEventsScreen::new();
        screen.auto_mode = true;
        screen.tick(30); // Should trigger at tick % 30 == 0
        assert_eq!(screen.event_counter, 1);
    }

    #[test]
    fn render_does_not_panic() {
        use super::Screen;
        let screen = ToastEventsScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        screen.view(&mut frame, Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn render_zero_area_does_not_panic() {
        use super::Screen;
        let screen = ToastEventsScreen::new();
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        screen.view(&mut frame, Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn keybindings_returns_entries() {
        use super::Screen;
        let screen = ToastEventsScreen::new();
        let bindings = screen.keybindings();
        assert_eq!(bindings.len(), 7);
        assert_eq!(bindings[0].key, "m");
    }

    #[test]
    fn title_and_label() {
        use super::Screen;
        let screen = ToastEventsScreen::new();
        assert_eq!(screen.title(), "Toast Events");
        assert_eq!(screen.tab_label(), "Toasts");
    }
}
