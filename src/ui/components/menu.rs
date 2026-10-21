//! Оформление выпадающих и контекстных меню.

use eframe::egui::{self, Response, RichText, Ui, vec2};

const MENU_TEXT_SIZE: f32 = 15.0;

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
