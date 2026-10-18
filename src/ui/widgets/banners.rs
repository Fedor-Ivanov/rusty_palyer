//! Плашки по центру сверху окна: ошибка и прогресс добавления треков.

use eframe::egui::{
    self, Align, CornerRadius, CursorIcon, Label, Layout, Rect, RichText, Sense, Stroke, vec2,
};

use crate::ui::theme::{BANNER_BORDER, BANNER_FILL, RED, SLIDER_TRACK, TEXT, TEXT_MUTED};

const ERROR_BANNER_MAX_WIDTH: f32 = 440.0;
const ERROR_BANNER_BUTTON_WIDTH: f32 = 48.0;
/// Плашки сверху окна (ошибка, прогресс): отступ от верхнего края и зазор между ними
pub const BANNER_TOP: f32 = 16.0;
pub const BANNER_GAP: f32 = 8.0;
const LOADING_BANNER_WIDTH: f32 = 300.0;

/// Плашка с ошибкой по центру сверху, поверх всего окна, в красной рамке: текст,
/// под ним справа — кнопка «OK».
/// Все ошибки приложения показываются только так. Возвращает `true`, когда нажали «OK».
/// `top` — отступ от верха окна: под плашкой прогресса, если она видна.
pub fn error_banner(ctx: &egui::Context, message: &str, top: f32) -> bool {
    let mut dismissed = false;
    egui::Area::new(egui::Id::unique("error_banner"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, top))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(BANNER_FILL)
                .stroke(Stroke::new(1.5, RED))
                .corner_radius(12)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_max_width(ERROR_BANNER_MAX_WIDTH);
                    // Текст сверху, переносится по строкам
                    ui.add(Label::new(RichText::new(message).color(TEXT)).wrap());
                    ui.add_space(8.0);

                    // «OK» под текстом, у правого края. Ширина строки — по тексту, а не
                    // по максимуму плашки: короткая ошибка даёт узкую плашку
                    let ok = egui::Button::new(RichText::new("OK").color(TEXT))
                        .corner_radius(8)
                        .min_size(vec2(ERROR_BANNER_BUTTON_WIDTH, 0.0));
                    let width = ui.min_rect().width().max(ERROR_BANNER_BUTTON_WIDTH);
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(Rect::from_min_size(
                                ui.cursor().min,
                                vec2(width, ui.spacing().interact_size.y),
                            ))
                            .layout(Layout::right_to_left(Align::Min)),
                        |ui| {
                            dismissed = ui
                                .add(ok)
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked();
                        },
                    );
                });
        });
    dismissed
}

/// Плашка прогресса добавления треков по центру сверху, поверх всего окна — там же, где
/// ошибки. `progress` — сколько обработано и сколько всего; `None`, пока идёт обход папки
/// и общее число неизвестно. Возвращает, нажали ли «Cancel», и нижний край плашки —
/// чтобы плашку ошибки поставить под ней, а не поверх.
pub fn loading_banner(ctx: &egui::Context, progress: Option<(usize, usize)>) -> (bool, f32) {
    let mut cancel = false;
    let response = egui::Area::new(egui::Id::unique("loading_banner"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, BANNER_TOP))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(BANNER_FILL)
                .stroke(Stroke::new(1.0, BANNER_BORDER))
                .corner_radius(12)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    // Ширина постоянная: иначе плашка дёргалась бы при смене цифр
                    ui.set_width(LOADING_BANNER_WIDTH);
                    ui.horizontal(|ui| {
                        let text = match progress {
                            Some((done, total)) => format!("Adding {done} of {total}"),
                            None => {
                                ui.add(egui::Spinner::new().size(12.0).color(TEXT_MUTED));
                                "Scanning folder…".to_owned()
                            }
                        };
                        ui.add(
                            Label::new(RichText::new(text).size(13.0).color(TEXT))
                                .selectable(false),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let button = egui::Button::new(
                                RichText::new("Cancel").size(13.0).color(TEXT_MUTED),
                            )
                            .frame(false);
                            cancel = ui
                                .add(button)
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked();
                        });
                    });

                    // Полоса прогресса — в стиле ползунка плеера. Пока общее число неизвестно,
                    // её нет: вместо неё спиннер в строке выше
                    if let Some((done, total)) = progress {
                        ui.add_space(8.0);
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
                        let fraction = if total == 0 {
                            1.0
                        } else {
                            done as f32 / total as f32
                        };
                        let filled = Rect::from_min_size(
                            rect.min,
                            vec2(rect.width() * fraction, rect.height()),
                        );
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(255), SLIDER_TRACK);
                        ui.painter()
                            .rect_filled(filled, CornerRadius::same(255), TEXT);
                    }
                });
        })
        .response;
    (cancel, response.rect.bottom())
}
