//! Окно настроек. Открывается кнопкой «Settings» внизу боковой панели (`panels::sidebar`).

use eframe::egui::{self, Align, CursorIcon, Label, Layout, RichText, Sense, Stroke, Ui, vec2};

use crate::{
    core::settings::{Glass, GlassTint, Language},
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self, ButtonStyle},
        i18n::{self, t},
        style::theme::{TEXT, TEXT_FAINT, TEXT_MUTED, tint_swatch},
    },
};

/// Ширина содержимого окна настроек
const SETTINGS_WIDTH: f32 = 320.0;
/// Кружки оттенков стекла: диаметр и зазор между ними
const SWATCH_SIZE: f32 = 20.0;
const SWATCH_GAP: f32 = 8.0;
/// Ползунок прозрачности — шириной во все кружки оттенков, чтобы строки были вровень
const TRANSPARENCY_SLIDER_WIDTH: f32 =
    SWATCH_SIZE * GlassTint::ALL.len() as f32 + SWATCH_GAP * (GlassTint::ALL.len() - 1) as f32;
const SLIDER_TRACK_HEIGHT: f32 = 4.0;
const SLIDER_THUMB_RADIUS: f32 = 7.0;
/// Строка настройки — высотой с самый высокий элемент, список языка: подписи всех строк
/// по центру своих элементов, а строки идут с одним шагом
const SETTING_ROW_HEIGHT: f32 = 32.0;
/// Зазор между строками настроек
const SETTING_ROW_GAP: f32 = 6.0;

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

            setting_row(ui, t().language, |ui| {
                let options =
                    Language::ALL.map(|language| (language, i18n::language_name(language)));
                if let Some(language) =
                    components::select(ui, self.core.settings.language(), &options)
                {
                    self.dispatch(Action::SetLanguage(language));
                }
            });
            ui.add_space(SETTING_ROW_GAP);

            // Стекло: оттенок и прозрачность. Меняются сразу — окно видно за диалогом
            let glass = self.core.settings.glass();
            setting_row(ui, t().glass_tint, |ui| {
                ui.spacing_mut().item_spacing.x = SWATCH_GAP;
                // В right_to_left первым добавляется самый правый кружок
                for tint in GlassTint::ALL.into_iter().rev() {
                    if tint_button(ui, tint, glass.tint == tint) {
                        self.dispatch(Action::SetGlass(Glass { tint, ..glass }));
                    }
                }
            });
            ui.add_space(SETTING_ROW_GAP);
            setting_row(ui, t().transparency, |ui| {
                if let Some(transparency) = components::slider(
                    ui,
                    TRANSPARENCY_SLIDER_WIDTH,
                    glass.transparency,
                    SLIDER_TRACK_HEIGHT,
                    SLIDER_THUMB_RADIUS,
                ) {
                    self.dispatch(Action::SetGlass(Glass {
                        transparency,
                        ..glass
                    }));
                }
            });
            ui.add_space(SETTING_ROW_GAP);

            // Поиск в интернете: выключен, пока пользователь не включит — ниже сказано,
            // что и куда отправляется
            let online = self.core.settings.online_lookups();
            setting_row(ui, t().online_search, |ui| {
                if components::switch(ui, online) {
                    self.dispatch(Action::SetOnlineLookups(!online));
                }
            });
            ui.add(
                Label::new(
                    RichText::new(t().online_search_note)
                        .size(12.0)
                        .color(TEXT_FAINT),
                )
                .wrap()
                .selectable(false),
            );
            ui.add_space(SETTING_ROW_GAP + 6.0);

            // Версия приложения — из Cargo.toml на момент сборки
            setting_row(ui, t().version, |ui| {
                ui.add(Label::new(
                    RichText::new(env!("CARGO_PKG_VERSION"))
                        .size(14.0)
                        .color(TEXT),
                ));
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

/// Строка настройки: подпись слева, элемент управления прижат вправо
/// Высота строки задана заранее: в `horizontal` подпись центрировалась бы по высоте строки
/// на момент её добавления — до элемента справа, и рядом с высоким списком языка
/// оказывалась бы выше его середины
fn setting_row(ui: &mut Ui, label: &str, add_control: impl FnOnce(&mut Ui)) {
    let size = vec2(ui.available_width(), SETTING_ROW_HEIGHT);
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_height(SETTING_ROW_HEIGHT);
        ui.add(Label::new(RichText::new(label).size(14.0).color(TEXT_MUTED)).selectable(false));
        ui.with_layout(Layout::right_to_left(Align::Center), add_control);
    });
}

/// Кружок оттенка стекла; выбранный — в светлом кольце. Возвращает `true` по клику
fn tint_button(ui: &mut Ui, tint: GlassTint, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(SWATCH_SIZE, SWATCH_SIZE), Sense::click());
    let response = response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(i18n::tint_name(tint));
    let center = rect.center();
    let radius = SWATCH_SIZE / 2.0;
    let painter = ui.painter();
    if selected {
        // Кольцо снаружи и зазор до цвета — кружок выбранного чуть меньше
        painter.circle_stroke(center, radius - 1.0, Stroke::new(1.5, TEXT));
        painter.circle_filled(center, radius - 4.0, tint_swatch(tint));
    } else {
        let grow = if response.hovered() { 0.0 } else { 1.0 };
        painter.circle_filled(center, radius - grow, tint_swatch(tint));
    }
    response.clicked()
}
