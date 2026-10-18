//! Виды библиотеки в центральной области: исполнители, альбомы, композиции.

use eframe::egui::{
    self, Align, Align2, FontId, Label, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2,
};

use std::{collections::HashSet, path::PathBuf};

use super::{CentralView, LibraryGroups, ListFocus, PlayerApp};
use crate::{
    library::{
        Track,
        groups::{self, AlbumGroup},
    },
    ui::{
        covers::CoverSize,
        format::plural,
        icons::paint_overlay_play,
        theme::{
            HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, SELECTION_STROKE_WIDTH, TEXT, TEXT_FAINT,
            TEXT_MUTED,
        },
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
    /// Сравнение без учёта регистра — как при группировке
    fn matches(&self, album: &AlbumGroup) -> bool {
        album.title.to_lowercase() == self.title.to_lowercase()
            && album.artist.to_lowercase() == self.artist.to_lowercase()
    }
}
/// Кнопка play поверх обложки альбома: размер и отступ от угла обложки
const OVERLAY_PLAY_SIZE: f32 = 32.0;
const OVERLAY_PLAY_INSET: f32 = 8.0;
/// Отступ рамки вокруг плитки с открытым меню — чтобы рамка не ложилась на край обложки
const TILE_HIGHLIGHT_PADDING: f32 = 6.0;

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

/// Пункты контекстного меню альбома и исполнителя
const GROUP_MENU: [(&str, GroupAction); 3] = [
    ("Play", GroupAction::Play),
    ("Add to playlist", GroupAction::AddToPlaylist),
    ("Remove from library", GroupAction::Remove),
];

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
            _ => {
                widgets::block_title(ui, view.title(), Align::LEFT, TEXT);
            }
        }
    }

    /// Пересчитывает исполнителей и альбомы, если библиотека изменилась
    fn refresh_groups(&mut self) {
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
            placeholder(ui, "Library is empty");
            return;
        }
        // Имя — по центру под круглой картинкой. Сетка заимствует исполнителей —
        // действие из меню запоминаем и выполняем после неё
        let mut action = None;
        let clicked = widgets::tile_grid(
            ui,
            "artists",
            artists.len(),
            1,
            Align::Center,
            true,
            |ui, index, image_rect, tile| {
                let artist = &artists[index];
                paint_artist_placeholder(ui, image_rect, &artist.name, tile.hover);
                paint_menu_highlight(ui, tile);
                if let Some(menu_action) = group_context_menu(tile) {
                    action = Some((menu_action, index));
                }
                vec![TileLine {
                    text: artist.name.clone(),
                    primary: true,
                }]
            },
        );
        if let Some(index) = clicked {
            self.selected_artist = Some(self.groups.artists[index].name.clone());
        }
        if let Some((action, index)) = action {
            let name = self.groups.artists[index].name.clone();
            let tracks = self.artist_track_indices(&name);
            self.apply_group_action(action, GroupKind::Artist, name, &tracks);
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
                            plural(album_count, "album", "albums"),
                            plural(tracks.len(), "song", "songs"),
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
                            widgets::pill_button(ui, "Play all", ButtonStyle::Primary).clicked();
                        add_to_playlist =
                            widgets::pill_button(ui, "Add to playlist", ButtonStyle::Secondary)
                                .clicked();
                    });
                });
            });

        if play_all {
            // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
            self.playlist_view.clear_selection();
            let result = self.controller.play_all(tracks, None);
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
        let (title, artist, year, cover_from) = (
            album.title.clone(),
            album.artist.clone(),
            album.year,
            album.cover_from.clone(),
        );

        // ---- шапка
        let mut play_all = false;
        let mut add_all = false;
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
                    let cover = self.covers.get(&cover_from, CoverSize::Large);
                    widgets::paint_cover(ui, cover_rect, cover, TILE_RADIUS);
                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.add(
                            Label::new(RichText::new(&title).size(ARTIST_NAME_SIZE).color(TEXT))
                                .selectable(false)
                                .truncate(),
                        );
                        ui.add(
                            Label::new(RichText::new(&artist).size(16.0).color(TEXT_MUTED))
                                .selectable(false)
                                .truncate(),
                        );
                        let tracks = plural(indices.len(), "track", "tracks");
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
                            play_all = widgets::pill_button(ui, "Play all", ButtonStyle::Primary)
                                .clicked();
                            add_all =
                                widgets::pill_button(ui, "Add to playlist", ButtonStyle::Secondary)
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
                let (add_label, remove_label) = match selected {
                    0 | 1 => (
                        "Add to playlist".to_owned(),
                        "Remove from library".to_owned(),
                    ),
                    count => (
                        format!("Add {count} tracks to playlist"),
                        format!("Remove {count} tracks from library"),
                    ),
                };
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

        if play_all {
            self.playlist_view.clear_selection();
            let result = self
                .controller
                .play_all(self.playable_tracks(&indices), None);
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
            self.play_album_from(&indices, index);
        }
    }

    /// Двойной клик по треку альбома: альбом заменяет плейлист и играет с этого трека
    fn play_album_from(&mut self, indices: &[usize], position: usize) {
        let Some(&clicked) = indices.get(position) else {
            return;
        };
        if self.library.is_missing(clicked) {
            self.error = Some("File not found: it was moved or deleted".into());
            return;
        }
        // Номер трека среди тех, что можно сыграть: пропавшие файлы в плейлист не попадают
        let start = indices[..position]
            .iter()
            .filter(|&&index| !self.library.is_missing(index))
            .count();
        self.playlist_view.clear_selection();
        let result = self
            .controller
            .play_all(self.playable_tracks(indices), Some(start));
        self.set_result(result);
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
        let albums: Vec<&AlbumGroup> = self
            .groups
            .albums
            .iter()
            .filter(|album| {
                artist
                    .as_ref()
                    .is_none_or(|artist| album.artist.to_lowercase() == *artist)
            })
            .collect();
        let covers = &mut self.covers;
        if albums.is_empty() {
            let text = if artist.is_some() {
                "No albums"
            } else {
                "Library is empty"
            };
            placeholder(ui, text);
            return;
        }
        // Сетка заимствует альбомы и кэш обложек — действие запоминаем и выполняем после неё.
        // Клик по плитке открывает альбом; воспроизведение — кнопкой на обложке и из меню
        let mut action = None;
        let clicked = widgets::tile_grid(
            ui,
            id_salt,
            albums.len(),
            3,
            Align::Min,
            true,
            |ui, index, image_rect, tile| {
                let album = albums[index];
                let cover = covers.get(&album.cover_from, CoverSize::Thumbnail);
                widgets::paint_cover(ui, image_rect, cover, TILE_RADIUS);

                // Кнопка play в левом верхнем углу обложки — проявляется при наведении на плитку
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
                        action = Some((GroupAction::Play, index));
                    }
                }

                paint_menu_highlight(ui, tile);
                if let Some(menu_action) = group_context_menu(tile) {
                    action = Some((menu_action, index));
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

        if let Some(index) = clicked {
            let album = AlbumKey {
                title: albums[index].title.clone(),
                artist: albums[index].artist.clone(),
            };
            self.selected_album = Some(album);
            // Выделение относилось к трекам прошлого альбома
            self.album_tracks_view.clear_selection();
        } else if let Some((action, index)) = action {
            let (title, tracks) = (albums[index].title.clone(), albums[index].tracks.clone());
            self.apply_group_action(action, GroupKind::Album, title, &tracks);
        }
    }

    /// Действие с альбомом или исполнителем `name`: `tracks` — номера его треков в библиотеке
    fn apply_group_action(
        &mut self,
        action: GroupAction,
        kind: GroupKind,
        name: String,
        tracks: &[usize],
    ) {
        match action {
            GroupAction::Play => {
                // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
                self.playlist_view.clear_selection();
                let result = self.controller.play_all(self.playable_tracks(tracks), None);
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
                self.pending_removal = Some(LibraryRemoval { kind, name, paths });
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

    /// Все треки библиотеки списком. Двойной клик добавляет трек в плейлист и запускает его;
    /// из меню — добавить в плейлист или убрать из библиотеки
    fn render_songs(&mut self, ui: &mut Ui) {
        if self.library.tracks().is_empty() {
            placeholder(ui, "Library is empty");
            return;
        }

        // Пока библиотека заимствована списком, менять ничего нельзя — запоминаем, что сделать
        let mut add_to_playlist = false;
        let mut remove = false;
        let library = &self.library;
        let response = self.library_view.show(
            ui,
            library.tracks(),
            |index| widgets::RowState {
                current: false,
                missing: library.is_missing(index),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                let (add_label, remove_label) = match selected {
                    0 | 1 => (
                        "Add to playlist".to_owned(),
                        "Remove from library".to_owned(),
                    ),
                    count => (
                        format!("Add {count} tracks to playlist"),
                        format!("Remove {count} tracks from library"),
                    ),
                };
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
        if add_to_playlist {
            let tracks = self.selected_library_tracks();
            self.add_to_playlist(tracks);
        }
        if remove || response.delete {
            let selected: Vec<usize> = self.library_view.selection().iter().copied().collect();
            self.request_songs_removal(&selected);
        }
        if let Some(index) = response.activated {
            self.play_from_library(index);
        }
    }

    fn selected_library_tracks(&self) -> Vec<Track> {
        let tracks = self.library.tracks();
        self.library_view
            .selection()
            .iter()
            .filter_map(|&index| tracks.get(index).cloned())
            .collect()
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

/// Альбом или исполнитель, удаление которых из библиотеки ждёт подтверждения
pub(super) struct LibraryRemoval {
    kind: GroupKind,
    /// Название альбома или имя исполнителя
    name: String,
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
        let name = match tracks.as_slice() {
            [track] => track.title.clone(),
            _ => String::new(),
        };
        let paths: HashSet<PathBuf> = tracks.iter().map(|track| track.path.clone()).collect();
        if !paths.is_empty() {
            self.pending_removal = Some(LibraryRemoval {
                kind: GroupKind::Songs,
                name,
                paths,
            });
        }
    }

    /// Диалог «точно удалить из библиотеки?» — пока ждём ответа
    pub(super) fn render_removal_dialog(&mut self, ctx: &egui::Context) {
        let Some(removal) = &self.pending_removal else {
            return;
        };
        let (title, message) = match removal.kind {
            GroupKind::Album => (
                "Remove album from library?".to_owned(),
                format!(
                    "\u{201C}{}\u{201D} will be removed from your library. \
                     The files will stay on your disk.",
                    removal.name
                ),
            ),
            GroupKind::Artist => (
                "Remove artist from library?".to_owned(),
                format!(
                    "All songs by \u{201C}{}\u{201D} will be removed from your library. \
                     The files will stay on your disk.",
                    removal.name
                ),
            ),
            GroupKind::Songs if removal.paths.len() == 1 => (
                "Remove song from library?".to_owned(),
                format!(
                    "\u{201C}{}\u{201D} will be removed from your library. \
                     The file will stay on your disk.",
                    removal.name
                ),
            ),
            GroupKind::Songs => (
                format!("Remove {} songs from library?", removal.paths.len()),
                format!(
                    "{} songs will be removed from your library. \
                     The files will stay on your disk.",
                    removal.paths.len()
                ),
            ),
        };
        let answer =
            widgets::confirm_dialog(ctx, "remove_from_library", &title, &message, "Remove");
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

/// Пока у плитки открыто контекстное меню, она обведена рамкой, как выделенный трек
/// в списке, — видно, к чему относится меню
fn paint_menu_highlight(ui: &Ui, tile: &widgets::Tile) {
    let menu_open = ui.ctx().animate_bool_with_time_and_easing(
        tile.response.id.with("menu_open"),
        tile.response.context_menu_opened(),
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    if menu_open > 0.0 {
        ui.painter().rect_stroke(
            tile.response.rect.expand(TILE_HIGHLIGHT_PADDING),
            TILE_RADIUS + TILE_HIGHLIGHT_PADDING as u8,
            Stroke::new(
                SELECTION_STROKE_WIDTH,
                HIGHLIGHT_STRONG.gamma_multiply(menu_open),
            ),
            StrokeKind::Inside,
        );
    }
}

/// Контекстное меню альбома или исполнителя. Возвращает выбранное действие
fn group_context_menu(tile: &widgets::Tile) -> Option<GroupAction> {
    let mut action = None;
    tile.response.context_menu(|ui| {
        widgets::menu_style(ui);
        for (label, menu_action) in GROUP_MENU {
            if widgets::menu_item(ui, label).clicked() {
                action = Some(menu_action);
            }
        }
    });
    action
}
