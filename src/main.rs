// В релизной сборке под Windows не открываем консольное окно рядом с приложением.
// В отладочной консоль остаётся — в неё пишутся ошибки
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod core;
mod ui;

use eframe::egui;

/// Имя приложения: заголовок окна и имя папки с данными (библиотекой)
pub const APP_NAME: &str = "LMP";

fn main() -> eframe::Result {
    // Иконка вшивается в бинарник при сборке, отдельный файл рядом не нужен.
    // На macOS — сквиркл по шаблону Apple, иначе — пластинка на прозрачном фоне
    #[cfg(target_os = "macos")]
    const ICON_PNG: &[u8] = include_bytes!("../assets/images/icon_macos.png");
    #[cfg(not(target_os = "macos"))]
    const ICON_PNG: &[u8] = include_bytes!("../assets/images/icon_windows.png");
    let icon = eframe::icon_data::from_png_bytes(ICON_PNG).expect("app icon is not a valid PNG");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // Стартовые ширины: библиотека 240 + центр + список треков 300 — центру остаётся ~440
            .with_inner_size([980.0, 620.0])
            // Минимумы: боковые панели по 180 + центральная область 260 = 620, плюс запас
            .with_min_inner_size([640.0, 400.0])
            .with_icon(icon)
            // Прозрачное окно: под ним система рисует настоящее размытие (см. window_glass.rs)
            .with_transparent(true),
        renderer: ui::style::window_glass::renderer(),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            if let Err(e) = ui::style::window_glass::apply(cc) {
                eprintln!("Could not enable the window blur: {e}");
            }
            Ok(Box::new(ui::PlayerApp::new(cc)))
        }),
    )
}
