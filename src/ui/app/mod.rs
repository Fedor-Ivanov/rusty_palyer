//! Окно плеера: состояние приложения и раскладка панелей. Каждая панель — отдельный
//! модуль с `impl PlayerApp`: дочерним модулям видны приватные поля `PlayerApp`.

mod controls;
mod library_panel;
mod library_views;
mod loading;
mod now_playing;
mod playlist_panel;

use eframe::egui::{self, Ui};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use self::loading::{Destination, PendingLoad};
use super::{
    covers::CoverCache,
    theme::WINDOW_TINT,
    widgets::{self, TrackListView},
};
use crate::{
    library::{
        Library,
        groups::{AlbumGroup, ArtistGroup},
        loader::Source,
    },
    playback::{PlayerController, PlayerState},
};

// Кнопки-иконки в заголовке библиотеки и в нижнем блоке списка
const ADD_BUTTON_SIZE: f32 = 24.0;
// Ширина боковых панелей (библиотека и список треков): пределы ресайза и стартовая ширина
const SIDE_PANEL_MIN_WIDTH: f32 = 180.0;
const SIDE_PANEL_MAX_WIDTH: f32 = 420.0;
const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
const TRACK_LIST_DEFAULT_WIDTH: f32 = 300.0;
// Минимальная ширина центральной области (CentralPanel): её обеспечивают максимумы боковых панелей
const CONTENT_MIN_WIDTH: f32 = 260.0;
// Зазор над плашкой текущего трека
const NOW_PLAYING_BAR_GAP: f32 = 8.0;

pub struct PlayerApp {
    controller: PlayerController,
    error: Option<String>,
    /// Позиция ползунка (в секундах), пока пользователь его тянет
    drag_secs: Option<f32>,
    /// Фоновые добавления треков. Обрабатываются по очереди: треки второго появятся
    /// в списке после треков первого, даже если он закончит раньше
    loaders: VecDeque<PendingLoad>,
    /// С какого момента виден прогресс текущего добавления; `None` — не виден
    loader_shown_at: Option<Instant>,
    /// Треки, сохранённые между запусками
    library: Library,
    library_view: TrackListView,
    playlist_view: TrackListView,
    /// Какой список получает клавиши (⌘A, Esc, Delete) — тот, с которым работали последним
    focus: ListFocus,
    /// Что показано в центральной области
    view: CentralView,
    /// Исполнитель, чьи альбомы открыты в виде «Artists»; `None` — показаны все исполнители
    selected_artist: Option<String>,
    /// Открытый альбом — из «Albums» или из профиля исполнителя
    selected_album: Option<library_views::AlbumKey>,
    /// Список треков открытого альбома
    album_tracks_view: TrackListView,
    /// Альбом или исполнитель, удаление которых из библиотеки ждёт подтверждения в диалоге
    pending_removal: Option<library_views::LibraryRemoval>,
    /// Исполнители и альбомы библиотеки — пересчитываются, только когда библиотека изменилась
    groups: LibraryGroups,
    /// Миниатюры обложек для сетки альбомов
    covers: CoverCache,
}

#[derive(Default)]
struct LibraryGroups {
    /// Ревизия библиотеки, по которой посчитаны группы; `None` — ещё не считались
    revision: Option<u64>,
    artists: Vec<ArtistGroup>,
    albums: Vec<AlbumGroup>,
}

/// Что показано в центральной области: текущий трек или один из видов библиотеки
#[derive(Clone, Copy, PartialEq)]
enum CentralView {
    NowPlaying,
    Artists,
    Albums,
    Songs,
}

