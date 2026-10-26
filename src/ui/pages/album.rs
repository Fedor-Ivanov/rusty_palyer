//! Страница альбома: обложка, название, исполнитель, кнопки и список треков.

use eframe::egui::{self, Label, RichText, Sense, Ui, vec2};

use super::{ARTIST_NAME_SIZE, TILE_RADIUS, shared::track_menu};
use crate::{
    core::library::Track,
    ui::{
        action::Action,
        app::{ListFocus, PlayerApp},
        components::{self, ButtonStyle},
        covers::CoverSize,
        i18n::t,
        navigation::{AlbumKey, Route},
        style::theme::{TEXT, TEXT_MUTED},
    },
};

/// Обложка на странице альбома
pub(crate) const ALBUM_PAGE_COVER_SIZE: f32 = 160.0;

impl PlayerApp {
    /// Страница альбома: обложка, название, исполнитель, год, кнопки «Play» и
    /// «Add to playlist» и ниже — список его треков
    pub(crate) fn render_album_page(&mut self, ui: &mut Ui, key: &AlbumKey) {
        self.core.refresh_groups();
        let Some(album) = self
            .core
            .groups
            .albums
            .iter()
            .find(|album| key.matches(album))
        else {
            // Альбома больше нет — его треки убрали из медиатеки: уровень выше
            self.dispatch(Action::Navigate(self.route.parent()));
            return;
        };
        let indices = album.tracks.clone();
        let (title, artist, year, cover_candidates) = (
            album.title.clone(),
            album.artist.clone(),
            album.year,
            album.cover_candidates.clone(),
        );

        // ---- шапка
        let mut play_all = false;
        let mut add_all = false;
        let mut open_artist = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let (cover_rect, _) = ui.allocate_exact_size(
                        vec2(ALBUM_PAGE_COVER_SIZE, ALBUM_PAGE_COVER_SIZE),
                        Sense::hover(),
                    );
                    let cover = self.covers.get(&cover_candidates, CoverSize::Large);
                    components::paint_cover(ui, cover_rect, cover, TILE_RADIUS);
                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.add(
                            Label::new(RichText::new(&title).size(ARTIST_NAME_SIZE).color(TEXT))
                                .selectable(false)
                                .truncate(),
                        );
                        // Имя исполнителя — ссылка на его профиль
                        open_artist =
                            components::text_link(ui, &artist, 16.0, TEXT_MUTED).clicked();
                        let tracks = t().count(indices.len(), &t().track_count);
                        let details = match year {
                            Some(year) => format!("{year} · {tracks}"),
                            None => tracks,
                        };
                        ui.add(
                            Label::new(RichText::new(details).size(13.0).color(TEXT_MUTED))
                                .selectable(false),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            play_all = components::pill_button(ui, t().play, ButtonStyle::Primary)
                                .clicked();
                            add_all = components::pill_button(
                                ui,
                                t().add_to_queue,
                                ButtonStyle::Secondary,
                            )
                            .clicked();
                        });
                    });
                });
            });

        // ---- треки альбома
        let tracks: Vec<Track> = indices
            .iter()
            .filter_map(|&index| self.core.library.tracks().get(index).cloned())
            .collect();
        let current_path = self
            .core
            .controller
            .player()
            .current_track
            .as_ref()
            .map(|track| track.path.clone());
        let library = &self.core.library;
        // Меню знает только, сколько строк выделено: какие — узнаём после списка
        let mut menu = None;
        // Плейлисты для подменю «Add to playlist ›»
        let playlists = self.playlist_entries();
        let response = self.album_tracks_view.show(
            ui,
            &tracks,
            |index| components::RowState {
                // Подсвечен трек, который сейчас играет
                current: current_path.as_ref() == Some(&tracks[index].path),
                missing: library.is_missing(indices[index]),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                // «Go to album» здесь не нужен: мы на странице альбома
                if let Some(item) = track_menu(ui, selected, &playlists, None) {
                    menu = Some(item);
                }
            },
        );
        if response.interacted {
            self.focus = ListFocus::Library;
        }

        // Номера выделенных треков альбома — в номера треков библиотеки
        let selected: Vec<usize> = self
            .album_tracks_view
            .selection()
            .iter()
            .filter_map(|&index| indices.get(index).copied())
            .collect();

        if open_artist {
            // Как из «Now Playing»: профиль исполнителя в Artists, крошки
            // «Artists › Исполнитель» ведут оттуда дальше
            self.dispatch(Action::Navigate(Route::Artist(artist)));
            return;
        }
        let action = if play_all {
            Some(Action::PlayLibraryTracks(indices))
        } else if add_all {
            Some(Action::AddToQueue(indices))
        } else {
            match menu {
                Some(item) => Some(item.into_action(selected)),
                None if response.delete => Some(Action::RequestSongsRemoval(selected)),
                // Двойной клик играет только этот трек: он встаёт в очередь, а не заменяет
                // её альбомом — для альбома целиком есть кнопки «Play» и «Add to queue»
                None => response
                    .activated
                    .map(|index| Action::PlayFromLibrary(indices[index])),
            }
        };
        if let Some(action) = action {
            self.dispatch(action);
        }
    }
}
