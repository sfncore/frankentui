#![forbid(unsafe_code)]

//! Per-screen snapshot tests for the FrankenTUI Demo Showcase.
//!
//! Each screen is rendered at standard sizes and compared against stored
//! baselines. Run `BLESS=1 cargo test -p ftui-demo-showcase` to create or
//! update snapshot files.
//!
//! Naming convention: `screen_name_scenario_WIDTHxHEIGHT`

use ftui_core::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};
use ftui_core::geometry::Rect;
use ftui_demo_showcase::app::{AppModel, ScreenId};
use ftui_demo_showcase::screens::Screen;
use ftui_harness::assert_snapshot;
use ftui_render::frame::Frame;
use ftui_render::grapheme_pool::GraphemePool;
use ftui_runtime::Model;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Press,
    })
}

fn ctrl_press(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: Modifiers::CTRL,
        kind: KeyEventKind::Press,
    })
}

// ============================================================================
// Dashboard
// ============================================================================

#[test]
fn dashboard_initial_80x24() {
    let screen = ftui_demo_showcase::screens::dashboard::Dashboard::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("dashboard_initial_80x24", &frame.buffer);
}

#[test]
fn dashboard_initial_120x40() {
    let screen = ftui_demo_showcase::screens::dashboard::Dashboard::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("dashboard_initial_120x40", &frame.buffer);
}

#[test]
fn dashboard_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::dashboard::Dashboard::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("dashboard_tiny_40x10", &frame.buffer);
}

#[test]
fn dashboard_zero_area() {
    let screen = ftui_demo_showcase::screens::dashboard::Dashboard::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(1, 1, &mut pool);
    let area = Rect::new(0, 0, 0, 0);
    screen.view(&mut frame, area);
    // No panic = success
}

#[test]
fn dashboard_title() {
    let screen = ftui_demo_showcase::screens::dashboard::Dashboard::new();
    assert_eq!(screen.title(), "Dashboard");
    assert_eq!(screen.tab_label(), "Dashboard");
}

// ============================================================================
// Shakespeare
// ============================================================================

#[test]
fn shakespeare_initial_120x40() {
    let screen = ftui_demo_showcase::screens::shakespeare::Shakespeare::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("shakespeare_initial_120x40", &frame.buffer);
}

#[test]
fn shakespeare_initial_80x24() {
    let screen = ftui_demo_showcase::screens::shakespeare::Shakespeare::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("shakespeare_initial_80x24", &frame.buffer);
}

#[test]
fn shakespeare_after_scroll_120x40() {
    let mut screen = ftui_demo_showcase::screens::shakespeare::Shakespeare::new();
    for _ in 0..5 {
        screen.update(&press(KeyCode::Down));
    }
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("shakespeare_after_scroll_120x40", &frame.buffer);
}

#[test]
fn shakespeare_end_key_120x40() {
    let mut screen = ftui_demo_showcase::screens::shakespeare::Shakespeare::new();
    screen.update(&press(KeyCode::End));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("shakespeare_end_key_120x40", &frame.buffer);
}

#[test]
fn shakespeare_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::shakespeare::Shakespeare::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("shakespeare_tiny_40x10", &frame.buffer);
}

// ============================================================================
// Code Explorer
// ============================================================================

#[test]
fn code_explorer_initial_120x40() {
    let screen = ftui_demo_showcase::screens::code_explorer::CodeExplorer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("code_explorer_initial_120x40", &frame.buffer);
}

#[test]
fn code_explorer_initial_80x24() {
    let screen = ftui_demo_showcase::screens::code_explorer::CodeExplorer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("code_explorer_initial_80x24", &frame.buffer);
}

#[test]
fn code_explorer_scrolled_120x40() {
    let mut screen = ftui_demo_showcase::screens::code_explorer::CodeExplorer::new();
    for _ in 0..20 {
        screen.update(&press(KeyCode::Down));
    }
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("code_explorer_scrolled_120x40", &frame.buffer);
}

