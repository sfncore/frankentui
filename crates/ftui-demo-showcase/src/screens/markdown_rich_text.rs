#![forbid(unsafe_code)]

//! Markdown and Rich Text screen — typography and text processing.
//!
//! Demonstrates:
//! - `MarkdownRenderer` with custom `MarkdownTheme`
//! - Text style attributes (bold, italic, underline, etc.)
//! - Unicode text with CJK and emoji in a `Table`
//! - `WrapMode` and `Alignment` cycling

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ftui_core::geometry::Rect;
use ftui_extras::markdown::{MarkdownRenderer, MarkdownTheme};
use ftui_layout::{Constraint, Flex};
use ftui_render::frame::Frame;
use ftui_runtime::Cmd;
use ftui_style::Style;
use ftui_text::WrapMode;
use ftui_text::text::{Line, Span, Text};
use ftui_widgets::Widget;
use ftui_widgets::block::{Alignment, Block};
use ftui_widgets::borders::{BorderType, Borders};
use ftui_widgets::paragraph::Paragraph;
use ftui_widgets::rule::Rule;
use ftui_widgets::table::{Row, Table};

use super::{HelpEntry, Screen};
use crate::theme;

const SAMPLE_MARKDOWN: &str = "\
# FrankenTUI Markdown Demo

## Typography Showcase

This demonstrates the **full range** of *Markdown rendering* capabilities
built into the framework.

### Inline Styles

You can use **bold**, *italic*, ~~strikethrough~~, and `inline code`.
Combine them: ***bold italic***, **`bold code`**, *`italic code`*.

### Links

