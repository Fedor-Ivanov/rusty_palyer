//! Виды библиотеки в центральной области: исполнители, альбомы, композиции.

use eframe::egui::{self, Align, Align2, FontId, Label, Rect, RichText, Sense, Ui, vec2};

use std::{collections::HashSet, path::PathBuf};

use super::{CentralView, LibraryGroups, ListFocus, PlayerApp};
use crate::{
    library::{
        Track,
        groups::{self, AlbumGroup},
    },
    ui::{
        covers::CoverSize,
        i18n::{fill, t},
        icons::paint_overlay_play,
        theme::{HIGHLIGHT, HIGHLIGHT_STRONG, TEXT, TEXT_FAINT, TEXT_MUTED},
        widgets::{self, ButtonStyle, TileLine},
    },
};

/// Скругление картинок в сетке
const TILE_RADIUS: u8 = 8;
/// Имя исполнителя в шапке его профиля и название альбома на странице альбома
const ARTIST_NAME_SIZE: f32 = 30.0;
/// Обложка на странице альбома
const ALBUM_PAGE_COVER_SIZE: f32 = 160.0;

/// Какой альбом открыт. Альбом — пара «название + исполнитель», как при группировке;
/// хранятся имена, а не номер: номера альбомов меняются вместе с библиотекой
#[derive(Clone)]
pub(super) struct AlbumKey {
    title: String,
    artist: String,
}

impl AlbumKey {
    pub(super) fn of(album: &AlbumGroup) -> Self {
        Self {
            title: album.title.clone(),
            artist: album.artist.clone(),
        }
    }

    /// Сравнение без учёта регистра — как при группировке
    fn matches(&self, album: &AlbumGroup) -> bool {
        album.title.to_lowercase() == self.title.to_lowercase()
            && album.artist.to_lowercase() == self.artist.to_lowercase()
    }
}

/// Поиск в виде библиотеки. Запрос один на открытый вид: при смене вида он очищается
/// (см. `PlayerApp::set_view`). Artists ищет по имени исполнителя, Albums — по названию
/// альбома, Songs — по названию трека и имени исполнителя
#[derive(Default)]
pub(super) struct LibrarySearch {
    query: String,
    /// Найденные треки Songs; `None` — запрос пуст, показана вся библиотека
    found_songs: Option<FoundSongs>,
}

impl LibrarySearch {
    pub(super) fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

/// Треки, найденные в Songs. Пересчитываются, только когда изменился запрос или библиотека:
/// в библиотеке могут быть десятки тысяч треков, а кадры рисуются постоянно
struct FoundSongs {
    revision: u64,
    /// Запрос в нижнем регистре, по которому искали
    query: String,
    /// Номера найденных треков в библиотеке
    indices: Vec<usize>,
    /// Копии найденных треков — их показывает список
    tracks: Vec<Track>,
}

/// Кнопка play поверх обложки альбома: размер и отступ от угла обложки
const OVERLAY_PLAY_SIZE: f32 = 32.0;
const OVERLAY_PLAY_INSET: f32 = 8.0;

/// Что сделать с альбомом или исполнителем — из контекстного меню или кнопкой на обложке
#[derive(Clone, Copy)]
enum GroupAction {
    /// Заменить плейлист их треками и играть
    Play,
    AddToPlaylist,
    /// Убрать их треки из библиотеки (файлы не трогает)
    Remove,
}

/// Что удаляется из библиотеки: альбом, исполнитель или отдельные треки — от этого
/// зависят тексты диалога удаления
#[derive(Clone, Copy)]
pub(super) enum GroupKind {
    Album,
    Artist,
    /// Треки, выделенные в списке (Songs или страница альбома)
    Songs,
}

/// К каким плиткам относится действие
enum Target {
    /// Одна плитка: кнопка play на обложке
    One(usize),
    /// Выделенные: контекстное меню, клавиша Delete
    Selection,
}

impl PlayerApp {
    /// Вид библиотеки `view` во всю центральную область
    pub(super) fn render_library_view(&mut self, ui: &mut Ui, view: CentralView) {
        egui::Frame::new()
            .inner_margin(16.0)
            .show(ui, |ui| self.render_breadcrumbs(ui, view));
        match (view, &self.selected_album) {
            (CentralView::Albums | CentralView::Artists, Some(album)) => {
                let album = album.clone();
                self.render_album_page(ui, &album);
            }
            (CentralView::Songs, _) => self.render_songs(ui),
            (CentralView::Artists, None) => self.render_artists(ui),
            (CentralView::Albums, None) => self.render_albums(ui),
            (CentralView::NowPlaying, _) => {}
        }
    }