#[test]
fn code_explorer_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::code_explorer::CodeExplorer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("code_explorer_tiny_40x10", &frame.buffer);
}

#[test]
fn code_explorer_wide_200x50() {
    let screen = ftui_demo_showcase::screens::code_explorer::CodeExplorer::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(200, 50, &mut pool);
    let area = Rect::new(0, 0, 200, 50);
    screen.view(&mut frame, area);
    assert_snapshot!("code_explorer_wide_200x50", &frame.buffer);
}

// ============================================================================
// Widget Gallery
// ============================================================================

#[test]
fn widget_gallery_initial_120x40() {
    let screen = ftui_demo_showcase::screens::widget_gallery::WidgetGallery::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("widget_gallery_initial_120x40", &frame.buffer);
}

#[test]
fn widget_gallery_initial_80x24() {
    let screen = ftui_demo_showcase::screens::widget_gallery::WidgetGallery::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("widget_gallery_initial_80x24", &frame.buffer);
}

#[test]
fn widget_gallery_section2_120x40() {
    let mut screen = ftui_demo_showcase::screens::widget_gallery::WidgetGallery::new();
    screen.update(&press(KeyCode::Right));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("widget_gallery_section2_120x40", &frame.buffer);
}

#[test]
fn widget_gallery_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::widget_gallery::WidgetGallery::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("widget_gallery_tiny_40x10", &frame.buffer);
}

#[test]
fn widget_gallery_with_tick_120x40() {
    let mut screen = ftui_demo_showcase::screens::widget_gallery::WidgetGallery::new();
    screen.tick(5);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("widget_gallery_with_tick_120x40", &frame.buffer);
}

// ============================================================================
// Layout Lab
// ============================================================================

#[test]
fn layout_lab_initial_120x40() {
    let screen = ftui_demo_showcase::screens::layout_lab::LayoutLab::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("layout_lab_initial_120x40", &frame.buffer);
}

#[test]
fn layout_lab_initial_80x24() {
    let screen = ftui_demo_showcase::screens::layout_lab::LayoutLab::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("layout_lab_initial_80x24", &frame.buffer);
}

#[test]
fn layout_lab_preset2_120x40() {
    let mut screen = ftui_demo_showcase::screens::layout_lab::LayoutLab::new();
    screen.update(&press(KeyCode::Right));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("layout_lab_preset2_120x40", &frame.buffer);
}

#[test]
fn layout_lab_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::layout_lab::LayoutLab::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("layout_lab_tiny_40x10", &frame.buffer);
}

#[test]
fn layout_lab_wide_200x50() {
    let screen = ftui_demo_showcase::screens::layout_lab::LayoutLab::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(200, 50, &mut pool);
    let area = Rect::new(0, 0, 200, 50);
    screen.view(&mut frame, area);
    assert_snapshot!("layout_lab_wide_200x50", &frame.buffer);
}

// ============================================================================
// Forms & Input
// ============================================================================

#[test]
fn forms_input_initial_120x40() {
    let screen = ftui_demo_showcase::screens::forms_input::FormsInput::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("forms_input_initial_120x40", &frame.buffer);
}

#[test]
fn forms_input_initial_80x24() {
    let screen = ftui_demo_showcase::screens::forms_input::FormsInput::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("forms_input_initial_80x24", &frame.buffer);
}

#[test]
fn forms_input_panel_switch_120x40() {
    let mut screen = ftui_demo_showcase::screens::forms_input::FormsInput::new();
    screen.update(&ctrl_press(KeyCode::Right));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("forms_input_panel_switch_120x40", &frame.buffer);
}

#[test]
fn forms_input_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::forms_input::FormsInput::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("forms_input_tiny_40x10", &frame.buffer);
}

