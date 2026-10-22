//! Добавление перетаскиванием: файлы и папки из Finder или Проводника бросают на окно.
//! Пока их несут над окном, видны две зоны — «в медиатеку» (всё левее очереди) и «в очередь»
//! (панель очереди); зона под курсором подсвечена. Брошенное добавляется туда же, куда
//! добавляет кнопка «+».
//!
//! Пока над окном несут файлы, система не сообщает приложению, где курсор: события движения
//! мыши не приходят ни на macOS, ни на Windows (winit сообщает только сами файлы). Поэтому
//! курсор спрашивается у системы напрямую (`Cursor`) — каждый кадр, пока файлы над окном.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, StrokeKind};

use super::{
    action::Action,
    app::PlayerApp,
    i18n::t,
    loading::Destination,
    style::theme::{HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, TEXT, TEXT_MUTED},
};

/// Отступ зон от краёв окна и друг от друга
const ZONE_INSET: f32 = 12.0;
const ZONE_RADIUS: u8 = 16;
const ZONE_TEXT_SIZE: f32 = 20.0;
/// Затемнение окна под зонами — чтобы надписи читались поверх любого содержимого
const DIM: Color32 = Color32::from_black_alpha(140);

impl PlayerApp {
    /// Зоны над окном, пока над ним несут файлы, и добавление брошенных. `queue` — где панель
    /// очереди: там зона «в очередь», левее — «в медиатеку». Вызывать в конце кадра, поверх всего
    pub(crate) fn render_file_drop(&mut self, ctx: &egui::Context, queue: Rect) {
        let (hovering, dropped) = ctx.input(|input| {
            (
                !input.raw.hovered_files.is_empty(),
                input
                    .raw
                    .dropped_files
                    .iter()
                    .map(|file| file.path().to_path_buf())
                    .collect::<Vec<_>>(),
            )
        });
        let window = ctx.content_rect();
        let zones = [
            (Destination::Library, window.with_max_x(queue.left())),
            (
                Destination::Queue,
                Rect::from_x_y_ranges(queue.x_range(), window.y_range()),
            ),
        ];
        let target = self.drop_cursor.position(ctx).and_then(|pointer| {
            zones
                .iter()
                .find(|(_, zone)| zone.contains(pointer))
                .map(|(destination, _)| *destination)
        });

        if !dropped.is_empty() {
            // Курсор узнать не удалось — в медиатеку: оттуда треки можно и сыграть, и добавить
            // в очередь
            let destination = target.unwrap_or(Destination::Library);
            self.dispatch(Action::AddPaths {
                paths: dropped,
                destination,
            });
        }

        let visibility = ctx.animate_bool_with_time_and_easing(
            egui::Id::unique("file_drop"),
            hovering,
            HOVER_FADE_SECS,
            egui::emath::easing::cubic_out,
        );
        if visibility == 0.0 {
            return;
        }
        // Где курсор, узнаём опросом — нужны кадры и без событий
        if hovering {
            ctx.request_repaint();
        }

        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("file_drop"),
            ))
            .with_clip_rect(window);
        let fade = |color: Color32| color.gamma_multiply(visibility);
        painter.rect_filled(window, 0, fade(DIM));
        for (destination, zone) in zones {
            let active = target == Some(destination);
            let label = match destination {
                Destination::Queue => t().add_to_queue,
                _ => t().add_to_library,
            };
            let rect = zone.shrink(ZONE_INSET);
            let (fill, stroke, text) = if active {
                (HIGHLIGHT_STRONG, Stroke::new(2.0, TEXT), TEXT)
            } else {
                (HIGHLIGHT, Stroke::new(1.0, TEXT_MUTED), TEXT_MUTED)
            };
            painter.rect(
                rect,
                CornerRadius::same(ZONE_RADIUS),
                fade(fill),
                Stroke::new(stroke.width, fade(stroke.color)),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(ZONE_TEXT_SIZE),
                fade(text),
            );
        }
    }
}

/// Курсор в координатах окна — спрашивается у системы. `None` — система не дала окна
/// (или платформа без поддержки): тогда зона под курсором неизвестна
pub(crate) struct Cursor(Option<platform::Cursor>);

impl Cursor {
    pub(crate) fn new(cc: &eframe::CreationContext) -> Self {
        Self(platform::Cursor::new(cc))
    }

    /// Где курсор — в точках egui
    fn position(&self, ctx: &egui::Context) -> Option<egui::Pos2> {
        self.0.as_ref()?.position(ctx)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use eframe::egui::{self, Pos2, pos2};
    use objc2::rc::Retained;
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub struct Cursor {
        view: Retained<NSView>,
    }

    impl Cursor {
        pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
            let handle = cc.window_handle().ok()?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return None;
            };
            // SAFETY: вью принадлежит окну приложения; удерживаем его своей ссылкой
            let view = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?;
            Some(Self { view })
        }

        /// Курсор в точках вью (начало — левый верхний угол), переведённых в точки egui
        pub fn position(&self, ctx: &egui::Context) -> Option<Pos2> {
            let window = self.view.window()?;
            let in_window = window.mouseLocationOutsideOfEventStream();
            let point = self.view.convertPoint_fromView(in_window, None);
            let y = if self.view.isFlipped() {
                point.y
            } else {
                self.view.bounds().size.height - point.y
            };
            let zoom = ctx.zoom_factor();
            Some(pos2(point.x as f32 / zoom, y as f32 / zoom))
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use eframe::egui::{self, Pos2, pos2};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ScreenToClient,
        UI::WindowsAndMessaging::GetCursorPos,
    };

    pub struct Cursor {
        hwnd: HWND,
    }

    impl Cursor {
        pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
            match cc.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(handle) => Some(Self {
                    hwnd: handle.hwnd.get() as HWND,
                }),
                _ => None,
            }
        }

        /// Курсор в пикселях клиентской области окна, переведённых в точки egui
        pub fn position(&self, ctx: &egui::Context) -> Option<Pos2> {
            let mut point = POINT { x: 0, y: 0 };
            // SAFETY: окно живо всё время работы приложения, `point` — наша переменная
            let ok = unsafe {
                GetCursorPos(&mut point) != 0 && ScreenToClient(self.hwnd, &mut point) != 0
            };
            if !ok {
                return None;
            }
            let scale = ctx.pixels_per_point();
            Some(pos2(point.x as f32 / scale, point.y as f32 / scale))
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use eframe::egui::{self, Pos2};

    pub struct Cursor;

    impl Cursor {
        pub fn new(_cc: &eframe::CreationContext) -> Option<Self> {
            None
        }

        pub fn position(&self, _ctx: &egui::Context) -> Option<Pos2> {
            None
        }
    }
}
