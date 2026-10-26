//! Исполнители: сетка всех исполнителей и профиль одного — шапка и его альбомы.

use eframe::egui::{self, Align, Align2, FontId, Label, Rect, RichText, Sense, Ui, vec2};

use super::{
    ARTIST_NAME_SIZE,
    album::ALBUM_PAGE_COVER_SIZE,
    placeholder,
    search::{contains_query, normalized_query},
    shared::{GroupAction, group_action, group_context_menu},
};
use crate::{
    core::artist_images::ArtistImage,
    ui::{
        action::Action,
        app::{ListFocus, PlayerApp},
        components::{self, ButtonStyle, TileLine},
        covers::{Cover, CoverCache, CoverSize},
        dialogs::remove::GroupKind,
        i18n::t,
        navigation::Route,
        style::theme::{HIGHLIGHT, HIGHLIGHT_STRONG, TEXT, TEXT_FAINT, TEXT_MUTED},
    },
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

/// Картинка исполнителя кругом; пока её нет (ищется или не нашлась) — заглушка с буквой
fn paint_artist_image(
    ui: &Ui,
    rect: Rect,
    covers: &mut CoverCache,
    image: ArtistImage,
    name: &str,
    hover: f32,
) {
    let size = if rect.width() > 200.0 {
        CoverSize::Large
    } else {
        CoverSize::Thumbnail
    };
    let cover = match &image {
        ArtistImage::File(path) => covers.get(std::slice::from_ref(path), size),
        ArtistImage::Url(url) => covers.get_url(url, size),
        ArtistImage::Searching | ArtistImage::Missing => Cover::Missing,
    };
    if let Cover::Ready(..) = cover {
        // Скругление — половина стороны: круг. Больше 255 скругление не бывает
        let radius = (rect.width() / 2.0).min(255.0) as u8;
        components::paint_cover(ui, rect, cover, radius);
        return;
    }
    paint_artist_placeholder(ui, rect, name, hover);
}

impl PlayerApp {
    /// Исполнители сеткой: картинка (фото, а пока его нет — заглушка с буквой) и имя.
    /// Клик по исполнителю открывает его профиль
    pub(crate) fn render_artists(&mut self, ui: &mut Ui) {
        self.core.refresh_groups();
        let artists = &self.core.groups.artists;
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
            .set_scope(&format!("{}/{query}", self.core.library.revision()));
        let mut action = None;
        // Плейлисты для подменю «Add to playlist ›»
        let playlists = self.playlist_entries();
        let (core, covers) = (&mut self.core, &mut self.covers);
        let response = self.artists_grid.show(
            ui,
            shown.len(),
            1,
            Align::Center,
            self.focus == ListFocus::Grid,
            |ui, index, image_rect, tile| {
                // Показанные на экране ищутся первыми — см. `ArtistImages`
                let name = core.groups.artists[shown[index]].name.clone();
                let image = core.artist_image(&name);
                paint_artist_image(ui, image_rect, covers, image, &name, tile.hover);
                if let Some(menu_action) = group_context_menu(tile, GroupKind::Artist, &playlists) {
                    action = Some(menu_action);
                }
                vec![TileLine {
                    text: name,
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
                self.core.groups.artists[shown[index]].name.clone(),
            )));
        } else if let Some(action) = action {
            // Исполнители — в порядке сетки, их треки — альбом за альбомом
            let names: Vec<String> = self
                .artists_grid
                .selection()
                .iter()
                .map(|&index| self.core.groups.artists[shown[index]].name.clone())
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
    /// «Play» (заменить плейлист его треками и играть) и «Add to playlist»
    pub(crate) fn render_artist_header(&mut self, ui: &mut Ui, artist: &str) {
        self.core.refresh_groups();
        let tracks = self.artist_track_indices(artist);
        // Сыграть можно только треки, чьи файлы на месте — их и считаем
        let playable = tracks
            .iter()
            .filter(|&&index| !self.core.library.is_missing(index))
            .count();
        let album_count = self.artist_albums(artist).count();

        let image = self.core.artist_image(artist);
        let mut play_all = false;
        let mut add_to_queue = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Фото исполнителя кругом — размером с обложку на странице альбома
                    let (image_rect, _) = ui.allocate_exact_size(
                        vec2(ALBUM_PAGE_COVER_SIZE, ALBUM_PAGE_COVER_SIZE),
                        Sense::hover(),
                    );
                    paint_artist_image(ui, image_rect, &mut self.covers, image, artist, 0.0);
                    ui.add_space(20.0);

                    ui.vertical(|ui| {
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
                                play_all =
                                    components::pill_button(ui, t().play, ButtonStyle::Primary)
                                        .clicked();
                                add_to_queue = components::pill_button(
                                    ui,
                                    t().add_to_queue,
                                    ButtonStyle::Secondary,
                                )
                                .clicked();
                            });
                        });
                    });
                });
            });

        if play_all {
            self.dispatch(Action::PlayLibraryTracks(tracks));
        } else if add_to_queue {
            self.dispatch(Action::AddToQueue(tracks));
        }
    }
}
