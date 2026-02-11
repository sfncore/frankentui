#[allow(dead_code)]
mod app;
#[allow(dead_code)]
mod data;
mod msg;
mod panels;
mod screen;
mod screens;
#[allow(dead_code)]
mod theme;

use ftui_runtime::{Program, ProgramConfig, ScreenMode};

fn main() -> std::io::Result<()> {
    let config = ProgramConfig {
        screen_mode: ScreenMode::AltScreen,
        mouse: true,
        ..Default::default()
    };

    let dashboard = app::GtApp::new();
    let mut program = Program::with_config(dashboard, config)?;
    program.run()
}
