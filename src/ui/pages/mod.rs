//! Страницы центральной области: что открыто — решает `Route` (см. `navigation`).
//! Каждая страница — отдельный файл с `impl PlayerApp`; здесь — выбор страницы, заголовок
//! с «хлебными крошками» и поиском и общее для страниц.

mod album;
mod albums;
mod artists;
mod now_playing;
mod search;
mod shared;
mod songs;

pub(crate) use search::LibrarySearch;

use eframe::egui::{self, Align, Align2, FontId, Ui};

use crate::ui::{
    action::Action,
    app::PlayerApp,
    components::{self},
    i18n::{fill, t},
    navigation::{Route, Section},
    style::theme::{TEXT, TEXT_FAINT},
};

/// Скругление картинок в сетке
pub(crate) const TILE_RADIUS: u8 = 8;

/// Имя исполнителя в шапке его профиля и название альбома на странице альбома
pub(crate) const ARTIST_NAME_SIZE: f32 = 30.0;

/// Текст по центру оставшегося места
pub(crate) fn placeholder(ui: &Ui, text: &str) {
    ui.painter().text(
        ui.available_rect_before_wrap().center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        TEXT_FAINT,
    );
}

impl PlayerApp {
    /// Открытая страница медиатеки (`self.route`) во всю центральную область
    pub(crate) fn render_library_view(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .inner_margin(16.0)
            .show(ui, |ui| self.render_breadcrumbs(ui));
        match self.route.clone() {
            Route::Album(album) | Route::ArtistAlbum { album, .. } => {
                self.render_album_page(ui, &album)
            }
            Route::Artist(artist) => self.render_artist_profile(ui, &artist),
            Route::Artists => self.render_artists(ui),
            Route::Albums => self.render_albums(ui),
            Route::Songs => self.render_songs(ui),
            Route::NowPlaying => {}
        }
    }

    /// Заголовок страницы. Внутри исполнителя или альбома — «хлебные крошки», каждая ведёт
    /// на свой уровень: «Albums › Альбом», «Artists › Исполнитель › Альбом»
    pub(crate) fn render_breadcrumbs(&mut self, ui: &mut Ui) {
        let section = self.route.section();
        let title = section.title();
        let target = match self.route.clone() {
            Route::Album(album) => {
                components::breadcrumb_title(ui, &[title], &album.title).map(|_| Route::Albums)
            }
            Route::ArtistAlbum { artist, album } => {
                match components::breadcrumb_title(ui, &[title, &artist], &album.title) {
                    Some(0) => Some(Route::Artists),
                    Some(_) => Some(Route::Artist(artist)),
                    None => None,
                }
            }
            Route::Artist(artist) => {
                components::breadcrumb_title(ui, &[title], &artist).map(|_| Route::Artists)
            }
            // Верхний уровень раздела — с полем поиска справа. В пустой медиатеке искать нечего
            Route::Artists | Route::Albums | Route::Songs
                if !self.core.library.tracks().is_empty() =>
            {
                self.render_title_with_search(ui, section);
                None
            }
            _ => {
                components::block_title(ui, title, Align::LEFT, TEXT);
                None
            }
        };
        if let Some(route) = target {
            self.dispatch(Action::Navigate(route));
        }
    }

    /// Заголовок вида слева и поле поиска справа. У Songs рядом с заголовком — число треков,
    /// как «Playlist · N tracks»; во время поиска — сколько найдено: «· 3 of 120 tracks»
    pub(crate) fn render_title_with_search(&mut self, ui: &mut Ui, section: Section) {
        let note = (section == Section::Songs).then(|| {
            self.refresh_found_songs();
            let total = self.core.library.tracks().len();
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
        let changed = components::title_with_search(ui, section.title(), query, |ui| match &note {
            Some(note) => {
                components::block_title_with_note(ui, section.title(), note);
            }
            None => {
                components::block_title(ui, section.title(), Align::LEFT, TEXT);
            }
        });
        // Выделение сеток сбрасывается само — запрос входит в их scope. Список Songs
        // показывает другие треки — его выделение ни на что не указывает
        if changed && section == Section::Songs {
            self.library_view.clear_selection();
        }
    }
}
