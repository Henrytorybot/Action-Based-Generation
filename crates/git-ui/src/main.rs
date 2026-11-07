// Release builds open without a console window behind them; debug builds keep
// it so panics and `dbg!` stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod git;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([720.0, 520.0])
            .with_title("Action-Based-Generation — Git"),
        ..Default::default()
    };

    eframe::run_native(
        "abg-git-ui",
        options,
        Box::new(|cc| Ok(Box::new(app::GitApp::new(cc)))),
    )
}
