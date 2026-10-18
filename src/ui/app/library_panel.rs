//! Левая панель: библиотека и кнопка «+» с меню добавления.

use eframe::egui::{self, Align, Rect, Ui, pos2, vec2};

use super::{ADD_BUTTON_SIZE, PlayerApp};
use crate::ui::{icons::paint_add, theme::TEXT, widgets};

impl PlayerApp {
    /// Содержимое самой левой боковой панели
    pub(super) fn render_sidebar(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = widgets::block_title(ui, "Library", Align::LEFT, TEXT);

            // «+» прижат к правому краю строки, заголовок при этом остаётся по центру
            let add_rect = Rect::from_center_size(
                pos2(row.right() - ADD_BUTTON_SIZE / 2.0, row.center().y),
                vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
            );
            let add = widgets::icon_button(ui, add_rect, widgets::HitShape::Circle, "add", true);
            paint_add(ui, add_rect, add.hover);

            // Меню под «+», выровнено по его правому краю. Открывается и закрывается
            // по нашему `clicked` — он учитывает только клики внутри круга
            egui::Popup::menu(&add.response)
                .open_memory(add.clicked.then_some(egui::SetOpenCommand::Toggle))
                .align(egui::RectAlign::BOTTOM_END)
                .gap(4.0)
                .show(|ui| {
                    widgets::menu_style(ui);
                    if widgets::menu_item(ui, "Add files").clicked() {
                        self.add_files();
                    }
                    if widgets::menu_item(ui, "Add folder").clicked() {
                        self.add_folder();
                    }
                });
        });
    }
}