    /// Заголовок вида. Внутри исполнителя или альбома — «хлебные крошки», каждая ведёт
    /// на свой уровень: «Albums › Альбом», «Artists › Исполнитель › Альбом»
    fn render_breadcrumbs(&mut self, ui: &mut Ui, view: CentralView) {
        let artist = self.selected_artist.clone();
        let album = self
            .selected_album
            .as_ref()
            .map(|album| album.title.clone());
        match (view, artist, album) {
            (CentralView::Albums, _, Some(album)) => {
                if widgets::breadcrumb_title(ui, &[view.title()], &album).is_some() {
                    self.selected_album = None;
                }
            }
            (CentralView::Artists, Some(artist), Some(album)) => {
                match widgets::breadcrumb_title(ui, &[view.title(), &artist], &album) {
                    Some(0) => {
                        self.selected_artist = None;
                        self.selected_album = None;
                    }
                    Some(_) => self.selected_album = None,
                    None => {}
                }
            }
            (CentralView::Artists, Some(artist), None) => {
                if widgets::breadcrumb_title(ui, &[view.title()], &artist).is_some() {
                    self.selected_artist = None;
                }
            }
            // Верхний уровень вида — с полем поиска справа. В пустой библиотеке искать нечего
            (CentralView::Artists | CentralView::Albums | CentralView::Songs, _, _)
                if !self.library.tracks().is_empty() =>
            {
                self.render_title_with_search(ui, view);
            }
            _ => {
                widgets::block_title(ui, view.title(), Align::LEFT, TEXT);
            }
        }
    }

    /// Заголовок вида слева и поле поиска справа. У Songs рядом с заголовком — число треков,
    /// как «Playlist · N tracks»; во время поиска — сколько найдено: «· 3 of 120 tracks»
    fn render_title_with_search(&mut self, ui: &mut Ui, view: CentralView) {
        let note = (view == CentralView::Songs).then(|| {
            self.refresh_found_songs();
            let total = self.library.tracks().len();
            let tracks = t().count(total, &t().track_count);
            match &self.search.found_songs {
                Some(found) => format!(
                    "· {}",
                    fill(
                        t().found_of,
                        &[
                            ("found", &found.indices.len().to_string()),
                            ("tracks", &tracks)
                        ]
                    )
                ),
                None => format!("· {tracks}"),
            }
        });
        let query = &mut self.search.query;
        let changed = widgets::title_with_search(ui, view.title(), query, |ui| match &note {
            Some(note) => {
                widgets::block_title_with_note(ui, view.title(), note);
            }
            None => {
                widgets::block_title(ui, view.title(), Align::LEFT, TEXT);
            }
        });
        // Выделение сеток сбрасывается само — запрос входит в их scope. Список Songs
        // показывает другие треки — его выделение ни на что не указывает
        if changed && view == CentralView::Songs {
            self.library_view.clear_selection();
        }
    }

