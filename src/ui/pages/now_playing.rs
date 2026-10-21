//! Центральная область: информация о текущем треке и обложка, без треков — «Open files»;
//! плашка текущего трека, пока открыт вид библиотеки.

use eframe::egui::{self, CursorIcon, Rect, RichText, Sense, Shape, Ui, pos2, vec2};

use crate::{
    core::library::{
        Track,
        groups::{self, AlbumGroup},
    },
    ui::action::Action,
    ui::{
        app::PlayerApp,
        components,
        components::format_secs,
        covers::CoverSize,
        i18n::t,
        loading::Destination,
        navigation::{AlbumKey, Route},
        style::theme::{TEXT, TEXT_MUTED},
    },
};
use std::path::PathBuf;

const COVER_MAX_SIZE: f32 = 320.0;
/// Размер текста деталей трека
const DETAILS_SIZE: f32 = 16.0;
const COVER_RADIUS: u8 = 12;
/// Фон начинает растворяться на столько выше низа центральной области
const BACKDROP_FADE_ABOVE: f32 = 120.0;

impl PlayerApp {
    /// Кнопка по центру всего оставшегося места
    pub(crate) fn render_add_tracks_block(&mut self, ui: &mut Ui) {
        let text = RichText::new(t().open_files).size(18.0).color(TEXT);

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
            .on_hover_text(t().open_files)
            .clicked()
        {
            self.dispatch(Action::AddFiles(Destination::Queue));
        }
    }

    /// Фон деталей текущего трека — его размытая обложка — под центральной областью `ui`
    /// и ниже, под панелью управления до низа окна. Внизу центральной области начинает
    /// растворяться и к низу окна исчезает: без резкой границы с панелью управления.
    /// При смене трека прежний фон плавно сменяется новым (см. `CoverFade`). Возвращает
    /// фигуру: её кладут под панели (см. `ui`), а не поверх них
    pub(crate) fn track_backdrop(&mut self, ui: &Ui) -> Option<Shape> {
        let track = self.core.controller.player().current_track.clone()?;
        let candidates = self.cover_candidates(&track);
        let cover = self.covers.get(&candidates, CoverSize::Backdrop);
        let progress = self.backdrop_fade.update(ui, cover);
        let content = ui.max_rect();
        let rect = Rect::from_min_max(
            content.min,
            pos2(content.right(), ui.ctx().content_rect().bottom()),
        );
        self.backdrop_fade
            .backdrop(rect, content.bottom() - BACKDROP_FADE_ABOVE, progress)
    }

    /// Где искать обложку трека: сначала в нём самом (у треков сборника картинки бывают
    /// разные), потом в остальных треках его альбома — обложка бывает не у всех
    pub(crate) fn cover_candidates(&mut self, track: &Track) -> Vec<PathBuf> {
        let mut candidates = vec![track.path.clone()];
        if let Some(album) = self.track_album(track) {
            candidates.extend(album.cover_candidates.iter().cloned());
        }
        candidates
    }

    /// Альбом трека в библиотеке — ищется так же, как при группировке: без учёта регистра,
    /// пустое имя — «Unknown …»
    fn track_album(&mut self, track: &Track) -> Option<&AlbumGroup> {
        self.core.refresh_groups();
        let artist_key = groups::album_artist(track).to_lowercase();
        let album_key = normalized_name(&track.album, groups::UNKNOWN_ALBUM);
        self.core.groups.albums.iter().find(|album| {
            album.title.to_lowercase() == album_key && album.artist.to_lowercase() == artist_key
        })
    }

    /// Детали текущего трека: название, исполнитель и альбом (ссылки — если они есть
    /// в библиотеке), длительность; ниже — обложка по центру свободного места
    pub(crate) fn render_current_track_info(&mut self, ui: &mut Ui) {
        let Some(track) = self.core.controller.player().current_track.clone() else {
            return;
        };
        // Исполнитель и альбом трека в библиотеке — ищутся так же, как при группировке
        let album_in_library = self.track_album(&track).map(AlbumKey::of);
        // Ссылка ведёт к исполнителю альбома: «Angèle; Kiddy Smile» — к Angèle
        let artist_key = groups::album_artist(&track).to_lowercase();
        let artist_in_library = self
            .core
            .groups
            .artists
            .iter()
            .find(|artist| artist.name.to_lowercase() == artist_key)
            .map(|artist| artist.name.clone());
        let cover_candidates = self.cover_candidates(&track);

        let mut open_artist = false;
        let mut open_album = false;
        ui.vertical(|ui| {
            ui.label(RichText::new(&track.title).size(DETAILS_SIZE).color(TEXT));
            if !track.artist.is_empty() {
                if artist_in_library.is_some() {
                    open_artist =
                        components::text_link(ui, &track.artist, DETAILS_SIZE, TEXT_MUTED)
                            .on_hover_text(t().show_artist)
                            .clicked();
                } else {
                    // Не из медиатеки — тот же вид, только без перехода
                    components::text_line(ui, &track.artist, DETAILS_SIZE, TEXT_MUTED);
                }
            }
            if !track.album.is_empty() {
                if album_in_library.is_some() {
                    open_album = components::text_link(ui, &track.album, DETAILS_SIZE, TEXT_MUTED)
                        .on_hover_text(t().show_album)
                        .clicked();
                } else {
                    components::text_line(ui, &track.album, DETAILS_SIZE, TEXT_MUTED);
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
            // Пока обложка нового трека читается, видна прежняя — см. `CoverFade`
            let cover = self.covers.get(&cover_candidates, CoverSize::Large);
            let progress = self.cover_fade.update(ui, cover);
            self.cover_fade.paint(
                ui,
                Rect::from_center_size(free.center(), vec2(side, side)),
                progress,
                COVER_RADIUS,
            );
        });

        if open_artist && let Some(artist) = artist_in_library {
            self.dispatch(Action::Navigate(Route::Artist(artist)));
        } else if open_album && let Some(album) = album_in_library {
            self.dispatch(Action::Navigate(Route::Album(album)));
        }
    }

    /// Плашка текущего трека внизу центральной области. Возвращает `true`, если по ней кликнули
    pub(crate) fn render_now_playing_bar(&mut self, ui: &mut Ui) -> bool {
        let Some(track) = &self.core.controller.player().current_track else {
            return false;
        };
        components::now_playing_bar(ui, &track.title, &track.artist)
            .on_hover_text(t().show_current_track)
            .clicked()
    }
}

/// Название альбома для сравнения — как при группировке: без пробелов по краям,
/// в нижнем регистре, пустое — `unknown`
fn normalized_name(text: &str, unknown: &str) -> String {
    let text = text.trim();
    if text.is_empty() { unknown } else { text }.to_lowercase()
}
