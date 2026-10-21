//! Окно настроек. Открывается кнопкой «Settings» внизу боковой панели (`panels::sidebar`).

use eframe::egui::{self, Align, Label, Layout, RichText};

use crate::{
    core::settings::Language,
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self, ButtonStyle},
        i18n::{self, t},
        style::theme::{TEXT, TEXT_MUTED},
    },
};

/// Ширина содержимого окна настроек
const SETTINGS_WIDTH: f32 = 320.0;

impl PlayerApp {
    /// Окно настроек — пока открыто. Изменения применяются сразу и сохраняются сами
    /// (см. `save_settings_if_changed`); «Done», Esc и клик мимо окна просто закрывают его
    pub(crate) fn render_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut done = false;
        let modal = components::modal("settings").show(ctx, |ui| {
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
                    if let Some(language) =
                        components::select(ui, self.core.settings.language(), &options)
                    {
                        self.dispatch(Action::SetLanguage(language));
                    }
                });
            });

            ui.add_space(20.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                done = components::pill_button(ui, t().done, ButtonStyle::Primary).clicked();
            });
        });
        if done || modal.should_close() {
            self.settings_open = false;
        }
    }
}
