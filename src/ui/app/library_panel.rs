//! Левая панель: библиотека — заголовок с кнопкой «+» и навигация по видам библиотеки.
//! Выбранный вид показывается в центральной области.

use eframe::egui::{self, Align, Ui};

use super::{CentralView, PlayerApp, loading::Destination};
use crate::ui::{theme::TEXT, widgets};

impl PlayerApp {
    pub(super) fn render_library(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = widgets::block_title(ui, "Library", Align::LEFT, TEXT);
            self.render_add_button(ui, row, Destination::Library);

            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for view in [
                CentralView::Artists,
                CentralView::Albums,
                CentralView::Songs,
            ] {
                if widgets::nav_item(ui, view.title(), view.title(), self.view == view).clicked() {
                    self.set_view(view);
                    // Пункт меню всегда открывает вид целиком: из профиля исполнителя
                    // или альбома — обратно к верхнему уровню
                    self.selected_artist = None;
                    self.selected_album = None;
                }
            }
        });
    }
}