impl CentralView {
    fn title(self) -> &'static str {
        match self {
            Self::NowPlaying => "Now Playing",
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Songs => "Songs",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ListFocus {
    Library,
    Playlist,
}

impl PlayerApp {
    pub fn new() -> Self {
        let (library, error) = Library::load(eframe::storage_dir(crate::APP_NAME));
        let outdated = library.outdated_paths();
        let mut app = Self {
            controller: PlayerController::new(),
            error,
            drag_secs: None,
            loaders: VecDeque::new(),
            loader_shown_at: None,
            library,
            library_view: TrackListView::new("library"),
            playlist_view: TrackListView::new("playlist"),
            focus: ListFocus::Playlist,
            view: CentralView::NowPlaying,
            selected_artist: None,
            selected_album: None,
            album_tracks_view: TrackListView::new("album_tracks"),
            pending_removal: None,
            groups: LibraryGroups::default(),
            covers: CoverCache::new(),
        };
        // Библиотека сохранена старым форматом — один раз перечитываем её теги в фоне
        if let Some(paths) = outdated {
            app.start_loading(Source::Files(paths), Destination::LibraryRefresh);
        }
        app
    }

    /// Запоминает ошибку для плашки. Успех её не сбрасывает: плашка закрывается только
    /// кнопкой «OK», иначе ошибка исчезала бы сама при следующем удачном действии
    fn set_result(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
}

impl eframe::App for PlayerApp {
    /// Окно прозрачное: под ним система рисует размытие, поэтому сами ничего не заливаем
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Трек доиграл — переходим к следующему в очереди
        if self.controller.poll_events()
            && let Err(e) = self.controller.play_next()
        {
            self.error = Some(e);
        }

        self.poll_loaders(ui.ctx());
        self.covers.update(ui.ctx());

        // Пока трек играет, обновляем время и ползунок
        if self.controller.player().state == PlayerState::Playing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // Тонировка системного размытия — первой, под всеми панелями. Других заливок у панелей
        // нет: стекло по всему окну одного цвета
        ui.painter().rect_filled(ui.max_rect(), 0, WINDOW_TINT);

        // У CentralPanel нет своей минимальной ширины — он получает остаток после боковых
        // панелей. Поэтому минимум обеспечивают их максимумы: каждая может занять только то,
        // что останется после минимумов всего, что ещё не размещено. При сужении окна боковые
        // панели сжимаются первыми, а центральная не становится уже CONTENT_MIN_WIDTH
        let sidebar_max_width = (ui.available_width() - SIDE_PANEL_MIN_WIDTH - CONTENT_MIN_WIDTH)
            .clamp(SIDE_PANEL_MIN_WIDTH, SIDE_PANEL_MAX_WIDTH);

        // egui подсвечивает край панели белой линией при наведении и перетаскивании —
        // толщину берёт из hovered/active fg_stroke родительского Ui. Вместо неё у нас своя
        // линия в тон интерфейса (paint_resize_line), поэтому на время боковых панелей
        // толщина обнуляется. Меняется только толщина: цвет текста виджетов внутри не страдает
        let widgets = &mut ui.style_mut().visuals.widgets;
        let edge_strokes = (
            widgets.hovered.fg_stroke.width,
            widgets.active.fg_stroke.width,
        );
        widgets.hovered.fg_stroke.width = 0.0;
        widgets.active.fg_stroke.width = 0.0;

        // Библиотека — у левого края окна
        let sidebar = egui::Panel::left("library_sidebar")
            .frame(egui::Frame::new())
            .default_size(SIDEBAR_DEFAULT_WIDTH)
            .size_range(SIDE_PANEL_MIN_WIDTH..=sidebar_max_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_library(ui);
            });

        // Считается после боковой панели: available_width уже без её ширины
        let track_list_max_width = (ui.available_width() - CONTENT_MIN_WIDTH)
            .clamp(SIDE_PANEL_MIN_WIDTH, SIDE_PANEL_MAX_WIDTH);

        // Список треков — у правого края окна
        let track_list = egui::Panel::right("track_list")
            .frame(egui::Frame::new())
            .default_size(TRACK_LIST_DEFAULT_WIDTH)
            .size_range(SIDE_PANEL_MIN_WIDTH..=track_list_max_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                // Panel подгоняет ширину под содержимое, поэтому без этого она сжимается
                // обратно до минимума сразу после перетаскивания края
                ui.set_min_width(ui.available_width());

                // Блок прижат к низу панели. Добавляется до списка: панель сразу забирает
                // своё место, а ScrollArea получает только оставшееся и под блок не заходит
                egui::Panel::bottom("track_list_bottom")
                    // Без своего frame панель залила бы себя фоном по умолчанию
                    .frame(egui::Frame::new().inner_margin(16.0))
                    .resizable(false)
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        self.render_playback_modes(ui);
                    });

                self.render_playlist(ui);
            });

        let widgets = &mut ui.style_mut().visuals.widgets;
        (
            widgets.hovered.fg_stroke.width,
            widgets.active.fg_stroke.width,
        ) = edge_strokes;

        // Панели нужно добавлять до CentralPanel — он занимает всё оставшееся место.
        // Добавлена после боковых, поэтому тянется только под центральной карточкой
        egui::Panel::bottom("audio_controls")
            .frame(egui::Frame::new())
            .show_separator_line(false)
            .show(ui, |ui| {
                // Без стекла: контролы лежат прямо на фоне окна
                egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                    self.render_controls(ui);
                });
            });

        // Центральная область без своего фона: под ней тонированное системное размытие
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                // Открыт вид библиотеки, а трек есть — внизу плашка с ним, по клику на которую
                // возвращаемся к деталям трека. Добавляется до содержимого: панель сразу
                // забирает своё место, и содержимое под неё не заходит.
                // Появляется и прячется, выезжая снизу: show_collapsible анимирует это сам,
                // и содержимое над плашкой сдвигается вместе с ней. Высота задана точно —
                // иначе в первом кадре панель не знает её и прыгает
                let mut show_bar = self.view != CentralView::NowPlaying
                    && self.controller.player().current_track.is_some();
                egui::Panel::bottom("now_playing_bar")
                    .frame(egui::Frame::new().inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: NOW_PLAYING_BAR_GAP as i8,
                        bottom: 0,
                    }))
                    .exact_size(widgets::NOW_PLAYING_BAR_HEIGHT + NOW_PLAYING_BAR_GAP)
                    .resizable(false)
                    .drag_to_open(false)
                    .show_separator_line(false)
                    .show_collapsible(ui, &mut show_bar, |ui| {
                        if self.render_now_playing_bar(ui) {
                            self.view = CentralView::NowPlaying;
                        }
                    });

                match self.view {
                    CentralView::NowPlaying => {
                        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                            ui.vertical_centered_justified(|ui| {
                                if self.controller.player().track_list.tracks.is_empty() {
                                    self.render_add_tracks_block(ui);
                                } else {
                                    self.render_current_track_info(ui);
                                }
                            })
                        });
                    }
                    view => self.render_library_view(ui, view),
                }
            });

        self.render_removal_dialog(ui.ctx());

        // Плашки поверх всего окна, по центру сверху: прогресс добавления (только когда
        // poll_loaders решил его показать) и под ним ошибка — пока не нажмут «OK»
        let mut banner_top = widgets::BANNER_TOP;
        if self.loader_shown_at.is_some()
            && let Some(pending) = self.loaders.front()
        {
            let (cancel, bottom) =
                widgets::loading_banner(ui.ctx(), pending.action(), pending.loader.progress());
            banner_top = bottom - ui.ctx().content_rect().top() + widgets::BANNER_GAP;
            if cancel {
                self.cancel_loading();
            }
        }
        if let Some(error) = &self.error
            && widgets::error_banner(ui.ctx(), error, banner_top)
        {
            self.error = None;
        }

        // Линии ресайза — последними, поверх всего, на границе панелей — там, где egui даёт тянуть край
        let sidebar_rect = sidebar.response.rect;
        widgets::paint_resize_line(ui, sidebar_rect.right(), sidebar_rect.y_range());
        let track_list_rect = track_list.response.rect;
        widgets::paint_resize_line(ui, track_list_rect.left(), track_list_rect.y_range());
    }
}