#[test]
fn forms_input_tab_down_120x40() {
    let mut screen = ftui_demo_showcase::screens::forms_input::FormsInput::new();
    screen.update(&press(KeyCode::Tab));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("forms_input_tab_down_120x40", &frame.buffer);
}

// ============================================================================
// Data Viz
// ============================================================================

#[test]
fn data_viz_initial_120x40() {
    let screen = ftui_demo_showcase::screens::data_viz::DataViz::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("data_viz_initial_120x40", &frame.buffer);
}

#[test]
fn data_viz_initial_80x24() {
    let screen = ftui_demo_showcase::screens::data_viz::DataViz::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("data_viz_initial_80x24", &frame.buffer);
}

#[test]
fn data_viz_after_ticks_120x40() {
    let mut screen = ftui_demo_showcase::screens::data_viz::DataViz::new();
    screen.tick(35);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("data_viz_after_ticks_120x40", &frame.buffer);
}

#[test]
fn data_viz_bar_horizontal_120x40() {
    let mut screen = ftui_demo_showcase::screens::data_viz::DataViz::new();
    screen.update(&press(KeyCode::Char('d')));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("data_viz_bar_horizontal_120x40", &frame.buffer);
}

#[test]
fn data_viz_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::data_viz::DataViz::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("data_viz_tiny_40x10", &frame.buffer);
}

// ============================================================================
// File Browser
// ============================================================================

#[test]
fn file_browser_initial_120x40() {
    let screen = ftui_demo_showcase::screens::file_browser::FileBrowser::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("file_browser_initial_120x40", &frame.buffer);
}

#[test]
fn file_browser_initial_80x24() {
    let screen = ftui_demo_showcase::screens::file_browser::FileBrowser::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("file_browser_initial_80x24", &frame.buffer);
}

#[test]
fn file_browser_navigate_down_120x40() {
    let mut screen = ftui_demo_showcase::screens::file_browser::FileBrowser::new();
    for _ in 0..3 {
        screen.update(&press(KeyCode::Down));
    }
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("file_browser_navigate_down_120x40", &frame.buffer);
}

#[test]
fn file_browser_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::file_browser::FileBrowser::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("file_browser_tiny_40x10", &frame.buffer);
}

#[test]
fn file_browser_panel_switch_120x40() {
    let mut screen = ftui_demo_showcase::screens::file_browser::FileBrowser::new();
    screen.update(&ctrl_press(KeyCode::Right));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("file_browser_panel_switch_120x40", &frame.buffer);
}

// ============================================================================
// Markdown & Rich Text
// ============================================================================

#[test]
fn markdown_initial_120x40() {
    let screen = ftui_demo_showcase::screens::markdown_rich_text::MarkdownRichText::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("markdown_initial_120x40", &frame.buffer);
}

#[test]
fn markdown_initial_80x24() {
    let screen = ftui_demo_showcase::screens::markdown_rich_text::MarkdownRichText::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("markdown_initial_80x24", &frame.buffer);
}

#[test]
fn markdown_scrolled_120x40() {
    let mut screen = ftui_demo_showcase::screens::markdown_rich_text::MarkdownRichText::new();
    for _ in 0..10 {
        screen.update(&press(KeyCode::Down));
    }
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("markdown_scrolled_120x40", &frame.buffer);
}

#[test]
fn markdown_wrap_cycle_120x40() {
    let mut screen = ftui_demo_showcase::screens::markdown_rich_text::MarkdownRichText::new();
    screen.update(&press(KeyCode::Char('w')));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("markdown_wrap_cycle_120x40", &frame.buffer);
}

#[test]
fn markdown_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::markdown_rich_text::MarkdownRichText::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("markdown_tiny_40x10", &frame.buffer);
}

// ============================================================================
// Advanced Features
// ============================================================================

#[test]
fn advanced_initial_120x40() {
    let screen = ftui_demo_showcase::screens::advanced_features::AdvancedFeatures::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("advanced_initial_120x40", &frame.buffer);
}

