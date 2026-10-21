//! Выпадающий список.

use eframe::egui::{self, Align2, CursorIcon, FontId, RectAlign, Sense, Stroke, Ui, pos2, vec2};

use super::menu::{menu_item, menu_style};
use crate::ui::theme::{HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, TEXT, TEXT_MUTED};

const SELECT_SIZE: egui::Vec2 = vec2(160.0, 32.0);
const SELECT_PADDING: f32 = 12.0;
const SELECT_TEXT_SIZE: f32 = 14.0;
/// Стрелка вниз у правого края: полуширина и полувысота «галочки»
const CHEVRON_HALF_SIZE: egui::Vec2 = vec2(4.0, 2.0);

/// Выпадающий список: плашка с выбранным вариантом и стрелкой, по клику — меню всех
/// вариантов `options` (значение и подпись). Возвращает вариант, если выбрали другой
pub fn select<T: Copy + PartialEq>(ui: &mut Ui, current: T, options: &[(T, &str)]) -> Option<T> {
    let (rect, response) = ui.allocate_exact_size(SELECT_SIZE, Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        response.hovered(),
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );

    let painter = ui.painter();
    painter.rect_filled(rect, 8, HIGHLIGHT.lerp_to_gamma(HIGHLIGHT_STRONG, hover));
    let label = options
        .iter()
        .find(|(value, _)| *value == current)
        .map_or("", |(_, label)| label);
    painter.text(
        rect.left_center() + vec2(SELECT_PADDING, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(SELECT_TEXT_SIZE),
        TEXT,
    );
    let chevron = pos2(
        rect.right() - SELECT_PADDING - CHEVRON_HALF_SIZE.x,
        rect.center().y,
    );
    let (dx, dy) = (CHEVRON_HALF_SIZE.x, CHEVRON_HALF_SIZE.y);
    painter.line(
        vec![
            chevron + vec2(-dx, -dy),
            chevron + vec2(0.0, dy),
            chevron + vec2(dx, -dy),
        ],
        Stroke::new(1.5, TEXT_MUTED),
    );

    // Меню под плашкой, той же ширины
    let mut chosen = None;
    egui::Popup::menu(&response)
        .align(RectAlign::BOTTOM_START)
        .width(rect.width())
        .gap(4.0)
        .show(|ui| {
            menu_style(ui);
            for &(value, label) in options {
                if menu_item(ui, label).clicked() && value != current {
                    chosen = Some(value);
                }
            }
        });
    chosen
}
