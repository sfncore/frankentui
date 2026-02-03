#![forbid(unsafe_code)]
//! Snapshot tests for text effects visual regression (bd-3cuk).
//!
//! Run with: cargo test -p ftui-extras --test text_effects_snapshots
//! Update snapshots: BLESS=1 cargo test -p ftui-extras --test text_effects_snapshots

#[cfg(feature = "text-effects")]
use ftui_core::geometry::Rect;
#[cfg(feature = "text-effects")]
use ftui_harness::assert_snapshot_ansi;
#[cfg(feature = "text-effects")]
use ftui_render::cell::PackedRgba;
#[cfg(feature = "text-effects")]
use ftui_render::frame::Frame;
#[cfg(feature = "text-effects")]
use ftui_render::grapheme_pool::GraphemePool;
#[cfg(feature = "text-effects")]
use ftui_widgets::Widget;

// Import the text effects module
// Note: Requires the text-effects feature to be enabled
#[cfg(feature = "text-effects")]
use ftui_extras::text_effects::{
    AsciiArtStyle, AsciiArtText, ColorGradient, Direction, StyledText, TextEffect,
};

// =============================================================================
// Gradient Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_rainbow_gradient() {
    let text = StyledText::new("RAINBOW GRADIENT TEST")
        .effect(TextEffect::RainbowGradient { speed: 0.0 })
        .time(0.0);

    let area = Rect::new(0, 0, 30, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(30, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_rainbow_gradient", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_horizontal_gradient() {
    let gradient = ColorGradient::sunset();
    let text = StyledText::new("SUNSET GRADIENT")
        .effect(TextEffect::HorizontalGradient { gradient })
        .time(0.0);

    let area = Rect::new(0, 0, 20, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(20, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_horizontal_gradient", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_animated_gradient_frame_0() {
    let gradient = ColorGradient::cyberpunk();
    let text = StyledText::new("CYBERPUNK")
        .effect(TextEffect::AnimatedGradient {
            gradient,
            speed: 1.0,
        })
        .time(0.0);

    let area = Rect::new(0, 0, 15, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_animated_gradient_f0", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_animated_gradient_frame_50() {
    let gradient = ColorGradient::cyberpunk();
    let text = StyledText::new("CYBERPUNK")
        .effect(TextEffect::AnimatedGradient {
            gradient,
            speed: 1.0,
        })
        .time(0.5);

    let area = Rect::new(0, 0, 15, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_animated_gradient_f50", &frame.buffer);
}

// =============================================================================
// Wave Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_wave_frame_0() {
    let text = StyledText::new("WAVE TEXT")
        .effect(TextEffect::Wave {
            amplitude: 1.0,
            wavelength: 5.0,
            speed: 1.0,
            direction: Direction::Down,
        })
        .time(0.0);

    let area = Rect::new(0, 0, 15, 3);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 3, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_wave_f0", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_wave_frame_25() {
    let text = StyledText::new("WAVE TEXT")
        .effect(TextEffect::Wave {
            amplitude: 1.0,
            wavelength: 5.0,
            speed: 1.0,
            direction: Direction::Down,
        })
        .time(0.25);

    let area = Rect::new(0, 0, 15, 3);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 3, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_wave_f25", &frame.buffer);
}

// =============================================================================
// Glow Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_glow_static() {
    let text = StyledText::new("GLOW")
        .effect(TextEffect::Glow {
            color: PackedRgba::rgb(0, 255, 255),
            intensity: 0.8,
        })
        .base_color(PackedRgba::rgb(255, 255, 255))
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_glow_static", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_pulsing_glow() {
    let text = StyledText::new("PULSE")
        .effect(TextEffect::PulsingGlow {
            color: PackedRgba::rgb(255, 0, 128),
            speed: 2.0,
        })
        .time(0.25);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_pulsing_glow", &frame.buffer);
}

// =============================================================================
// ASCII Art Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_ascii_art_block() {
    let art = AsciiArtText::new("HI", AsciiArtStyle::Block);
    let lines = art.render_lines();

    // Create a buffer big enough for the ASCII art
    let height = lines.len() as u16;
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(width.max(10), height.max(5), &mut pool);

    // Render each line
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if x < width as usize && y < height as usize {
                frame
                    .buffer
                    .set_raw(x as u16, y as u16, ftui_render::cell::Cell::from_char(ch));
            }
        }
    }

    assert_snapshot_ansi!("text_effects_ascii_art_block", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_ascii_art_banner() {
    let art = AsciiArtText::new("AB", AsciiArtStyle::Banner);
    let lines = art.render_lines();

    let height = lines.len() as u16;
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(width.max(10), height.max(5), &mut pool);

    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if x < width as usize && y < height as usize {
                frame
                    .buffer
                    .set_raw(x as u16, y as u16, ftui_render::cell::Cell::from_char(ch));
            }
        }
    }

    assert_snapshot_ansi!("text_effects_ascii_art_banner", &frame.buffer);
}

