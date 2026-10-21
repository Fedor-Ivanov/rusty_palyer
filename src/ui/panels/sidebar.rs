//! Левая панель: библиотека — заголовок с кнопкой «+» и навигация по видам библиотеки.
//! Выбранный вид показывается в центральной области.

use eframe::egui::{self, Align, Ui};

use super::{
    PlayerApp,
    loading::Destination,
    route::{Route, Section},
};
use crate::ui::{i18n::t, theme::TEXT, widgets};

impl PlayerApp {
    pub(super) fn render_library(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = widgets::block_title(ui, t().library, Align::LEFT, TEXT);
            self.render_add_button(ui, row, Destination::Library);

            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for section in Section::LIBRARY {
                let selected = self.route.section() == section;
                if widgets::nav_item(ui, section.title(), section.title(), selected).clicked() {
                    // Пункт меню всегда открывает раздел целиком: из профиля исполнителя
                    // или альбома — обратно к верхнему уровню
                    self.navigate(Route::from(section));
                }
            }
        });
    }
}
