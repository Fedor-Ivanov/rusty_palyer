mod app;
mod audio;
mod metadata;
mod player;
mod track;
mod utils;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 320.0])
            .with_min_inner_size([320.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rusty Player",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(app::PlayerApp::new()))
        }),
    )
}