// =============================================================================
// Fade Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_fade_in_0() {
    let text = StyledText::new("FADE IN")
        .effect(TextEffect::FadeIn { progress: 0.0 })
        .base_color(PackedRgba::rgb(255, 255, 255))
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_fade_in_0", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_fade_in_50() {
    let text = StyledText::new("FADE IN")
        .effect(TextEffect::FadeIn { progress: 0.5 })
        .base_color(PackedRgba::rgb(255, 255, 255))
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_fade_in_50", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_fade_in_100() {
    let text = StyledText::new("FADE IN")
        .effect(TextEffect::FadeIn { progress: 1.0 })
        .base_color(PackedRgba::rgb(255, 255, 255))
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_fade_in_100", &frame.buffer);
}

// =============================================================================
// Pulse Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_pulse_min() {
    let text = StyledText::new("PULSE")
        .effect(TextEffect::Pulse {
            speed: 1.0,
            min_alpha: 0.3,
        })
        .base_color(PackedRgba::rgb(255, 100, 100))
        .time(0.5); // At 0.5s with 1Hz, should be near min

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_pulse_min", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_pulse_max() {
    let text = StyledText::new("PULSE")
        .effect(TextEffect::Pulse {
            speed: 1.0,
            min_alpha: 0.3,
        })
        .base_color(PackedRgba::rgb(255, 100, 100))
        .time(0.0); // At 0s, should be at max

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_pulse_max", &frame.buffer);
}

// =============================================================================
// Typewriter Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_typewriter_partial() {
    let text = StyledText::new("TYPEWRITER")
        .effect(TextEffect::Typewriter { visible_chars: 5.0 })
        .time(0.0);

    let area = Rect::new(0, 0, 15, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_typewriter_partial", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_typewriter_complete() {
    let text = StyledText::new("TYPEWRITER")
        .effect(TextEffect::Typewriter {
            visible_chars: 10.0,
        })
        .time(0.0);

    let area = Rect::new(0, 0, 15, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_typewriter_complete", &frame.buffer);
}

// =============================================================================
// Effect Chain Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_effect_chain() {
    let text = StyledText::new("CHAINED")
        .effect(TextEffect::RainbowGradient { speed: 0.0 })
        .effect(TextEffect::Pulse {
            speed: 1.0,
            min_alpha: 0.5,
        })
        .time(0.0);

    let area = Rect::new(0, 0, 12, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(12, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_chain", &frame.buffer);
}

// =============================================================================
// Scramble Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_scramble_start() {
    let text = StyledText::new("SCRAMBLE")
        .effect(TextEffect::Scramble { progress: 0.0 })
        .seed(42)
        .time(0.0);

    let area = Rect::new(0, 0, 12, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(12, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_scramble_start", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_scramble_end() {
    let text = StyledText::new("SCRAMBLE")
        .effect(TextEffect::Scramble { progress: 1.0 })
        .seed(42)
        .time(0.0);

    let area = Rect::new(0, 0, 12, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(12, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_scramble_end", &frame.buffer);
}

// =============================================================================
// Color Wave Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_color_wave() {
    let text = StyledText::new("COLOR WAVE")
        .effect(TextEffect::ColorWave {
            color1: PackedRgba::rgb(255, 0, 0),
            color2: PackedRgba::rgb(0, 0, 255),
            speed: 1.0,
            wavelength: 5.0,
        })
        .time(0.0);

    let area = Rect::new(0, 0, 15, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(15, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_color_wave", &frame.buffer);
}

// =============================================================================
// Glitch Effect Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_glitch_low() {
    let text = StyledText::new("GLITCH")
        .effect(TextEffect::Glitch { intensity: 0.2 })
        .seed(42)
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_glitch_low", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_glitch_high() {
    let text = StyledText::new("GLITCH")
        .effect(TextEffect::Glitch { intensity: 0.8 })
        .seed(42)
        .time(0.0);

    let area = Rect::new(0, 0, 10, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(10, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_glitch_high", &frame.buffer);
}

// =============================================================================
// Preset Gradient Snapshots
// =============================================================================

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_preset_fire() {
    let text = StyledText::new("FIRE GRADIENT")
        .effect(TextEffect::HorizontalGradient {
            gradient: ColorGradient::fire(),
        })
        .time(0.0);

    let area = Rect::new(0, 0, 18, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(18, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_preset_fire", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_preset_ocean() {
    let text = StyledText::new("OCEAN GRADIENT")
        .effect(TextEffect::HorizontalGradient {
            gradient: ColorGradient::ocean(),
        })
        .time(0.0);

    let area = Rect::new(0, 0, 18, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(18, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_preset_ocean", &frame.buffer);
}

#[test]
#[cfg(feature = "text-effects")]
fn snapshot_preset_matrix() {
    let text = StyledText::new("MATRIX GRADIENT")
        .effect(TextEffect::HorizontalGradient {
            gradient: ColorGradient::matrix(),
        })
        .time(0.0);

    let area = Rect::new(0, 0, 20, 1);
    let mut pool = GraphemePool::new();
    let mut frame = Frame::new(20, 1, &mut pool);
    text.render(area, &mut frame);
    assert_snapshot_ansi!("text_effects_preset_matrix", &frame.buffer);
}
