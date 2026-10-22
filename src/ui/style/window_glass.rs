//! Настоящее стекло под окном: окно прозрачное, а система размывает рабочий стол
//! и окна за ним.

/// Включает системное размытие под окном. Если эффект недоступен, окно просто остаётся
/// прозрачным — это не повод падать, поэтому ошибка только возвращается.
pub fn apply(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::apply(cc);

    #[cfg(target_os = "windows")]
    return windows::apply(cc);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cc;
        Err("window blur is only supported on macOS and Windows".into())
    }
}

/// Чем рисовать окно. На Windows — OpenGL (glow): он рисует в обычную поверхность окна,
/// а её winit делает прозрачной, и под ней виден системный Acrylic. wgpu на Windows рисует
/// через DX12: напрямую в окно — без прозрачности, а через DirectComposition — под
/// поверхностью окна, которая его перекрывает (убрать её можно только флагом при создании
/// окна, а eframe его не передаёт). На macOS — wgpu, как по умолчанию
pub fn renderer() -> eframe::Renderer {
    #[cfg(target_os = "windows")]
    return eframe::Renderer::Glow;
    #[cfg(not(target_os = "windows"))]
    eframe::Renderer::default()
}

/// Windows: стекло — системный материал Acrylic (размывает окна и рабочий стол позади),
/// в тёмном варианте. Не Mica: Mica не просвечивает — это почти сплошная заливка в тон
/// обоев, а в светлой теме Windows ещё и светлая
#[cfg(target_os = "windows")]
mod windows {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
            DwmSetWindowAttribute,
        },
    };

    pub fn apply(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
        let handle = cc.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("the window is not a Win32 window".into());
        };
        let hwnd = handle.hwnd.get() as HWND;
        // Тёмный вариант материала и заголовка — независимо от темы Windows
        let _ = set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, 1u32);
        // Системный Acrylic есть в Windows 11 22H2 и новее; система рисует его сама,
        // окно не тормозит при перетаскивании. Нет его (Windows 10) — обычное размытие:
        // Acrylic Windows 10 заметно тормозит при перетаскивании окна
        if set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW).is_ok() {
            return Ok(());
        }
        window_vibrancy::apply_blur(cc, None).map_err(|e| e.to_string())
    }

    /// Атрибут окна для DWM. Ошибка — система его не знает (старая версия Windows)
    fn set_attribute<T>(hwnd: HWND, attribute: i32, value: T) -> Result<(), i32> {
        // SAFETY: окно живо, `value` живёт до конца вызова, размер — его собственный
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute as u32,
                (&raw const value).cast(),
                size_of::<T>() as u32,
            )
        };
        if result < 0 { Err(result) } else { Ok(()) }
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
        let mtm = MainThreadMarker::new()
            .ok_or("window blur can only be enabled from the main thread")?;

        let handle = cc.window_handle().map_err(|e| e.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("the window is not an AppKit window".into());
        };
        // SAFETY: вью принадлежит окну, которое живо всё время работы приложения
        let content_view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        // SAFETY: рамка окна живёт столько же, сколько окно
        let frame_view =
            unsafe { content_view.superview() }.ok_or("the window view has no frame view")?;

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
