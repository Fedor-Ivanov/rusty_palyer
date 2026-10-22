//! Настоящее стекло под окном: окно прозрачное, а система размывает рабочий стол
//! и окна за ним.

/// Включает системное размытие под окном. Если эффект недоступен, окно просто остаётся
/// прозрачным — это не повод падать, поэтому ошибка только возвращается.
pub fn apply(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::apply(cc);

    // Windows 11: Mica в тёмном варианте. Система рисует его под всем окном сама,
    // поэтому готовая функция из window-vibrancy здесь подходит. Mica есть только
    // в Windows 11 — в Windows 10 обычное системное размытие
    #[cfg(target_os = "windows")]
    return window_vibrancy::apply_mica(cc, Some(true))
        .or_else(|_| window_vibrancy::apply_blur(cc, None))
        .map_err(|e| e.to_string());

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = cc;
        Err("window blur is only supported on macOS and Windows".into())
    }
}

/// Настройки отрисовки (wgpu). Окно прозрачное, но рисовать в прозрачное окно умеет
/// не всякий способ вывода. На Windows по умолчанию wgpu выводит прямо в окно
/// (`DxgiFromHwnd`) — без прозрачности: стекла под окном не видно, фон сплошной. Прозрачность
/// умеет только вывод через DirectComposition (`DxgiFromVisual`), а он есть только у DX12 —
/// поэтому на Windows другие API (Vulkan) не используются. На macOS настройки по умолчанию
pub fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    #[allow(unused_mut)]
    let mut options = eframe::egui_wgpu::WgpuConfiguration::default();
    #[cfg(target_os = "windows")]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        use eframe::wgpu;
        setup.instance_descriptor.backends = wgpu::Backends::DX12;
        setup
            .instance_descriptor
            .backend_options
            .dx12
            .presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
    }
    options
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
