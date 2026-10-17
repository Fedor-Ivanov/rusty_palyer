mod app;
mod audio;
mod metadata;
mod player;
mod theme;
mod track;
mod ui_components;
mod utils;
mod window_glass;

use eframe::egui;

fn main() -> eframe::Result {
    // Иконка вшивается в бинарник при сборке, отдельный файл рядом не нужен.
    // На macOS — сквиркл по шаблону Apple, иначе — пластинка на прозрачном фоне
    #[cfg(target_os = "macos")]
    const ICON_PNG: &[u8] = include_bytes!("../assets/icon_macos.png");
    #[cfg(not(target_os = "macos"))]
    const ICON_PNG: &[u8] = include_bytes!("../assets/icon_windows.png");
    let icon = eframe::icon_data::from_png_bytes(ICON_PNG).expect("иконка — некорректный PNG");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 480.0])
            // Ширина: минимумы боковой панели 150 + списка треков 150 + центральной области 260 = 560, плюс запас
            .with_min_inner_size([600.0, 400.0])
            .with_icon(icon)
            // Прозрачное окно: под ним система рисует настоящее размытие (см. window_glass.rs)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "Rusty Player",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            if let Err(e) = window_glass::apply(cc) {
                eprintln!("Не удалось включить размытие под окном: {e}");
            }
            Ok(Box::new(app::PlayerApp::new()))
        }),
    )
}
