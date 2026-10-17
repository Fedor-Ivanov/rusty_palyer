//! Настоящее стекло под окном: окно прозрачное, а система размывает рабочий стол
//! и окна за ним.

/// Включает системное размытие под окном. Если эффект недоступен, окно просто остаётся
/// прозрачным — это не повод падать, поэтому ошибка только возвращается.
pub fn apply(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::apply(cc);

    // Windows 11: Mica в тёмном варианте. Система рисует его под всем окном сама,
    // поэтому готовая функция из window-vibrancy здесь подходит
    #[cfg(target_os = "windows")]
    return window_vibrancy::apply_mica(cc, Some(true)).map_err(|e| e.to_string());

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cc;
        Err("размытие под окном поддерживается только на macOS и Windows".into())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua,
        NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// NSVisualEffectView кладётся не внутрь вью egui, а рядом — в рамку окна, под него.
    /// Вью winit рисует egui прямо в свой слой (Metal), а AppKit всегда рисует подвью
    /// поверх слоя родителя: размытие внутри вью egui закрыло бы весь контент
    /// (так делает window-vibrancy — для Tauri это подходит, для egui нет).
    /// Сам вью egui при этом остаётся на месте, так что ввод не затрагивается.
    pub fn apply(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
        let mtm =
            MainThreadMarker::new().ok_or("размытие можно включить только из главного потока")?;

        let handle = cc.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("окно не AppKit".into());
        };
        // SAFETY: вью принадлежит окну, которое живо всё время работы приложения
        let content_view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        // SAFETY: рамка окна живёт столько же, сколько окно
        let frame_view = unsafe { content_view.superview() }.ok_or("у вью окна нет рамки")?;

        let blur = NSVisualEffectView::initWithFrame(mtm.alloc(), frame_view.bounds());
        // HUDWindow — тёмный материал под тёмную тему
        blur.setMaterial(NSVisualEffectMaterial::HUDWindow);
        // Тёмное оформление принудительно, как у виджетов macOS: иначе материал следует
        // системной теме и в светлой теме становится светлым, молочным
        // SAFETY: системная константа, доступна всё время работы приложения
        let dark = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua });
        blur.setAppearance(dark.as_deref());
        blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        // Active — размытие не гаснет, когда окно не в фокусе
        blur.setState(NSVisualEffectState::Active);
        blur.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        frame_view.addSubview_positioned_relativeTo(
            &blur,
            NSWindowOrderingMode::Below,
            Some(content_view),
        );
        Ok(())
    }
}