    /// Пересчитывает найденные в Songs треки, если изменился запрос или библиотека
    fn refresh_found_songs(&mut self) {
        let query = normalized_query(&self.search.query);
        if query.is_empty() {
            self.search.found_songs = None;
            return;
        }
        let revision = self.library.revision();
        let up_to_date = self
            .search
            .found_songs
            .as_ref()
            .is_some_and(|found| found.revision == revision && found.query == query);
        if up_to_date {
            return;
        }
        let (indices, tracks) = self
            .library
            .tracks()
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                contains_query(&track.title, &query) || contains_query(&track.artist, &query)
            })
            .map(|(index, track)| (index, track.clone()))
            .unzip();
        self.search.found_songs = Some(FoundSongs {
            revision,
            query,
            indices,
            tracks,
        });
    }

    /// Пересчитывает исполнителей и альбомы, если библиотека изменилась
    pub(super) fn refresh_groups(&mut self) {
        let revision = self.library.revision();
        if self.groups.revision != Some(revision) {
            let (artists, albums) = groups::group(self.library.tracks());
            self.groups = LibraryGroups {
                revision: Some(revision),
                artists,
                albums,
            };
        }
    }

    /// Исполнители сеткой: картинка (пока заглушка с первой буквой имени) и имя.
    /// Клик по исполнителю открывает его альбомы
    fn render_artists(&mut self, ui: &mut Ui) {
        if let Some(artist) = self.selected_artist.clone() {
            self.render_artist_header(ui, &artist);
            self.render_album_grid(ui, "artist_albums", Some(&artist));
            return;
        }
        self.refresh_groups();
        let artists = &self.groups.artists;
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
            .set_scope(&format!("{}/{query}", self.library.revision()));
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
            self.selected_artist = Some(self.groups.artists[shown[index]].name.clone());
        } else if let Some(action) = action {
            // Исполнители — в порядке сетки, их треки — альбом за альбомом
            let names: Vec<String> = self
                .artists_grid
                .selection()
                .iter()
                .map(|&index| self.groups.artists[shown[index]].name.clone())
                .collect();
            let tracks: Vec<usize> = names
                .iter()
                .flat_map(|name| self.artist_track_indices(name))
                .collect();
            self.apply_group_action(action, GroupKind::Artist, names, &tracks);
        }
    }

    /// Шапка профиля исполнителя: крупное имя, сколько альбомов и песен, кнопки
    /// «Play all» (заменить плейлист его треками и играть) и «Add to playlist»
    fn render_artist_header(&mut self, ui: &mut Ui, artist: &str) {
        self.refresh_groups();
        let tracks = self.artist_tracks(artist);
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
                            t().count(tracks.len(), &t().song_count),
                        ))
                        .size(13.0)
                        .color(TEXT_MUTED),
                    )
                    .selectable(false),
                );
                ui.add_space(12.0);

                // Без треков (все файлы пропали) кнопкам нечего делать
                ui.add_enabled_ui(!tracks.is_empty(), |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        play_all =
                            widgets::pill_button(ui, t().play_all, ButtonStyle::Primary).clicked();
                        add_to_playlist =
                            widgets::pill_button(ui, t().add_to_playlist, ButtonStyle::Secondary)
                                .clicked();
                    });
                });
            });

        if play_all {
            // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
            self.playlist_view.clear_selection();
            let result = self.controller.play_all(tracks);
            self.set_result(result);
        } else if add_to_playlist {
            self.add_to_playlist(tracks);
        }
    }

    /// Альбомы исполнителя `artist` (имя без учёта регистра, как при группировке),
    /// в порядке сетки
    fn artist_albums(&self, artist: &str) -> impl Iterator<Item = &AlbumGroup> {
        let artist = artist.to_lowercase();
        self.groups
            .albums
            .iter()
            .filter(move |album| album.artist.to_lowercase() == artist)
    }

    /// Все треки исполнителя для воспроизведения: альбом за альбомом в порядке сетки,
    /// внутри альбома — в порядке библиотеки. Треки, чьих файлов нет на диске, пропускаются:
    /// сыграть их всё равно не выйдет
    fn artist_tracks(&self, artist: &str) -> Vec<Track> {
        self.playable_tracks(&self.artist_track_indices(artist))
    }

    /// Номера всех треков исполнителя в библиотеке: альбом за альбомом в порядке сетки
    fn artist_track_indices(&self, artist: &str) -> Vec<usize> {
        self.artist_albums(artist)
            .flat_map(|album| album.tracks.iter().copied())
            .collect()
    }

    /// Страница альбома: обложка, название, исполнитель, год, кнопки «Play all» и
    /// «Add to playlist» и ниже — список его треков
    fn render_album_page(&mut self, ui: &mut Ui, key: &AlbumKey) {
        self.refresh_groups();
        let Some(album) = self.groups.albums.iter().find(|album| key.matches(album)) else {
            // Альбома больше нет — его треки убрали из библиотеки
            self.selected_album = None;
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
                    widgets::paint_cover(ui, cover_rect, cover, TILE_RADIUS);
                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.add(
                            Label::new(RichText::new(&title).size(ARTIST_NAME_SIZE).color(TEXT))
                                .selectable(false)
                                .truncate(),
                        );
                        // Имя исполнителя — ссылка на его профиль
                        open_artist = widgets::text_link(ui, &artist, 16.0, TEXT_MUTED).clicked();
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
                            play_all = widgets::pill_button(ui, t().play_all, ButtonStyle::Primary)
                                .clicked();
                            add_all = widgets::pill_button(
                                ui,
                                t().add_to_playlist,
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
            .filter_map(|&index| self.library.tracks().get(index).cloned())
            .collect();
        let current_path = self
            .controller
            .player()
            .current_track
            .as_ref()
            .map(|track| track.path.clone());
        let library = &self.library;
        let mut add_selected = false;
        let mut remove_selected = false;
        let response = self.album_tracks_view.show(
            ui,
            &tracks,
            |index| widgets::RowState {
                // Подсвечен трек, который сейчас играет
                current: current_path.as_ref() == Some(&tracks[index].path),
                missing: library.is_missing(indices[index]),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                let (add_label, remove_label) = track_menu_labels(selected);
                if widgets::menu_item(ui, &add_label).clicked() {
                    add_selected = true;
                }
                if widgets::menu_item(ui, &remove_label).clicked() {
                    remove_selected = true;
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
            self.set_view(CentralView::Artists);
            self.selected_artist = Some(artist);
            self.selected_album = None;
        } else if play_all {
            self.playlist_view.clear_selection();
            let result = self.controller.play_all(self.playable_tracks(&indices));
            self.set_result(result);
        } else if add_all {
            let tracks = self.playable_tracks(&indices);
            self.add_to_playlist(tracks);
        } else if add_selected {
            let tracks = self.playable_tracks(&selected);
            self.add_to_playlist(tracks);
        } else if remove_selected || response.delete {
            self.request_songs_removal(&selected);
        } else if let Some(index) = response.activated {
            // Двойной клик играет только этот трек: он встаёт в плейлист, а не заменяет его
            // альбомом — для альбома целиком есть кнопки «Play all» и «Add to playlist»
            self.play_from_library(indices[index]);
        }
    }

    /// Все альбомы библиотеки
    fn render_albums(&mut self, ui: &mut Ui) {
        self.render_album_grid(ui, "albums", None);
    }

    /// Альбомы сеткой: обложка, название, исполнитель и год. С `artist` — только альбомы
    /// этого исполнителя (имя без учёта регистра, как при группировке)
    fn render_album_grid(&mut self, ui: &mut Ui, id_salt: &str, artist: Option<&str>) {
        self.refresh_groups();
        let artist = artist.map(str::to_lowercase);
        // Поиск по названию — только во всех альбомах, в профиле исполнителя поля нет
        let query = match artist {
            Some(_) => String::new(),
            None => normalized_query(&self.search.query),
        };
        let albums: Vec<&AlbumGroup> = self
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
        let covers = &mut self.covers;
        if albums.is_empty() {
            let text = if artist.is_some() {
                t().no_albums
            } else if self.groups.albums.is_empty() {
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
            self.library.revision()
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
                widgets::paint_cover(ui, image_rect, cover, TILE_RADIUS);

                // Кнопка play в левом верхнем углу обложки — проявляется при наведении на плитку
                // и играет только этот альбом, независимо от выделения
                if tile.hover > 0.0 {
                    let play_rect = Rect::from_min_size(
                        image_rect.min + vec2(OVERLAY_PLAY_INSET, OVERLAY_PLAY_INSET),
                        vec2(OVERLAY_PLAY_SIZE, OVERLAY_PLAY_SIZE),
                    );
                    let play = widgets::icon_button(
                        ui,
                        play_rect,
                        widgets::HitShape::Circle,
                        &format!("play_album {index}"),
                        true,
                    );
                    paint_overlay_play(ui, play_rect, tile.hover, play.hover);
                    if play.clicked {
                        action = Some((GroupAction::Play, Target::One(index)));
                    }
                }

                if let Some(menu_action) = group_context_menu(tile, GroupKind::Album) {
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
            let album = AlbumKey {
                title: albums[index].title.clone(),
                artist: albums[index].artist.clone(),
            };
            self.selected_album = Some(album);
            // Выделение относилось к трекам прошлого альбома
            self.album_tracks_view.clear_selection();
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
            self.apply_group_action(action, GroupKind::Album, titles, &tracks);
        }
    }

    /// Действие с альбомами или исполнителями `names`: `tracks` — номера их треков
    /// в библиотеке, по порядку
    fn apply_group_action(
        &mut self,
        action: GroupAction,
        kind: GroupKind,
        names: Vec<String>,
        tracks: &[usize],
    ) {
        match action {
            GroupAction::Play => {
                // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
                self.playlist_view.clear_selection();
                let result = self.controller.play_all(self.playable_tracks(tracks));
                self.set_result(result);
            }
            GroupAction::AddToPlaylist => {
                let tracks = self.playable_tracks(tracks);
                self.add_to_playlist(tracks);
            }
            // Сразу не удаляем — сначала спрашиваем (см. render_removal_dialog).
            // Запоминаем пути, а не номера: пока диалог открыт, библиотека может измениться
            GroupAction::Remove => {
                let paths = tracks
                    .iter()
                    .filter_map(|&index| self.library.tracks().get(index))
                    .map(|track| track.path.clone())
                    .collect();
                self.pending_removal = Some(LibraryRemoval { kind, names, paths });
            }
        }
    }

    /// Треки библиотеки с номерами `indices` для воспроизведения. Треки, чьих файлов нет
    /// на диске, пропускаются: сыграть их всё равно не выйдет
    fn playable_tracks(&self, indices: &[usize]) -> Vec<Track> {
        indices
            .iter()
            .filter(|&&index| !self.library.is_missing(index))
            .filter_map(|&index| self.library.tracks().get(index).cloned())
            .collect()
    }

    /// Все треки библиотеки списком — или найденные поиском. Двойной клик добавляет трек
    /// в плейлист и запускает его; из меню — добавить в плейлист или убрать из библиотеки
    fn render_songs(&mut self, ui: &mut Ui) {
        if self.library.tracks().is_empty() {
            placeholder(ui, t().library_empty);
            return;
        }
        self.refresh_found_songs();
        if self
            .search
            .found_songs
            .as_ref()
            .is_some_and(|found| found.indices.is_empty())
        {
            placeholder(ui, t().no_results);
            return;
        }

        // «Play all» — показанные треки в порядке списка заменяют плейлист и играют
        let mut play_all = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 16,
            })
            .show(ui, |ui| {
                play_all = widgets::pill_button(ui, t().play_all, ButtonStyle::Primary).clicked();
            });
        if play_all {
            let shown = self.shown_song_indices();
            self.playlist_view.clear_selection();
            let result = self.controller.play_all(self.playable_tracks(&shown));
            self.set_result(result);
        }

        // Пока библиотека заимствована списком, менять ничего нельзя — запоминаем, что сделать.
        // Номера строк списка — номера в `found` (во время поиска) или сразу в библиотеке
        let mut add_to_playlist = false;
        let mut remove = false;
        let library = &self.library;
        let found = self.search.found_songs.as_ref();
        let tracks = found.map_or(library.tracks(), |found| &found.tracks);
        let response = self.library_view.show(
            ui,
            tracks,
            |index| widgets::RowState {
                current: false,
                missing: library.is_missing(found.map_or(index, |found| found.indices[index])),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                let (add_label, remove_label) = track_menu_labels(selected);
                if widgets::menu_item(ui, &add_label).clicked() {
                    add_to_playlist = true;
                }
                if widgets::menu_item(ui, &remove_label).clicked() {
                    remove = true;
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Library;
        }
        let selected: Vec<usize> = self
            .library_view
            .selection()
            .iter()
            .filter_map(|&index| self.song_index(index))
            .collect();
        if add_to_playlist {
            let tracks = selected
                .iter()
                .filter_map(|&index| self.library.tracks().get(index).cloned())
                .collect();
            self.add_to_playlist(tracks);
        }
        if remove || response.delete {
            self.request_songs_removal(&selected);
        }
        if let Some(index) = response.activated.and_then(|index| self.song_index(index)) {
            self.play_from_library(index);
        }
    }

    /// Номер в библиотеке трека из строки `row` списка Songs
    fn song_index(&self, row: usize) -> Option<usize> {
        match &self.search.found_songs {
            Some(found) => found.indices.get(row).copied(),
            None => (row < self.library.tracks().len()).then_some(row),
        }
    }

    /// Номера в библиотеке всех треков, показанных в Songs, по порядку
    fn shown_song_indices(&self) -> Vec<usize> {
        match &self.search.found_songs {
            Some(found) => found.indices.clone(),
            None => (0..self.library.tracks().len()).collect(),
        }
    }

    /// Двойной клик в библиотеке: трек сразу играет. Если он уже есть в плейлисте, играет
    /// оттуда, иначе встаёт в конец — повторный запуск не плодит копии в плейлисте
    fn play_from_library(&mut self, index: usize) {
        let Some(track) = self.library.tracks().get(index).cloned() else {
            return;
        };
        let playlist = &self.controller.player().track_list.tracks;
        let playlist_index = match playlist.iter().position(|t| t.path == track.path) {
            Some(existing) => existing,
            None => {
                self.controller.append_tracks([track]);
                self.controller.player().track_list.tracks.len() - 1
            }
        };
        let result = self
            .controller
            .select(playlist_index)
            .and_then(|()| self.controller.play());
        self.set_result(result);
    }
}

/// Запрос поиска для сравнения: без пробелов по краям, в нижнем регистре
fn normalized_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Есть ли `query` (уже из `normalized_query`) в `text` без учёта регистра.
/// Пустой запрос подходит ко всему
fn contains_query(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(query)
}

/// Текст по центру оставшегося места
fn placeholder(ui: &Ui, text: &str) {
    ui.painter().text(
        ui.available_rect_before_wrap().center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        TEXT_FAINT,
    );
}

/// Заглушка картинки исполнителя: круг с первой буквой имени. При наведении круг светлеет
fn paint_artist_placeholder(ui: &Ui, rect: Rect, name: &str, hover: f32) {
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

/// Альбомы, исполнители или треки, удаление которых из библиотеки ждёт подтверждения
pub(super) struct LibraryRemoval {
    kind: GroupKind,
    /// Названия альбомов, имена исполнителей или название трека — для текста диалога
    names: Vec<String>,
    /// Файлы их треков
    paths: HashSet<PathBuf>,
}

impl PlayerApp {
    /// Просит подтвердить удаление треков библиотеки с номерами `indices` (сами файлы
    /// не трогаются). Удаляются они после ответа в диалоге — см. `render_removal_dialog`
    fn request_songs_removal(&mut self, indices: &[usize]) {
        let tracks: Vec<&Track> = indices
            .iter()
            .filter_map(|&index| self.library.tracks().get(index))
            .collect();
        let names = tracks.iter().map(|track| track.title.clone()).collect();
        let paths: HashSet<PathBuf> = tracks.iter().map(|track| track.path.clone()).collect();
        if !paths.is_empty() {
            self.pending_removal = Some(LibraryRemoval {
                kind: GroupKind::Songs,
                names,
                paths,
            });
        }
    }

    /// Диалог «точно удалить из библиотеки?» — пока ждём ответа
    pub(super) fn render_removal_dialog(&mut self, ctx: &egui::Context) {
        let Some(removal) = &self.pending_removal else {
            return;
        };
        let t = t();
        let count = |word| t.count(removal.names.len(), word);
        let tracks = t.count(removal.paths.len(), &t.track_count);
        let (title, message) = match (removal.kind, removal.names.as_slice()) {
            (GroupKind::Album, [album]) => (
                t.remove_album_title.to_owned(),
                fill(t.remove_album_message, &[("album", album)]),
            ),
            (GroupKind::Album, _) => {
                let albums = count(&t.album_count);
                (
                    fill(t.remove_albums_title, &[("albums", &albums)]),
                    fill(
                        t.remove_albums_message,
                        &[("albums", &albums), ("tracks", &tracks)],
                    ),
                )
            }
            (GroupKind::Artist, [artist]) => (
                t.remove_artist_title.to_owned(),
                fill(t.remove_artist_message, &[("artist", artist)]),
            ),
            (GroupKind::Artist, _) => {
                let artists = count(&t.artist_count);
                (
                    fill(t.remove_artists_title, &[("artists", &artists)]),
                    fill(
                        t.remove_artists_message,
                        &[("artists", &artists), ("tracks", &tracks)],
                    ),
                )
            }
            (GroupKind::Songs, [song]) => (
                t.remove_song_title.to_owned(),
                fill(t.remove_song_message, &[("song", song)]),
            ),
            (GroupKind::Songs, _) => {
                let songs = count(&t.song_count);
                (
                    fill(t.remove_songs_title, &[("songs", &songs)]),
                    fill(t.remove_songs_message, &[("songs", &songs)]),
                )
            }
        };
        let answer =
            widgets::confirm_dialog(ctx, "remove_from_library", &title, &message, t.remove);
        match answer {
            widgets::Confirm::Pending => {}
            widgets::Confirm::Cancelled => self.pending_removal = None,
            widgets::Confirm::Confirmed => {
                let Some(removal) = self.pending_removal.take() else {
                    return;
                };
                let indices = self
                    .library
                    .tracks()
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| removal.paths.contains(&track.path))
                    .map(|(index, _)| index)
                    .collect();
                let result = self.library.remove(&indices);
                self.set_result(result);
                // Номера треков библиотеки сдвинулись — выделение в списках ни на что не указывает
                self.library_view.clear_selection();
                self.album_tracks_view.clear_selection();
            }
        }
    }
}

/// Контекстное меню альбома или исполнителя. Действует на все выделенные плитки —
/// в подписях их число. Возвращает выбранное действие
fn group_context_menu(tile: &widgets::Tile, kind: GroupKind) -> Option<GroupAction> {
    let mut action = None;
    tile.response.context_menu(|ui| {
        widgets::menu_style(ui);
        let t = t();
        let items = match (tile.selected, kind) {
            (1, _) => [
                (t.play.to_owned(), GroupAction::Play),
                (t.add_to_playlist.to_owned(), GroupAction::AddToPlaylist),
                (t.remove_from_library.to_owned(), GroupAction::Remove),
            ],
            (count, kind) => {
                let word = match kind {
                    GroupKind::Album => &t.album_count,
                    GroupKind::Artist => &t.artist_count,
                    GroupKind::Songs => &t.track_count,
                };
                let items = t.count(count, word);
                let items = [("items", items.as_str())];
                [
                    (fill(t.play_items, &items), GroupAction::Play),
                    (
                        fill(t.add_items_to_playlist, &items),
                        GroupAction::AddToPlaylist,
                    ),
                    (
                        fill(t.remove_items_from_library, &items),
                        GroupAction::Remove,
                    ),
                ]
            }
        };
        for (label, menu_action) in items {
            if widgets::menu_item(ui, &label).clicked() {
                action = Some(menu_action);
            }
        }
    });
    action
}

/// Подписи меню списка треков медиатеки: добавить в плейлист и удалить из медиатеки.
/// Для нескольких выделенных — с их числом
fn track_menu_labels(selected: usize) -> (String, String) {
    let t = t();
    match selected {
        0 | 1 => (
            t.add_to_playlist.to_owned(),
            t.remove_from_library.to_owned(),
        ),
        count => {
            let items = t.count(count, &t.track_count);
            let items = [("items", items.as_str())];
            (
                fill(t.add_items_to_playlist, &items),
                fill(t.remove_items_from_library, &items),
            )
        }
    }
}