Visit [FrankenTUI](https://github.com/example/frankentui) for more info.

### Blockquotes

> \"Any sufficiently advanced technology is indistinguishable from magic.\"
> — Arthur C. Clarke

### Code Block

```rust
fn main() {
    println!(\"Hello, FrankenTUI!\");
    let tui = FrankenTUI::new()
        .with_theme(Theme::dark())
        .build();
    tui.run().unwrap();
}
```

### Lists

Unordered:
- Styled text rendering
- Unicode-aware wrapping
- Grapheme cluster support
- CJK and emoji handling

Ordered:
1. Parse markdown with pulldown-cmark
2. Apply theme styles to elements
3. Render styled Text in Paragraph widget

### Headings at Every Level

#### H4: Sub-subsection
##### H5: Minor heading
###### H6: Smallest heading

---

*End of demo document.*
";

const WRAP_MODES: &[WrapMode] = &[
    WrapMode::None,
    WrapMode::Word,
    WrapMode::Char,
    WrapMode::WordChar,
];

const ALIGNMENTS: &[Alignment] = &[Alignment::Left, Alignment::Center, Alignment::Right];

pub struct MarkdownRichText {
    md_scroll: u16,
    rendered_md: Text,
    wrap_index: usize,
    align_index: usize,
}

impl Default for MarkdownRichText {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownRichText {
    pub fn new() -> Self {
        let md_theme = MarkdownTheme {
            h1: Style::new().fg(theme::fg::PRIMARY).bold(),
            h2: Style::new().fg(theme::accent::PRIMARY).bold(),
            h3: Style::new().fg(theme::accent::SECONDARY).bold(),
            h4: Style::new().fg(theme::accent::INFO).bold(),
            h5: Style::new().fg(theme::accent::SUCCESS).bold(),
            h6: Style::new().fg(theme::fg::SECONDARY).bold(),
            code_inline: Style::new()
                .fg(theme::accent::WARNING)
                .bg(theme::bg::SURFACE),
            code_block: Style::new().fg(theme::fg::SECONDARY).bg(theme::bg::SURFACE),
            blockquote: Style::new().fg(theme::fg::MUTED).italic(),
            link: Style::new().fg(theme::accent::LINK).underline(),
            emphasis: Style::new().italic(),
            strong: Style::new().bold(),
            strikethrough: Style::new().strikethrough(),
            list_bullet: Style::new().fg(theme::accent::PRIMARY),
            horizontal_rule: Style::new().fg(theme::fg::MUTED).dim(),
        };
        let renderer = MarkdownRenderer::new(md_theme).rule_width(36);
        let rendered_md = renderer.render(SAMPLE_MARKDOWN);

        Self {
            md_scroll: 0,
            rendered_md,
            wrap_index: 1, // Start at Word
            align_index: 0,
        }
    }

    fn current_wrap(&self) -> WrapMode {
        WRAP_MODES[self.wrap_index]
    }

    fn current_alignment(&self) -> Alignment {
        ALIGNMENTS[self.align_index]
    }

    fn wrap_label(&self) -> &'static str {
        match self.current_wrap() {
            WrapMode::None => "None",
            WrapMode::Word => "Word",
            WrapMode::Char => "Char",
            WrapMode::WordChar => "WordChar",
        }
    }

    fn alignment_label(&self) -> &'static str {
        match self.current_alignment() {
            Alignment::Left => "Left",
            Alignment::Center => "Center",
            Alignment::Right => "Right",
        }
    }

    // ---- Render panels ----

    fn render_markdown_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Markdown Renderer")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::MARKDOWN));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        Paragraph::new(self.rendered_md.clone())
            .wrap(WrapMode::Word)
            .scroll((self.md_scroll, 0))
            .render(inner, frame);
    }

    fn render_style_sampler(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Style Sampler")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::MARKDOWN));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let styles_text = Text::from_lines([
            Line::from_spans([
                Span::styled("Bold", theme::bold()),
                Span::raw("  "),
                Span::styled("Dim", theme::dim()),
                Span::raw("  "),
                Span::styled("Italic", theme::italic()),
                Span::raw("  "),
                Span::styled("Underline", theme::underline()),
            ]),
            Line::from_spans([
                Span::styled("Strikethrough", theme::strikethrough()),
                Span::raw("  "),
                Span::styled("Reverse", theme::reverse()),
                Span::raw("  "),
                Span::styled("Blink", theme::blink_style()),
            ]),
            Line::from_spans([
                Span::styled("Dbl-Underline", theme::double_underline()),
                Span::raw("  "),
                Span::styled("Curly-Underline", theme::curly_underline()),
                Span::raw("  "),
                Span::styled("[Hidden]", theme::hidden()),
            ]),
            Line::new(),
            Line::from_spans([
                Span::styled("Error", theme::error_style()),
                Span::raw("  "),
                Span::styled("Success", theme::success()),
                Span::raw("  "),
                Span::styled("Warning", theme::warning()),
                Span::raw("  "),
                Span::styled("Link", theme::link()),
                Span::raw("  "),
                Span::styled("Code", theme::code()),
            ]),
        ]);

        Paragraph::new(styles_text).render(inner, frame);
    }

    fn render_unicode_table(&self, frame: &mut Frame, area: Rect) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Unicode Showcase")
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::MARKDOWN));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let header =
            Row::new(["Text", "Type", "Cells"]).style(Style::new().fg(theme::fg::PRIMARY).bold());

        let rows = [
            Row::new(["Hello", "ASCII", "5"]),
            Row::new(["\u{4f60}\u{597d}\u{4e16}\u{754c}", "CJK", "8"]),
            Row::new(["\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}", "Hiragana", "10"]),
            Row::new(["\u{1f980}\u{1f525}\u{2728}", "Emoji", "6"]),
            Row::new(["caf\u{e9}", "Latin+accent", "4"]),
            Row::new(["\u{03b1} \u{03b2} \u{03b3} \u{03b4}", "Greek", "7"]),
            Row::new(["\u{2192} \u{2190} \u{2191} \u{2193}", "Arrows", "7"]),
            Row::new(["\u{2588}\u{2593}\u{2592}\u{2591}", "Block el.", "4"]),
        ];

        let widths = [
            Constraint::Min(12),
            Constraint::Min(12),
            Constraint::Fixed(6),
        ];

        Table::new(rows, widths)
            .header(header)
            .style(Style::new().fg(theme::fg::SECONDARY))
            .column_spacing(1)
            .render(inner, frame);
    }

    fn render_wrap_demo(&self, frame: &mut Frame, area: Rect) {
        let title = format!(
            "Wrap: {} | Align: {}",
            self.wrap_label(),
            self.alignment_label()
        );

        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title.as_str())
            .title_alignment(Alignment::Center)
            .style(Style::new().fg(theme::screen_accent::MARKDOWN));

        let inner = block.inner(area);
        block.render(area, frame);

        if inner.is_empty() {
            return;
        }

        let chunks = Flex::vertical()
            .constraints([Constraint::Fixed(1), Constraint::Min(1)])
            .split(inner);

        Paragraph::new("w: cycle wrap | a: cycle alignment")
            .style(theme::muted())
            .render(chunks[0], frame);

        let demo_text = "The quick brown fox jumps over the lazy dog. \
             Supercalifragilisticexpialidocious is quite a long word \
             that tests character-level wrapping behavior. \
             \u{4f60}\u{597d}\u{4e16}\u{754c} contains CJK characters \
             that are double-width. \u{1f980} Ferris says hello!";

        Paragraph::new(demo_text)
            .wrap(self.current_wrap())
            .alignment(self.current_alignment())
            .style(Style::new().fg(theme::fg::PRIMARY))
            .render(chunks[1], frame);
    }
}

