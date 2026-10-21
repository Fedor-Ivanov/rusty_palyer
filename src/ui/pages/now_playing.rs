//! Центральная область: информация о текущем треке и обложка, без треков — «Open files»;
//! плашка текущего трека, пока открыт вид библиотеки.

use eframe::egui::{
    self, Align, Align2, CursorIcon, FontId, Label, Layout, Rect, RichText, ScrollArea, Sense,
    Shape, Ui, UiBuilder, pos2, vec2,
};

use crate::{
    core::library::{
        Track,
        groups::{self, AlbumGroup},
        metadata::{FileMetadata, get_file_metadata},
    },
    ui::action::Action,
    ui::{
        app::{ADD_BUTTON_SIZE, PlayerApp},
        components,
        components::format_secs,
        covers::CoverSize,
        i18n::{fill, t},
        loading::Destination,
        navigation::{AlbumKey, Route},
        style::icons::paint_lyrics,
        style::theme::{COVER_CROSSFADE_SECS, TEXT, TEXT_FAINT, TEXT_MUTED},
    },
};
use std::path::PathBuf;

const COVER_MAX_SIZE: f32 = 320.0;
/// Размер текста деталей трека
const DETAILS_SIZE: f32 = 16.0;
const COVER_RADIUS: u8 = 12;
/// Текст песни: размер строк и зазор между ними
const LYRICS_SIZE: f32 = 20.0;
const LYRICS_LINE_GAP: f32 = 10.0;
/// Отступ текста песни от деталей трека над ним — больше, чем у обложки: строки текста
/// иначе сливаются с деталями
const LYRICS_TOP_GAP: f32 = 16.0;
/// На столько текст песни выезжает снизу, появляясь вместо обложки
const LYRICS_SLIDE: f32 = 12.0;
/// Наименьший зазор между названием трека и кнопкой текста песни справа от него
const LYRICS_BUTTON_GAP: f32 = 8.0;
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
    /// в библиотеке), год и жанр, длительность и битрейт; ниже — обложка по центру
    /// свободного места
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
        let details = self.current_details(&track);
        // «2006 · Instrumental Rock» — что из этого есть. Год — из файла, а если файл
        // не прочитался — из медиатеки
        let year = details
            .as_ref()
            .and_then(|details| details.year)
            .or(track.year);
        let genre = details.as_ref().map(|details| details.genre.trim());
        let year_and_genre: Vec<String> = year
            .map(|year| year.to_string())
            .into_iter()
            .chain(genre.filter(|genre| !genre.is_empty()).map(str::to_owned))
            .collect();
        // «4:02 · 320 kbps»
        let mut length = format_secs(track.duration_secs as f32);
        if let Some(kbps) = details.as_ref().and_then(|details| details.bitrate_kbps) {
            length += " · ";
            length += &fill(t().bitrate, &[("kbps", &kbps.to_string())]);
        }

        let mut open_artist = false;
        let mut open_album = false;
        let mut seek_to = None;
        let mut toggle_lyrics = false;
        ui.vertical(|ui| {
            // Название — слева, переносится, не заходя под кнопку текста песни справа.
            // Кнопка — по центру первой строки названия
            let full_width = ui.available_width();
            let title = ui
                .scope(|ui| {
                    ui.set_max_width(full_width - ADD_BUTTON_SIZE - LYRICS_BUTTON_GAP);
                    ui.label(RichText::new(&track.title).size(DETAILS_SIZE).color(TEXT))
                })
                .inner;
            let first_line =
                ui.fonts_mut(|fonts| fonts.row_height(&FontId::proportional(DETAILS_SIZE)));
            let button_rect = Rect::from_center_size(
                pos2(
                    title.rect.left() + full_width - ADD_BUTTON_SIZE / 2.0,
                    title.rect.top() + first_line / 2.0,
                ),
                vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
            );
            let (clicked, active, hover) =
                components::toggle_button(ui, button_rect, "lyrics", self.lyrics_open, t().lyrics);
            paint_lyrics(ui, button_rect, active, hover);
            toggle_lyrics = clicked;

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
            if !year_and_genre.is_empty() {
                components::text_line(ui, &year_and_genre.join(" · "), DETAILS_SIZE, TEXT_MUTED);
            }
            components::text_line(ui, &length, DETAILS_SIZE, TEXT_MUTED);
            ui.add_space(8.0);

            // Ниже — всё оставшееся место: текст песни или обложка. При переключении они
            // плавно сменяют друг друга, текст при этом чуть выезжает снизу
            let free = ui.available_rect_before_wrap();
            ui.allocate_rect(free, Sense::hover());
            let lyrics_shown = ui.ctx().animate_bool_with_time_and_easing(
                egui::Id::unique("lyrics_shown"),
                self.lyrics_open,
                COVER_CROSSFADE_SECS,
                egui::emath::easing::cubic_in_out,
            );
            if lyrics_shown > 0.0 {
                seek_to = ui
                    .scope(|ui| {
                        ui.multiply_opacity(lyrics_shown);
                        let slide = vec2(0.0, (1.0 - lyrics_shown) * LYRICS_SLIDE);
                        self.render_lyrics(ui, free.translate(slide), &track)
                    })
                    .inner;
            }
            if lyrics_shown < 1.0 {
                ui.scope(|ui| {
                    ui.multiply_opacity(1.0 - lyrics_shown);
                    // Обложка — по центру, по ширине и по высоте; на небольшом окне
                    // уменьшается, чтобы помещаться
                    let side = free.width().min(free.height()).clamp(0.0, COVER_MAX_SIZE);
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
            }
        });

        if toggle_lyrics {
            self.dispatch(Action::ToggleLyrics);
        }
        if let Some(secs) = seek_to {
            self.dispatch(Action::Seek(secs));
        }
        if open_artist && let Some(artist) = artist_in_library {
            self.dispatch(Action::Navigate(Route::Artist(artist)));
        } else if open_album && let Some(album) = album_in_library {
            self.dispatch(Action::Navigate(Route::Album(album)));
        }
    }

    /// Текст песни в `area`. Синхронизированный: текущая строка светлее остальных, текст
    /// прокручивается за ней, клик по строке перематывает к ней — тогда возвращает, куда
    /// (в секундах). Обычный — просто текст. Нет текста — заглушка
    fn render_lyrics(&mut self, ui: &mut Ui, area: Rect, track: &Track) -> Option<f32> {
        let area = area.with_min_y(area.top() + LYRICS_TOP_GAP);
        if !matches!(&self.current_lyrics, Some((path, _)) if *path == track.path) {
            self.current_lyrics = Some((track.path.clone(), self.core.lyrics.find(track)));
        }
        let Some((_, found)) = &self.current_lyrics else {
            return None;
        };
        let Some(lyrics) = &found.lyrics else {
            let message = if found.searching {
                t().searching_lyrics
            } else if found.instrumental {
                t().instrumental
            } else if found.unavailable {
                t().lyrics_unavailable
            } else {
                t().no_lyrics
            };
            ui.painter().text(
                area.center(),
                Align2::CENTER_CENTER,
                message,
                FontId::proportional(DETAILS_SIZE),
                TEXT_FAINT,
            );
            return None;
        };

        let current = lyrics.current_line(self.core.controller.position_ms());
        let scroll_to_current = current.is_some() && current != self.lyrics_line;
        self.lyrics_line = current;
        let mut seek_to = None;
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::top_down(Align::Min)),
        );
        // У каждого трека своя прокрутка: новый текст начинается сверху
        ScrollArea::vertical()
            .id_salt(("lyrics", &track.path))
            .auto_shrink(false)
            .show(&mut child, |ui| {
                for (index, line) in lyrics.lines.iter().enumerate() {
                    // Пустая строка: в синхронизированном тексте — проигрыш, в обычном —
                    // промежуток между куплетами
                    if line.text.is_empty() && !lyrics.synced {
                        ui.add_space(LYRICS_SIZE);
                        continue;
                    }
                    let text = if line.text.is_empty() {
                        "♪"
                    } else {
                        &line.text
                    };
                    // Синхронизированный: поётся сейчас — яркая, остальные — бледные, чтобы
                    // текущая была видна сразу и на светлом фоне обложки
                    let color = if !lyrics.synced || current == Some(index) {
                        TEXT
                    } else {
                        TEXT_FAINT
                    };
                    let mut label =
                        Label::new(RichText::new(text).size(LYRICS_SIZE).color(color)).wrap();
                    if lyrics.synced {
                        label = label.sense(Sense::click());
                    }
                    let response = ui.add(label);
                    if let Some(start) = line.start_ms {
                        let response = response.on_hover_cursor(CursorIcon::PointingHand);
                        if response.clicked() {
                            seek_to = Some(start as f32 / 1000.0);
                        }
                        if scroll_to_current && current == Some(index) {
                            response.scroll_to_me(Some(Align::Center));
                        }
                    }
                    ui.add_space(LYRICS_LINE_GAP);
                }
                // Запас снизу: последние строки тоже доходят до середины
                if lyrics.synced {
                    ui.add_space(area.height() / 2.0);
                }
            });
        seek_to
    }

    /// Теги файла текущего трека: читаются, только когда трек сменился
    fn current_details(&mut self, track: &Track) -> Option<FileMetadata> {
        match &self.current_details {
            Some((path, details)) if *path == track.path => details.clone(),
            _ => {
                let details = get_file_metadata(&track.path).ok();
                self.current_details = Some((track.path.clone(), details.clone()));
                details
            }
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
