//! Заголовки блоков и линии на границах панелей.

use eframe::egui::{Align, Color32, Label, Layout, Rangef, Rect, RichText, Stroke, Ui};

use crate::ui::theme::RESIZE_LINE;

const BLOCK_TITLE_SIZE: f32 = 16.0;
const RESIZE_LINE_WIDTH: f32 = 1.0;

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