impl Screen for MarkdownRichText {
    type Message = Event;

    fn update(&mut self, event: &Event) -> Cmd<Self::Message> {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match code {
                KeyCode::Up => {
                    self.md_scroll = self.md_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.md_scroll = self.md_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    self.md_scroll = self.md_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.md_scroll = self.md_scroll.saturating_add(10);
                }
                KeyCode::Home => {
                    self.md_scroll = 0;
                }
                KeyCode::Char('w') => {
                    self.wrap_index = (self.wrap_index + 1) % WRAP_MODES.len();
                }
                KeyCode::Char('a') => {
                    self.align_index = (self.align_index + 1) % ALIGNMENTS.len();
                }
                _ => {}
            }
        }
        Cmd::None
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let cols = Flex::horizontal()
            .constraints([Constraint::Percentage(50.0), Constraint::Percentage(50.0)])
            .split(area);

        self.render_markdown_panel(frame, cols[0]);

        let right_rows = Flex::vertical()
            .constraints([
                Constraint::Fixed(9),
                Constraint::Fixed(1),
                Constraint::Min(8),
                Constraint::Min(6),
            ])
            .split(cols[1]);

        self.render_style_sampler(frame, right_rows[0]);

        Rule::new()
            .style(Style::new().fg(theme::fg::MUTED).dim())
            .render(right_rows[1], frame);

