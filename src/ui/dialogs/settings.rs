//! Настройки: кнопка внизу боковой панели и модальное окно с ними.

use eframe::egui::{self, Align, Label, Layout, RichText, Ui};

use super::PlayerApp;
use crate::{
    settings::Language,
    ui::{
        i18n::{self, t},
        theme::{TEXT, TEXT_MUTED},
        widgets::{self, ButtonStyle},
    },
};

/// Ширина содержимого окна настроек
const SETTINGS_WIDTH: f32 = 320.0;

impl PlayerApp {
    /// Кнопка «Settings» — заголовок того же вида, что «Library» и «Playlist»: это отдельный
    /// раздел, а не ещё один вид библиотеки, и он не должен сливаться с её пунктами
    pub(super) fn render_settings_button(&mut self, ui: &mut Ui) {
        if widgets::title_button(ui, t().settings).clicked() {
            self.settings_open = true;
        }
    }

    /// Окно настроек — пока открыто. Изменения применяются сразу и сохраняются сами
    /// (см. `save_settings_if_changed`); «Done», Esc и клик мимо окна просто закрывают его
    pub(super) fn render_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut done = false;
        let modal = widgets::modal("settings").show(ctx, |ui| {
            ui.set_width(SETTINGS_WIDTH);
            ui.add(
                Label::new(RichText::new(t().settings).size(17.0).color(TEXT)).selectable(false),
            );
            ui.add_space(16.0);

            // Язык: подпись слева, список справа
            ui.horizontal(|ui| {
                ui.add(
                    Label::new(RichText::new(t().language).size(14.0).color(TEXT_MUTED))
                        .selectable(false),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let options =
                        Language::ALL.map(|language| (language, i18n::language_name(language)));
                    if let Some(language) = widgets::select(ui, self.settings.language(), &options)
                    {
                        self.settings.update_language(language);
                        // Сразу, а не со следующего кадра: окно перерисуется уже на новом языке
                        i18n::set_language(language);
                    }
                });
            });

            ui.add_space(20.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                done = widgets::pill_button(ui, t().done, ButtonStyle::Primary).clicked();
            });
        });
        if done || modal.should_close() {
            self.settings_open = false;
        }
    }
}
