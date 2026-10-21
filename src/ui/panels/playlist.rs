//! Правая панель: плейлист — что слушаем сейчас. Треки сюда попадают из библиотеки или
//! кнопкой «+» мимо библиотеки. Внизу — shuffle и повтор.

use eframe::egui::{self, Align, Align2, FontId, Layout, Sense, Ui, vec2};

use crate::{
    core::playback::RepeatMode,
    ui::{
        action::Action,
        app::{ADD_BUTTON_SIZE, ListFocus, PlayerApp},
        components,
        i18n::{fill, t},
        loading::Destination,
        style::icons::{paint_repeat, paint_shuffle},
        style::theme::{TEXT, TEXT_FAINT},
    },
};

// Зазор между кнопками повтора в нижнем блоке
const ICON_BUTTON_GAP: f32 = 4.0;

impl PlayerApp {
    pub(crate) fn render_playlist(&mut self, ui: &mut Ui) {
        // Вся область панели — запоминаем до заголовка, чтобы заглушка встала по её центру
        let panel_rect = ui.max_rect();
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            // Рядом с заголовком — сколько треков в плейлисте; пустой — без числа:
            // там и так «No tracks»
            let count = self.state.controller.player().track_list.tracks.len();
            let row = if count == 0 {
                components::block_title(ui, t().playlist, Align::LEFT, TEXT)
            } else {
                let note = format!("· {}", t().count(count, &t().track_count));
                components::block_title_with_note(ui, t().playlist, &note)
            };
            self.render_add_button(ui, row, Destination::Playlist);
        });

        if self.state.controller.player().track_list.tracks.is_empty() {
            if self.loaders.is_empty() {
                ui.painter().text(
                    panel_rect.center(),
                    Align2::CENTER_CENTER,
                    t().no_tracks,
                    FontId::proportional(14.0),
                    TEXT_FAINT,
                );
            }
            return;
        }

        // Меню знает только, сколько строк выделено: какие — узнаём после списка
        let mut remove = false;
        let list = &self.state.controller.player().track_list;
        let current = list.current_index;
        let response = self.playlist_view.show(
            ui,
            &list.tracks,
            |index| components::RowState {
                current: current == Some(index),
                missing: false,
            },
            self.focus == ListFocus::Playlist,
            |ui, selected| {
                let label = match selected {
                    0 | 1 => t().remove_from_playlist.to_owned(),
                    count => fill(
                        t().remove_items_from_playlist,
                        &[("items", &t().count(count, &t().track_count))],
                    ),
                };
                if components::menu_item(ui, &label).clicked() {
                    remove = true;
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Playlist;
        }
        if remove || response.delete {
            let selected = self.playlist_view.selection().clone();
            self.dispatch(Action::RemoveFromPlaylist(selected));
        }
        if let Some(index) = response.activated {
            self.dispatch(Action::PlayPlaylistTrack(index));
        }
    }

    /// Нижний блок плейлиста: слева shuffle, справа две кнопки повтора
    pub(crate) fn render_playback_modes(&mut self, ui: &mut Ui) {
        let list = &self.state.controller.player().track_list;
        let (shuffle, repeat) = (list.shuffle, list.repeat);
        let size = vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE);

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let (clicked, active, hover) =
                components::toggle_button(ui, rect, "shuffle", shuffle, t().shuffle);
            paint_shuffle(ui, rect, active, hover);
            if clicked {
                self.dispatch(Action::ToggleShuffle);
            }

            // Повторы прижаты вправо. Включена может быть только одна из кнопок —
            // это обеспечивает сам RepeatMode
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ICON_BUTTON_GAP;
                // В right_to_left первой добавляется самая правая кнопка
                for (mode, id_salt, tooltip) in [
                    (RepeatMode::All, "repeat_all", t().repeat_all),
                    (RepeatMode::One, "repeat_one", t().repeat_one),
                ] {
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    let (clicked, active, hover) =
                        components::toggle_button(ui, rect, id_salt, repeat == mode, tooltip);
                    paint_repeat(ui, rect, mode == RepeatMode::One, active, hover);
                    if clicked {
                        self.dispatch(Action::ToggleRepeat(mode));
                    }
                }
            });
        });
    }
}