        self.render_unicode_table(frame, right_rows[2]);
        self.render_wrap_demo(frame, right_rows[3]);
    }

    fn keybindings(&self) -> Vec<HelpEntry> {
        vec![
            HelpEntry {
                key: "\u{2191}/\u{2193}",
                action: "Scroll markdown",
            },
            HelpEntry {
                key: "PgUp/PgDn",
                action: "Scroll fast",
            },
            HelpEntry {
                key: "Home",
                action: "Scroll to top",
            },
            HelpEntry {
                key: "w",
                action: "Cycle wrap mode",
            },
            HelpEntry {
                key: "a",
                action: "Cycle alignment",
            },
        ]
    }

    fn title(&self) -> &'static str {
        "Markdown and Rich Text"
    }

    fn tab_label(&self) -> &'static str {
        "Markdown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: ftui_core::event::Modifiers::empty(),
            kind: KeyEventKind::Press,
        })
    }

    #[test]
    fn initial_state() {
        let screen = MarkdownRichText::new();
        assert_eq!(screen.md_scroll, 0);
        assert_eq!(screen.title(), "Markdown and Rich Text");
        assert_eq!(screen.tab_label(), "Markdown");
    }

    #[test]
    fn markdown_renders_headings() {
        let screen = MarkdownRichText::new();
        let plain: String = screen
            .rendered_md
            .lines()
            .iter()
            .map(|l| l.to_plain_text())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("FrankenTUI Markdown Demo"));
        assert!(plain.contains("Typography Showcase"));
        assert!(plain.contains("Code Block"));
    }

    #[test]
    fn markdown_renders_code_block() {
        let screen = MarkdownRichText::new();
        let plain: String = screen
            .rendered_md
            .lines()
            .iter()
            .map(|l| l.to_plain_text())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("fn main()"));
        assert!(plain.contains("println!"));
    }

    #[test]
    fn markdown_renders_list_bullets() {
        let screen = MarkdownRichText::new();
        let plain: String = screen
            .rendered_md
            .lines()
            .iter()
            .map(|l| l.to_plain_text())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("\u{2022} Styled text rendering"));
        assert!(plain.contains("1. Parse markdown"));
    }

    #[test]
    fn scroll_navigation() {
        let mut screen = MarkdownRichText::new();
        screen.update(&press(KeyCode::Down));
        assert_eq!(screen.md_scroll, 1);
        screen.update(&press(KeyCode::Down));
        assert_eq!(screen.md_scroll, 2);
        screen.update(&press(KeyCode::Up));
        assert_eq!(screen.md_scroll, 1);
        screen.update(&press(KeyCode::Home));
        assert_eq!(screen.md_scroll, 0);
        screen.update(&press(KeyCode::Up));
        assert_eq!(screen.md_scroll, 0);
    }

    #[test]
    fn page_scroll() {
        let mut screen = MarkdownRichText::new();
        screen.update(&press(KeyCode::PageDown));
        assert_eq!(screen.md_scroll, 10);
        screen.update(&press(KeyCode::PageUp));
        assert_eq!(screen.md_scroll, 0);
    }

    #[test]
    fn wrap_mode_cycles() {
        let mut screen = MarkdownRichText::new();
        assert_eq!(screen.wrap_label(), "Word");
        screen.update(&press(KeyCode::Char('w')));
        assert_eq!(screen.wrap_label(), "Char");
        screen.update(&press(KeyCode::Char('w')));
        assert_eq!(screen.wrap_label(), "WordChar");
        screen.update(&press(KeyCode::Char('w')));
        assert_eq!(screen.wrap_label(), "None");
        screen.update(&press(KeyCode::Char('w')));
        assert_eq!(screen.wrap_label(), "Word");
    }

    #[test]
    fn alignment_cycles() {
        let mut screen = MarkdownRichText::new();
        assert_eq!(screen.alignment_label(), "Left");
        screen.update(&press(KeyCode::Char('a')));
        assert_eq!(screen.alignment_label(), "Center");
        screen.update(&press(KeyCode::Char('a')));
        assert_eq!(screen.alignment_label(), "Right");
        screen.update(&press(KeyCode::Char('a')));
        assert_eq!(screen.alignment_label(), "Left");
    }

    #[test]
    fn keybindings_non_empty() {
        let screen = MarkdownRichText::new();
        assert!(!screen.keybindings().is_empty());
    }

    #[test]
    fn style_flags_all_represented() {
        let styles = [
            theme::bold(),
            theme::dim(),
            theme::italic(),
            theme::underline(),
            theme::strikethrough(),
            theme::reverse(),
            theme::blink_style(),
            theme::double_underline(),
            theme::curly_underline(),
        ];
        for style in &styles {
            assert_ne!(*style, Style::default());
        }
    }
}
