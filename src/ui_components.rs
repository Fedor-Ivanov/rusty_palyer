use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Label, Layout, Pos2, Rangef, Rect, Response,
    RichText, Sense, Stroke, TextureHandle, Ui, vec2,
};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::theme::{
    BANNER_BORDER, BANNER_FILL, HIGHLIGHT, RED, RESIZE_LINE, SLIDER_TRACK, TEXT, TEXT_FAINT,
    TEXT_MUTED,
};

const COVER_MAX_SIZE: f32 = 250.0;
pub const HOVER_FADE_SECS: f32 = 0.15;
pub const TRACK_ACTIVE_FADE_SECS: f32 = 0.2;
const BLOCK_TITLE_SIZE: f32 = 16.0;
const MENU_TEXT_SIZE: f32 = 15.0;
const RESIZE_LINE_WIDTH: f32 = 1.0;
const ERROR_BANNER_MAX_WIDTH: f32 = 440.0;
const ERROR_BANNER_BUTTON_WIDTH: f32 = 48.0;
/// Плашки сверху окна (ошибка, прогресс): отступ от верхнего края и зазор между ними
pub const BANNER_TOP: f32 = 16.0;
pub const BANNER_GAP: f32 = 8.0;
const LOADING_BANNER_WIDTH: f32 = 300.0;
const COVER_RADIUS: u8 = 12;

pub fn album_cover(ui: &mut Ui, cover_data: Option<&[u8]>) {
    if let Some(cover) = cover_data {
        let cache_id = egui::Id::unique("album_cover");
        let cover_hash = {
            let mut hasher = DefaultHasher::new();
            cover.hash(&mut hasher);
            hasher.finish()
        };
        let cached = ui
            .ctx()
            .data(|data| data.get_temp::<(u64, TextureHandle)>(cache_id))
            .filter(|(hash, _)| *hash == cover_hash)
            .map(|(_, texture)| texture);

        let texture = match cached {
            Some(texture) => texture,
            None => {
                let Ok(image) = image::load_from_memory(cover) else {
                    return;
                };
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let texture = ui.ctx().load_texture(
                    "album_cover",
                    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx().data_mut(|data| {
                    data.insert_temp(cache_id, (cover_hash, texture.clone()));
                });
                texture
            }
        };

        let size = texture.size_vec2();
        let max_size = vec2(ui.available_width(), COVER_MAX_SIZE);
        let scale = (max_size / size).min_elem().min(1.0);
        ui.add(egui::Image::new((texture.id(), size * scale)).corner_radius(COVER_RADIUS));
    } else {
        // Временная заглушка: квадрат с виниловой пластинкой
        let side = ui.available_width().min(COVER_MAX_SIZE);
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(COVER_RADIUS), HIGHLIGHT);

        // Размеры — доли стороны квадрата
        let center = rect.center();
        let r = |fraction: f32| side * fraction;
        let disc = Color32::from_rgb(0x12, 0x12, 0x14);
        let groove = Stroke::new(1.0, TEXT_FAINT.gamma_multiply(0.3));
        painter.circle_filled(center, r(0.42), disc);
        painter.circle_stroke(center, r(0.42), groove);
        for fraction in [0.37, 0.32, 0.27, 0.22] {
            painter.circle_stroke(center, r(fraction), groove);
        }
        painter.circle_filled(center, r(0.14), TEXT_FAINT);
        painter.circle_filled(center, r(0.025).max(2.0), disc);
    }
}

/// Форма области, в которой кнопка реагирует на курсор
pub enum HitShape {
    Rect,
    /// Круг, вписанный в прямоугольник кнопки
    Circle,
}

pub struct IconButton {
    pub response: Response,
    pub clicked: bool,
    /// Плавный hover для отрисовки: 0.0 — без наведения, 1.0 — наведено
    pub hover: f32,
}

/// Кнопка-иконка: клик, курсор-рука и плавный hover — только внутри `shape`
pub fn icon_button(
    ui: &mut Ui,
    rect: Rect,
    shape: HitShape,
    id_salt: &str,
    enabled: bool,
) -> IconButton {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.scope_id().with(id_salt), sense);

    // egui проверяет попадание по прямоугольнику, круг уточняем сами
    let inside = |pos: Pos2| match shape {
        HitShape::Rect => true,
        HitShape::Circle => pos.distance(rect.center()) <= rect.width() / 2.0,
    };
    let hovered = enabled && response.hover_pos().is_some_and(inside);
    let clicked = response.clicked() && response.interact_pointer_pos().is_some_and(inside);

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        hovered,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );

    IconButton {
        response,
        clicked,
        hover,
    }
}

/// Кнопка-переключатель в панели списка. Возвращает клик и плавные (0.0..=1.0)
/// значения включения и наведения для отрисовки иконки.
pub fn toggle_button(
    ui: &mut Ui,
    rect: Rect,
    id_salt: &str,
    on: bool,
    tooltip: &str,
) -> (bool, f32, f32) {
    let button = icon_button(ui, rect, HitShape::Circle, id_salt, true);
    let active = ui.ctx().animate_bool_with_time_and_easing(
        button.response.id.with("active"),
        on,
        TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let clicked = button.clicked;
    button.response.on_hover_text(tooltip);
    (clicked, active, button.hover)
}

/// Заголовок блока (панели) с выравниванием `align`: `Align::Min` — слева,
/// `Align::Center` — по центру, `Align::Max` — справа.
/// Возвращает прямоугольник строки во всю ширину — по нему можно поставить кнопки
/// рядом с заголовком.
pub fn block_title(ui: &mut Ui, text: &str, align: Align, color: Color32) -> Rect {
    ui.with_layout(Layout::top_down(align), |ui| {
        // Строка во всю ширину при любом выравнивании, иначе прямоугольник
        // был бы размером с текст
        ui.set_min_width(ui.available_width());
        let title = RichText::new(text).size(BLOCK_TITLE_SIZE).color(color);
        ui.add(Label::new(title).selectable(false));
    })
    .response
    .rect
}

/// Линия изменения ширины боковой панели — на всю высоту границы панелей, видна всегда
pub fn paint_resize_line(ui: &Ui, edge_x: f32, y_range: Rangef) {
    ui.painter()
        .vline(edge_x, y_range, Stroke::new(RESIZE_LINE_WIDTH, RESIZE_LINE));
}

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

/// Оформление пунктов меню: крупнее стандартных — больше шрифт и поля вокруг текста.
/// Вызывать в начале содержимого меню, до `menu_item`
pub fn menu_style(ui: &mut Ui) {
    ui.spacing_mut().button_padding = vec2(12.0, 7.0);
    ui.spacing_mut().item_spacing.y = 2.0;
}

/// Пункт меню
pub fn menu_item(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).size(MENU_TEXT_SIZE)))
}
