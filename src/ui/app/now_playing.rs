//! Центральная область: информация о текущем треке и обложка, без треков — «Open files»;
//! плашка текущего трека, пока открыт вид библиотеки.

use eframe::egui::{self, CursorIcon, Rect, RichText, Sense, Ui, vec2};

use super::{CentralView, PlayerApp, library_views::AlbumKey, loading::Destination};
use crate::library::groups;
use crate::ui::{
    covers::CoverSize,
    format::format_secs,
    theme::{TEXT, TEXT_MUTED},
    widgets,
};

const COVER_MAX_SIZE: f32 = 320.0;
/// Размер текста деталей трека
const DETAILS_SIZE: f32 = 16.0;
const COVER_RADIUS: u8 = 12;

impl PlayerApp {
    /// Кнопка по центру всего оставшегося места
    pub(super) fn render_add_tracks_block(&mut self, ui: &mut Ui) {
        let text = RichText::new("Open files").size(18.0).color(TEXT);

        // Layout в egui не центрирует по вертикали, поэтому считаем размер кнопки
        // заранее и ставим её в центр свободной области сами
        let text_size = egui::WidgetText::from(text.clone())
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            )
            .size();
        let button_size = text_size + 2.0 * ui.spacing().button_padding;
        let button_rect =
            Rect::from_center_size(ui.available_rect_before_wrap().center(), button_size);

        let open = egui::Button::new(text).frame(false);
        if ui
            .put(button_rect, open)
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Open files")
            .clicked()
        {
            self.add_files(Destination::Playlist);
        }
    }

    /// Детали текущего трека: название, исполнитель и альбом (ссылки — если они есть
    /// в библиотеке), длительность; ниже — обложка по центру свободного места
    pub(super) fn render_current_track_info(&mut self, ui: &mut Ui) {
        let Some(track) = self.controller.player().current_track.clone() else {
            return;
        };
        // Исполнитель и альбом трека в библиотеке — ищутся так же, как при группировке:
        // без учёта регистра, пустое имя — «Unknown …»
        self.refresh_groups();
        let normalized = |text: &str, unknown: &str| {
            let text = text.trim();
            if text.is_empty() { unknown } else { text }.to_lowercase()
        };
        let artist_key = normalized(&track.artist, groups::UNKNOWN_ARTIST);
        let album_key = normalized(&track.album, groups::UNKNOWN_ALBUM);
        let artist_in_library = self
            .groups
            .artists
            .iter()
            .find(|artist| artist.name.to_lowercase() == artist_key)
            .map(|artist| artist.name.clone());
        let album_in_library = self
            .groups
            .albums
            .iter()
            .find(|album| {
                album.title.to_lowercase() == album_key && album.artist.to_lowercase() == artist_key
            })
            .map(AlbumKey::of);

        let mut open_artist = false;
        let mut open_album = false;
        ui.vertical(|ui| {
            ui.label(RichText::new(&track.title).size(DETAILS_SIZE).color(TEXT));
            if !track.artist.is_empty() {
                if artist_in_library.is_some() {
                    open_artist = widgets::text_link(ui, &track.artist, DETAILS_SIZE, TEXT_MUTED)
                        .on_hover_text("Show artist")
                        .clicked();
                } else {
                    ui.label(
                        RichText::new(&track.artist)
                            .size(DETAILS_SIZE)
                            .color(TEXT_MUTED),
                    );
                }
            }
            if !track.album.is_empty() {
                if album_in_library.is_some() {
                    open_album = widgets::text_link(ui, &track.album, DETAILS_SIZE, TEXT_MUTED)
                        .on_hover_text("Show album")
                        .clicked();
                } else {
                    ui.label(
                        RichText::new(&track.album)
                            .size(DETAILS_SIZE)
                            .color(TEXT_MUTED),
                    );
                }
            }
            ui.label(
                RichText::new(format_secs(track.duration_secs as f32))
                    .size(DETAILS_SIZE)
                    .color(TEXT_MUTED),
            );
            ui.add_space(8.0);

            // Обложка — по центру всего оставшегося места, по ширине и по высоте; на небольшом
            // окне уменьшается, чтобы помещаться
            let free = ui.available_rect_before_wrap();
            let side = free.width().min(free.height()).clamp(0.0, COVER_MAX_SIZE);
            ui.allocate_rect(free, Sense::hover());
            let cover = self.covers.get(&track.path, CoverSize::Large);
            widgets::paint_cover(
                ui,
                Rect::from_center_size(free.center(), vec2(side, side)),
                cover,
                COVER_RADIUS,
            );
        });

        // Переход — как из меню библиотеки: вид целиком, без прошлых вложенных страниц
        if open_artist && let Some(artist) = artist_in_library {
            self.view = CentralView::Artists;
            self.selected_artist = Some(artist);
            self.selected_album = None;
        } else if open_album && let Some(album) = album_in_library {
            self.view = CentralView::Albums;
            self.selected_artist = None;
            self.selected_album = Some(album);
            self.album_tracks_view.clear_selection();
        }
    }

    /// Плашка текущего трека внизу центральной области. Возвращает `true`, если по ней кликнули
    pub(super) fn render_now_playing_bar(&mut self, ui: &mut Ui) -> bool {
        let Some(track) = &self.controller.player().current_track else {
            return false;
        };
        widgets::now_playing_bar(ui, &track.title, &track.artist)
            .on_hover_text("Show the current track")
            .clicked()
    }
}
