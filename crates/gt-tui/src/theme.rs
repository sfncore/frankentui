use ftui_extras::theme;
use ftui_style::Style;

pub fn status_bar_style() -> Style {
    Style::new()
        .bg(theme::alpha::OVERLAY)
        .fg(theme::fg::PRIMARY)
}

pub fn panel_border_style() -> Style {
    Style::new().fg(theme::fg::MUTED)
}

pub fn panel_border_focused() -> Style {
    Style::new().fg(theme::accent::PRIMARY)
}

pub fn panel_bg() -> Style {
    Style::new().bg(theme::alpha::SURFACE)
}

pub fn event_create() -> Style {
    Style::new().fg(theme::accent::SUCCESS)
}

pub fn event_error() -> Style {
    Style::new().fg(theme::accent::ERROR)
}

pub fn event_update() -> Style {
    Style::new().fg(theme::accent::INFO)
}

pub fn event_default() -> Style {
    Style::new().fg(theme::fg::SECONDARY)
}

pub fn agent_running() -> Style {
    Style::new().fg(theme::accent::SUCCESS)
}

pub fn agent_idle() -> Style {
    Style::new().fg(theme::fg::MUTED)
}

pub fn agent_stuck() -> Style {
    Style::new().fg(theme::accent::WARNING)
}

pub fn convoy_active() -> Style {
    Style::new().fg(theme::accent::PRIMARY)
}

pub fn convoy_landed() -> Style {
    Style::new().fg(theme::accent::SUCCESS)
}

pub fn keybind_key() -> Style {
    Style::new().fg(theme::accent::INFO)
}

pub fn keybind_action() -> Style {
    Style::new().fg(theme::fg::SECONDARY)
}

pub fn content_border() -> Style {
    Style::new().fg(theme::fg::MUTED)
}

pub fn muted() -> Style {
    Style::new().fg(theme::fg::MUTED)
}

pub fn table_theme_phase(tick_count: u64) -> f32 {
    ((tick_count as f32) * 0.02) % 1.0
}
