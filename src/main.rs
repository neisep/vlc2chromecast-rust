mod app;
mod config;
mod monitor;
mod vlc;

const WINDOW_WIDTH: f32 = 700.0;
const WINDOW_HEIGHT: f32 = 450.0;

fn main() -> eframe::Result {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
        .with_min_inner_size([500.0, 350.0])
        .with_drag_and_drop(true);

    let mut centered = false;
    if let Some((x, y)) = monitor::center_on_cursor_monitor(WINDOW_WIDTH, WINDOW_HEIGHT) {
        viewport = viewport.with_position([x, y]);
    } else {
        centered = true;
    }

    let options = eframe::NativeOptions {
        viewport,
        centered,
        ..Default::default()
    };

    eframe::run_native(
        "vlc2chromecast",
        options,
        Box::new(|_cc| Ok(Box::new(app::VlcChromecastApp::new()))),
    )
}
