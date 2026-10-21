//! Исполнители: сетка всех исполнителей и профиль одного — шапка и его альбомы.

use eframe::egui::{self, Align, Align2, FontId, Label, Rect, RichText, Ui};

use super::{
    ARTIST_NAME_SIZE, placeholder,
    search::{contains_query, normalized_query},
    shared::{GroupAction, group_action, group_context_menu},
};
use crate::ui::{
    action::Action,
    app::{ListFocus, PlayerApp},
    components::{self, ButtonStyle, TileLine},
    dialogs::remove::GroupKind,
    i18n::t,
    navigation::Route,
    style::theme::{HIGHLIGHT, HIGHLIGHT_STRONG, TEXT, TEXT_FAINT, TEXT_MUTED},
};

/// Заглушка картинки исполнителя: круг с первой буквой имени. При наведении круг светлеет
pub(crate) fn paint_artist_placeholder(ui: &Ui, rect: Rect, name: &str, hover: f32) {
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        rect.width() / 2.0,
        HIGHLIGHT.lerp_to_gamma(HIGHLIGHT_STRONG, hover),
    );
    let initial: String = name
        .chars()
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        initial,
        FontId::proportional(rect.width() * 0.36),
        TEXT_FAINT,
    );
}

impl PlayerApp {
    /// Исполнители сеткой: картинка (пока заглушка с первой буквой имени) и имя.
    /// Клик по исполнителю открывает его профиль
    pub(crate) fn render_artists(&mut self, ui: &mut Ui) {
        self.state.refresh_groups();
        let artists = &self.state.groups.artists;
        if artists.is_empty() {
            placeholder(ui, t().library_empty);
            return;
        }
        // Показанные исполнители — номера в `groups.artists`: поиск по имени
        let query = normalized_query(&self.search.query);
        let shown: Vec<usize> = (0..artists.len())
            .filter(|&index| contains_query(&artists[index].name, &query))
            .collect();
        if shown.is_empty() {
            placeholder(ui, t().no_results);
            return;
        }
        // Имя — по центру под круглой картинкой. Сетка заимствует исполнителей —
        // действие из меню запоминаем и выполняем после неё. Номера плиток относятся
        // к библиотеке и запросу — при их изменении выделение сбрасывается
        self.artists_grid
            .set_scope(&format!("{}/{query}", self.state.library.revision()));
        let mut action = None;
        let response = self.artists_grid.show(
            ui,
            shown.len(),
            1,
            Align::Center,
            self.focus == ListFocus::Grid,
            |ui, index, image_rect, tile| {
                let artist = &artists[shown[index]];
                paint_artist_placeholder(ui, image_rect, &artist.name, tile.hover);
                if let Some(menu_action) = group_context_menu(tile, GroupKind::Artist) {
                    action = Some(menu_action);
                }
                vec![TileLine {
                    text: artist.name.clone(),
                    primary: true,
                }]
            },
        );
        if response.interacted {
            self.focus = ListFocus::Grid;
        }
        if response.delete {
            action = Some(GroupAction::Remove);
        }

        if let Some(index) = response.opened {
            self.dispatch(Action::Navigate(Route::Artist(
                self.state.groups.artists[shown[index]].name.clone(),
            )));
        } else if let Some(action) = action {
            // Исполнители — в порядке сетки, их треки — альбом за альбомом
            let names: Vec<String> = self
                .artists_grid
                .selection()
                .iter()
                .map(|&index| self.state.groups.artists[shown[index]].name.clone())
                .collect();
            let tracks: Vec<usize> = names
                .iter()
                .flat_map(|name| self.artist_track_indices(name))
                .collect();
            self.dispatch(group_action(action, GroupKind::Artist, names, tracks));
        }
    }

    /// Профиль исполнителя: шапка и его альбомы сеткой
    pub(crate) fn render_artist_profile(&mut self, ui: &mut Ui, artist: &str) {
        self.render_artist_header(ui, artist);
        self.render_album_grid(ui, "artist_albums", Some(artist));
    }

    /// Шапка профиля исполнителя: крупное имя, сколько альбомов и песен, кнопки
    /// «Play all» (заменить плейлист его треками и играть) и «Add to playlist»
    pub(crate) fn render_artist_header(&mut self, ui: &mut Ui, artist: &str) {
        self.state.refresh_groups();
        let tracks = self.artist_track_indices(artist);
        // Сыграть можно только треки, чьи файлы на месте — их и считаем
        let playable = tracks
            .iter()
            .filter(|&&index| !self.state.library.is_missing(index))
            .count();
        let album_count = self.artist_albums(artist).count();

        let mut play_all = false;
        let mut add_to_playlist = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.add(
                    Label::new(RichText::new(artist).size(ARTIST_NAME_SIZE).color(TEXT))
                        .selectable(false)
                        .truncate(),
                );
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "{} · {}",
                            t().count(album_count, &t().album_count),
                            t().count(playable, &t().song_count),
                        ))
                        .size(13.0)
                        .color(TEXT_MUTED),
                    )
                    .selectable(false),
                );
                ui.add_space(12.0);

                // Без треков (все файлы пропали) кнопкам нечего делать
                ui.add_enabled_ui(playable > 0, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        play_all = components::pill_button(ui, t().play_all, ButtonStyle::Primary)
                            .clicked();
                        add_to_playlist = components::pill_button(
                            ui,
                            t().add_to_playlist,
                            ButtonStyle::Secondary,
                        )
                        .clicked();
                    });
                });
            });

        if play_all {
            self.dispatch(Action::PlayLibraryTracks(tracks));
        } else if add_to_playlist {
            self.dispatch(Action::AddToPlaylist(tracks));
        }
    }
}
