mod app;
mod comms;
mod helpers;
mod models;
mod state;
mod ui;
mod update_handler;

use app::GalaxyApp;

fn main() -> Result<(), eframe::Error> {
    // Silence noisy UI framework logs (winit/egui/eframe) unless explicitly overridden.
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var(
                "RUST_LOG",
                "gui_serena=debug,orchestrator=debug,winit=off,egui=off,eframe=off,tracing=off",
            );
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 1000.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Immutable Cosmic Borrow Galaxy",
        options,
        Box::new(|cc| Ok(Box::new(GalaxyApp::new(cc)))),
    )
}