#[test]
fn advanced_initial_80x24() {
    let screen = ftui_demo_showcase::screens::advanced_features::AdvancedFeatures::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("advanced_initial_80x24", &frame.buffer);
}

#[test]
fn advanced_panel_switch_120x40() {
    let mut screen = ftui_demo_showcase::screens::advanced_features::AdvancedFeatures::new();
    screen.update(&ctrl_press(KeyCode::Right));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("advanced_panel_switch_120x40", &frame.buffer);
}

#[test]
fn advanced_after_ticks_120x40() {
    let mut screen = ftui_demo_showcase::screens::advanced_features::AdvancedFeatures::new();
    for t in 1..=10 {
        screen.tick(t);
    }
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("advanced_after_ticks_120x40", &frame.buffer);
}

#[test]
fn advanced_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::advanced_features::AdvancedFeatures::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("advanced_tiny_40x10", &frame.buffer);
}

// ============================================================================
// Performance
// ============================================================================

#[test]
fn performance_initial_120x40() {
    let screen = ftui_demo_showcase::screens::performance::Performance::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("performance_initial_120x40", &frame.buffer);
}

#[test]
fn performance_initial_80x24() {
    let screen = ftui_demo_showcase::screens::performance::Performance::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(80, 24, &mut pool);
    let area = Rect::new(0, 0, 80, 24);
    screen.view(&mut frame, area);
    assert_snapshot!("performance_initial_80x24", &frame.buffer);
}

#[test]
fn performance_scrolled_120x40() {
    let mut screen = ftui_demo_showcase::screens::performance::Performance::new();
    screen.update(&press(KeyCode::PageDown));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("performance_scrolled_120x40", &frame.buffer);
}

#[test]
fn performance_end_key_120x40() {
    let mut screen = ftui_demo_showcase::screens::performance::Performance::new();
    screen.update(&press(KeyCode::End));
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    let area = Rect::new(0, 0, 120, 40);
    screen.view(&mut frame, area);
    assert_snapshot!("performance_end_key_120x40", &frame.buffer);
}

#[test]
fn performance_tiny_40x10() {
    let screen = ftui_demo_showcase::screens::performance::Performance::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(40, 10, &mut pool);
    let area = Rect::new(0, 0, 40, 10);
    screen.view(&mut frame, area);
    assert_snapshot!("performance_tiny_40x10", &frame.buffer);
}

// ============================================================================
// Full AppModel integration snapshots
// ============================================================================

#[test]
fn app_dashboard_full_120x40() {
    let app = AppModel::new();
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    app.view(&mut frame);
    assert_snapshot!("app_dashboard_full_120x40", &frame.buffer);
}

#[test]
fn app_shakespeare_full_120x40() {
    let mut app = AppModel::new();
    app.current_screen = ScreenId::Shakespeare;
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    app.view(&mut frame);
    assert_snapshot!("app_shakespeare_full_120x40", &frame.buffer);
}

#[test]
fn app_help_overlay_120x40() {
    let mut app = AppModel::new();
    app.help_visible = true;
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    app.view(&mut frame);
    assert_snapshot!("app_help_overlay_120x40", &frame.buffer);
}

#[test]
fn app_debug_overlay_120x40() {
    let mut app = AppModel::new();
    app.debug_visible = true;
    app.terminal_width = 120;
    app.terminal_height = 40;
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(120, 40, &mut pool);
    app.view(&mut frame);
    assert_snapshot!("app_debug_overlay_120x40", &frame.buffer);
}

#[test]
fn app_all_screens_80x24() {
    for &id in ScreenId::ALL {
        let mut app = AppModel::new();
        app.current_screen = id;
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(80, 24, &mut pool);
        app.view(&mut frame);
        let name = format!("app_{:?}_80x24", id).to_lowercase();
        assert_snapshot!(&name, &frame.buffer);
    }
}
