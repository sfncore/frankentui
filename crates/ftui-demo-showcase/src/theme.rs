#![forbid(unsafe_code)]

//! Shared theme styles for the demo showcase, backed by ftui-extras themes.

use ftui_extras::theme as core_theme;
use ftui_style::{Style, StyleFlags};

pub use core_theme::{
    AlphaColor, ColorToken, IntentStyles, IssueTypeStyles, PriorityStyles, SemanticStyles,
    SemanticSwatch, StatusStyles, ThemeId, accent, alpha, bg, blend_colors, blend_over, contrast,
    current_theme, current_theme_name, cycle_theme, fg, intent, issue_type, priority,
    semantic_styles, status, syntax, syntax_theme, theme_count, with_alpha, with_opacity,
};
pub use core_theme::{palette, set_theme};

/// Per-screen accent colors for visual distinction.
pub mod screen_accent {
    use super::{ColorToken, accent};

    pub const DASHBOARD: ColorToken = accent::ACCENT_1;
    pub const SHAKESPEARE: ColorToken = accent::ACCENT_4;
    pub const CODE_EXPLORER: ColorToken = accent::ACCENT_3;
    pub const WIDGET_GALLERY: ColorToken = accent::ACCENT_2;
    pub const LAYOUT_LAB: ColorToken = accent::ACCENT_8;
    pub const FORMS_INPUT: ColorToken = accent::ACCENT_6;
    pub const DATA_VIZ: ColorToken = accent::ACCENT_4;
    pub const FILE_BROWSER: ColorToken = accent::ACCENT_7;
    pub const ADVANCED: ColorToken = accent::ACCENT_5;
    pub const PERFORMANCE: ColorToken = accent::ACCENT_10;
    pub const MARKDOWN: ColorToken = accent::ACCENT_11;
    pub const VISUAL_EFFECTS: ColorToken = accent::ACCENT_12;
}

// ---------------------------------------------------------------------------
// Named styles
// ---------------------------------------------------------------------------

/// Semantic text styles.
pub fn title() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BOLD)
}

pub fn subtitle() -> Style {
    Style::new().fg(fg::SECONDARY).attrs(StyleFlags::ITALIC)
}

pub fn body() -> Style {
    Style::new().fg(fg::PRIMARY)
}

pub fn muted() -> Style {
    Style::new().fg(fg::MUTED)
}

pub fn link() -> Style {
    Style::new().fg(accent::LINK).attrs(StyleFlags::UNDERLINE)
}

pub fn code() -> Style {
    Style::new().fg(accent::INFO).bg(alpha::SURFACE)
}

pub fn error_style() -> Style {
    Style::new().fg(accent::ERROR).attrs(StyleFlags::BOLD)
}

pub fn success() -> Style {
    Style::new().fg(accent::SUCCESS).attrs(StyleFlags::BOLD)
}

pub fn warning() -> Style {
    Style::new().fg(accent::WARNING).attrs(StyleFlags::BOLD)
}

// ---------------------------------------------------------------------------
// Attribute showcase styles (exercises every StyleFlags variant)
// ---------------------------------------------------------------------------

pub fn bold() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BOLD)
}

pub fn dim() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::DIM)
}

pub fn italic() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::ITALIC)
}

pub fn underline() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::UNDERLINE)
}

pub fn double_underline() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::DOUBLE_UNDERLINE)
}

pub fn curly_underline() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::CURLY_UNDERLINE)
}

pub fn blink_style() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::BLINK)
}

pub fn reverse() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::REVERSE)
}

pub fn hidden() -> Style {
    Style::new().fg(fg::PRIMARY).attrs(StyleFlags::HIDDEN)
}

pub fn strikethrough() -> Style {
    Style::new()
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::STRIKETHROUGH)
}

// ---------------------------------------------------------------------------
// Component styles
// ---------------------------------------------------------------------------

/// Tab bar background.
pub fn tab_bar() -> Style {
    Style::new().bg(alpha::SURFACE).fg(fg::SECONDARY)
}

/// Active tab.
pub fn tab_active() -> Style {
    Style::new()
        .bg(alpha::HIGHLIGHT)
        .fg(fg::PRIMARY)
        .attrs(StyleFlags::BOLD)
}

/// Status bar background.
pub fn status_bar() -> Style {
    Style::new().bg(alpha::SURFACE).fg(fg::MUTED)
}

/// Content area border.
pub fn content_border() -> Style {
    Style::new().fg(fg::MUTED)
}

/// Help overlay background.
pub fn help_overlay() -> Style {
    Style::new().bg(alpha::OVERLAY).fg(fg::PRIMARY)
}
