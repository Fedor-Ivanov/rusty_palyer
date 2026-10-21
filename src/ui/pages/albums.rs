//! Альбомы сеткой — все или одного исполнителя (в его профиле).

use eframe::egui::{Align, Ui};

use super::{
    TILE_RADIUS, placeholder,
    search::{contains_query, normalized_query},
    shared::{GroupAction, Target, group_action, group_context_menu, overlay_play_button},
};
use crate::{
    core::library::groups::AlbumGroup,
    ui::action::Action,
    ui::{
        app::{ListFocus, PlayerApp},
        components::{self, TileLine},
        covers::CoverSize,
        dialogs::remove::GroupKind,
        i18n::t,
        navigation::{AlbumKey, Route},
    },
};

impl PlayerApp {
    /// Все альбомы библиотеки
    pub(crate) fn render_albums(&mut self, ui: &mut Ui) {
        self.render_album_grid(ui, "albums", None);
    }

    /// Альбомы сеткой: обложка, название, исполнитель и год. С `artist` — только альбомы
    /// этого исполнителя (имя без учёта регистра, как при группировке)
    pub(crate) fn render_album_grid(&mut self, ui: &mut Ui, id_salt: &str, artist: Option<&str>) {
        self.core.refresh_groups();
        // Чей профиль — для перехода в альбом из него; дальше имя нужно для сравнения
        let profile = artist.map(str::to_owned);
        let artist = artist.map(str::to_lowercase);
        // Поиск по названию — только во всех альбомах, в профиле исполнителя поля нет
        let query = match artist {
            Some(_) => String::new(),
            None => normalized_query(&self.search.query),
        };
        let albums: Vec<&AlbumGroup> = self
            .core
            .groups
            .albums
            .iter()
            .filter(|album| {
                artist
                    .as_ref()
                    .is_none_or(|artist| album.artist.to_lowercase() == *artist)
                    && contains_query(&album.title, &query)
            })
            .collect();
        // Плейлисты для подменю «Add to playlist ›»
        let playlists = self.playlist_entries();
        let covers = &mut self.covers;
        if albums.is_empty() {
            let text = if artist.is_some() {
                t().no_albums
            } else if self.core.groups.albums.is_empty() {
                t().library_empty
            } else {
                t().no_results
            };
            placeholder(ui, text);
            return;
        }
        // У каждой сетки своё выделение. Оно относится к содержимому: у профиля исполнителя —
        // к его альбомам, в поиске — к найденным; после изменения библиотеки номера плиток
        // значат другое
        let scope = format!(
            "{}/{}/{query}",
            artist.as_deref().unwrap_or_default(),
            self.core.library.revision()
        );
        let grid = if id_salt == "artist_albums" {
            &mut self.artist_albums_grid
        } else {
            &mut self.albums_grid
        };
        grid.set_scope(&scope);

        // Сетка заимствует альбомы и кэш обложек — действие запоминаем и выполняем после неё.
        // Клик по плитке открывает альбом; воспроизведение — кнопкой на обложке и из меню
        let mut action = None;
        let response = grid.show(
            ui,
            albums.len(),
            3,
            Align::Min,
            self.focus == ListFocus::Grid,
            |ui, index, image_rect, tile| {
                let album = albums[index];
                let cover = covers.get(&album.cover_candidates, CoverSize::Thumbnail);
                components::paint_cover(ui, image_rect, cover, TILE_RADIUS);

                // Кнопка play в левом верхнем углу обложки — проявляется при наведении на плитку
                // и играет только этот альбом, независимо от выделения
                // Кнопка play на обложке играет только этот альбом, независимо от выделения
                if overlay_play_button(ui, image_rect, tile, &format!("play_album {index}")) {
                    action = Some((GroupAction::Play, Target::One(index)));
                }

                if let Some(menu_action) = group_context_menu(tile, GroupKind::Album, &playlists) {
                    action = Some((menu_action, Target::Selection));
                }

                vec![
                    TileLine {
                        text: album.title.clone(),
                        primary: true,
                    },
                    TileLine {
                        text: album.artist.clone(),
                        primary: false,
                    },
                    TileLine {
                        text: album.year.map(|year| year.to_string()).unwrap_or_default(),
                        primary: false,
                    },
                ]
            },
        );
        if response.interacted {
            self.focus = ListFocus::Grid;
        }
        if response.delete {
            action = Some((GroupAction::Remove, Target::Selection));
        }

        if let Some(index) = response.opened {
            let album = AlbumKey::of(albums[index]);
            self.dispatch(Action::Navigate(match profile {
                Some(artist) => Route::ArtistAlbum { artist, album },
                None => Route::Album(album),
            }));
        } else if let Some((action, target)) = action {
            // Альбомы — в порядке сетки, их треки — альбом за альбомом
            let targets: Vec<usize> = match target {
                Target::One(index) => vec![index],
                Target::Selection => {
                    let grid = if id_salt == "artist_albums" {
                        &self.artist_albums_grid
                    } else {
                        &self.albums_grid
                    };
                    grid.selection().iter().copied().collect()
                }
            };
            let titles = targets.iter().map(|&i| albums[i].title.clone()).collect();
            let tracks: Vec<usize> = targets
                .iter()
                .flat_map(|&i| albums[i].tracks.iter().copied())
                .collect();
            self.dispatch(group_action(action, GroupKind::Album, titles, tracks));
        }
    }
}
