//! Пункты навигации (меню библиотеки) и плашка текущего трека.

use eframe::egui::{self, Align2, CursorIcon, FontId, Response, RichText, Sense, Ui, pos2, vec2};

use crate::ui::theme::{
    HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, TEXT, TEXT_MUTED, TRACK_ACTIVE_FADE_SECS,
};

const NAV_ITEM_HEIGHT: f32 = 32.0;
const NAV_ITEM_PADDING: f32 = 10.0;
pub const NOW_PLAYING_BAR_HEIGHT: f32 = 52.0;
const NOW_PLAYING_BAR_PADDING: f32 = 14.0;

/// Пункт навигации во всю ширину: при наведении плавно подсвечивается, выбранный —
/// подсвечен как играющий трек в списке
pub fn nav_item(ui: &mut Ui, id_salt: &str, label: &str, selected: bool) -> Response {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), NAV_ITEM_HEIGHT), Sense::hover());
    let id = ui.scope_id().with(id_salt);
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);

    let hover = ui.ctx().animate_bool_with_time_and_easing(
        id.with("hover"),
        response.hovered() && !selected,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    let active = ui.ctx().animate_bool_with_time_and_easing(
        id.with("active"),
        selected,
        TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let fill = HIGHLIGHT
        .gamma_multiply(hover)
        .lerp_to_gamma(HIGHLIGHT_STRONG, active);
    ui.painter().rect_filled(rect, 8, fill);
    ui.painter().text(
        rect.left_center() + vec2(NAV_ITEM_PADDING, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(15.0),
        TEXT_MUTED.lerp_to_gamma(TEXT, active),
    );
    response
}

/// Плашка текущего трека во всю ширину: название и исполнитель. Кликабельна целиком.
/// Текст рисуется прямо на плашке, а не виджетами `Label`: обрезанный `Label` сам ловит
/// наведение (ради подсказки с полным текстом) и отнимал бы его у плашки — подсветка
/// мигала бы, пока курсор переходит с текста на пустое место
pub fn now_playing_bar(ui: &mut Ui, title: &str, subtitle: &str) -> Response {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), NOW_PLAYING_BAR_HEIGHT),
        Sense::hover(),
    );
    let id = ui.scope_id().with("now_playing_bar");
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);

    let hover = ui.ctx().animate_bool_with_time_and_easing(
        id.with("hover"),
        response.hovered(),
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    ui.painter()
        .rect_filled(rect, 12, HIGHLIGHT.lerp_to_gamma(HIGHLIGHT_STRONG, hover));

    // Строка, обрезанная с «…» по ширине плашки
    const LINE_GAP: f32 = 2.0;
    let text_width = rect.width() - 2.0 * NOW_PLAYING_BAR_PADDING;
    let line = |text: &str, size: f32, color| {
        egui::WidgetText::from(RichText::new(text).size(size).color(color)).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            text_width,
            FontId::proportional(size),
        )
    };
    let title = line(title, 15.0, TEXT);
    let subtitle = (!subtitle.is_empty()).then(|| line(subtitle, 13.0, TEXT_MUTED));

    // Блок строк — по центру плашки по высоте
    let block_height = title.size().y
        + subtitle
            .as_ref()
            .map_or(0.0, |galley| LINE_GAP + galley.size().y);
    let mut pos = pos2(
        rect.left() + NOW_PLAYING_BAR_PADDING,
        rect.center().y - block_height / 2.0,
    );
    let title_height = title.size().y;
    ui.painter().galley(pos, title, TEXT);
    if let Some(subtitle) = subtitle {
        pos.y += title_height + LINE_GAP;
        ui.painter().galley(pos, subtitle, TEXT_MUTED);
    }
    response
}
