mod app;
mod desmos;
mod geometry;
mod model;
mod persistence;
mod ui;

use app::DrawafuncApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Drawafunc")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Drawafunc",
        options,
        Box::new(|cc| Ok(Box::new(DrawafuncApp::new(cc)))),
    )
}
