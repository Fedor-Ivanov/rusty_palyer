//! Окна сохранённых плейлистов: название (сохранить очередь, новый плейлист,
//! переименовать) и подтверждение удаления.

use eframe::egui::{self, Align, Key, Label, Layout, RichText, TextEdit};

use crate::{
    core::playlists::PlaylistId,
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self, ButtonStyle},
        i18n::{fill, t},
        style::theme::TEXT,
    },
};

const DIALOG_WIDTH: f32 = 340.0;

/// Окно названия плейлиста — пока оно открыто
pub(crate) struct NameDialog {
    pub(crate) purpose: NamePurpose,
    /// Что введено в поле
    pub(crate) name: String,
    /// Курсор ставится в поле один раз, при открытии
    focused: bool,
}

/// Зачем спрашиваем название
pub(crate) enum NamePurpose {
    /// Сохранить очередь как плейлист
    SaveQueue,
    /// Новый плейлист из треков медиатеки (их номера)
    NewPlaylist(Vec<usize>),
    /// Переименовать плейлист
    Rename(PlaylistId),
}

impl NameDialog {
    pub(crate) fn new(purpose: NamePurpose, name: String) -> Self {
        Self {
            purpose,
            name,
            focused: false,
        }
    }
}

impl PlayerApp {
    /// Окно названия плейлиста. «Save» (или Enter) отправляет действие по цели окна;
    /// пустое название сохранить нельзя. «Cancel», Esc и клик мимо закрывают окно
    pub(crate) fn render_playlist_name_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.name_dialog else {
            return;
        };
        let title = match dialog.purpose {
            NamePurpose::SaveQueue => t().save_playlist_title,
            NamePurpose::NewPlaylist(_) => t().new_playlist_title,
            NamePurpose::Rename(_) => t().rename_playlist_title,
        };
        let mut save = false;
        let mut cancel = false;
        let modal = components::modal("playlist_name").show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.add(Label::new(RichText::new(title).size(17.0).color(TEXT)).selectable(false));
            ui.add_space(12.0);

            let field = ui.add(
                TextEdit::singleline(&mut dialog.name)
                    .hint_text(t().playlist_name_hint)
                    .font(egui::FontId::proportional(15.0))
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(f32::INFINITY),
            );
            if !dialog.focused {
                field.request_focus();
                dialog.focused = true;
            }
            let has_name = !dialog.name.trim().is_empty();
            // Enter в поле — то же, что «Save»
            if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) && has_name {
                save = true;
            }

            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // В right_to_left первой добавляется самая правая кнопка
                ui.add_enabled_ui(has_name, |ui| {
                    if components::pill_button(ui, t().save, ButtonStyle::Primary).clicked() {
                        save = true;
                    }
                });
                if components::pill_button(ui, t().cancel, ButtonStyle::Secondary).clicked() {
                    cancel = true;
                }
            });
        });

        if save && let Some(dialog) = self.name_dialog.take() {
            let name = dialog.name.trim().to_owned();
            self.dispatch(match dialog.purpose {
                NamePurpose::SaveQueue => Action::SaveQueueAsPlaylist(name),
                NamePurpose::NewPlaylist(tracks) => Action::CreatePlaylist { name, tracks },
                NamePurpose::Rename(id) => Action::RenamePlaylist { id, name },
            });
        } else if cancel || modal.should_close() {
            self.name_dialog = None;
        }
    }

    /// Подтверждение удаления плейлистов — пока ждём ответа. Один плейлист называется
    /// по имени, несколько — числом
    pub(crate) fn render_playlist_deletion_dialog(&mut self, ctx: &egui::Context) {
        // Уже удалённые (например, на другой странице) не считаем
        let playlists = &self.core.playlists;
        self.pending_playlist_deletion
            .retain(|&id| playlists.get(id).is_some());
        let (title, message) = match self.pending_playlist_deletion.as_slice() {
            [] => return,
            [id] => {
                let name = playlists.get(*id).map_or("", |playlist| &playlist.name);
                (
                    t().delete_playlist_title.to_owned(),
                    fill(t().delete_playlist_message, &[("playlist", name)]),
                )
            }
            ids => {
                let count = t().count(ids.len(), &t().playlist_count);
                let values = [("playlists", count.as_str())];
                (
                    fill(t().delete_playlists_title, &values),
                    fill(t().delete_playlists_message, &values),
                )
            }
        };
        let answer =
            components::confirm_dialog(ctx, "delete_playlist", &title, &message, t().delete);
        match answer {
            components::Confirm::Pending => {}
            components::Confirm::Cancelled => self.pending_playlist_deletion.clear(),
            components::Confirm::Confirmed => {
                let ids = std::mem::take(&mut self.pending_playlist_deletion);
                self.dispatch(Action::DeletePlaylists(ids));
            }
        }
    }
}
