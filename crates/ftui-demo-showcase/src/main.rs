#![forbid(unsafe_code)]

//! FrankenTUI Demo Showcase binary entry point.

use ftui_demo_showcase::app::{AppModel, ScreenId};
use ftui_demo_showcase::cli;
use ftui_runtime::{Program, ProgramConfig, ScreenMode};

fn main() {
    let opts = cli::Opts::parse();

    let screen_mode = match opts.screen_mode.as_str() {
        "inline" => ScreenMode::Inline {
            ui_height: opts.ui_height,
        },
        _ => ScreenMode::AltScreen,
    };

    let start_screen = if opts.start_screen >= 1 {
        let idx = (opts.start_screen as usize).saturating_sub(1);
        ScreenId::ALL
            .get(idx)
            .copied()
            .unwrap_or(ScreenId::Dashboard)
    } else {
        ScreenId::Dashboard
    };

    let mut model = AppModel::new();
    model.current_screen = start_screen;

    let config = ProgramConfig {
        screen_mode,
        mouse: opts.mouse,
        ..ProgramConfig::default()
    };
    match Program::with_config(model, config) {
        Ok(mut program) => {
            if let Err(e) = program.run() {
                eprintln!("Runtime error: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            std::process::exit(1);
        }
    }
}
