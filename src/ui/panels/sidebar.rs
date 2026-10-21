//! Левая панель: сверху медиатека — заголовок с кнопкой «+» и разделы (Artists, Albums,
//! Songs), внизу — кнопка настроек. Выбранный раздел открывается в центральной области.

use eframe::egui::{self, Align, Ui};

use crate::ui::{
    action::Action,
    app::PlayerApp,
    components,
    i18n::t,
    loading::Destination,
    navigation::{Route, Section},
    style::theme::TEXT,
};

impl PlayerApp {
    /// Вся левая панель
    pub(crate) fn render_sidebar(&mut self, ui: &mut Ui) {
        self.render_library_nav(ui);
        egui::Panel::bottom("library_sidebar_bottom")
            .frame(egui::Frame::new().inner_margin(16.0))
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| self.render_settings_button(ui));
    }

    /// Кнопка «Settings» — заголовок того же вида, что «Library» и «Playlist»: это отдельный
    /// раздел, а не ещё один раздел медиатеки, и он не должен сливаться с её пунктами
    fn render_settings_button(&mut self, ui: &mut Ui) {
        if components::title_button(ui, t().settings).clicked() {
            self.settings_open = true;
        }
    }

    /// Медиатека: заголовок с «+» и разделы
    fn render_library_nav(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = components::block_title(ui, t().library, Align::LEFT, TEXT);
            self.render_add_button(ui, row, Destination::Library);

            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for section in Section::LIBRARY {
                let selected = self.route.section() == section;
                if components::nav_item(ui, section.title(), section.title(), selected).clicked() {
                    // Пункт меню всегда открывает раздел целиком: из профиля исполнителя
                    // или альбома — обратно к верхнему уровню
                    self.dispatch(Action::Navigate(Route::from(section)));
                }
            }
        });
    }
}
