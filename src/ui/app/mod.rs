//! Окно плеера: состояние приложения и раскладка панелей. Каждая панель — отдельный
//! модуль с `impl PlayerApp`: дочерним модулям видны приватные поля `PlayerApp`.

mod controls;
mod library_panel;
mod library_views;
mod loading;
mod now_playing;
mod playlist_panel;
mod settings_dialog;

use eframe::egui::{self, Ui};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use self::loading::{Destination, PendingLoad};
use super::{
    covers::CoverCache,
    i18n,
    theme::WINDOW_TINT,
    widgets::{self, TileGridView, TrackListView},
};
use crate::{
    library::{
        Library,
        groups::{AlbumGroup, ArtistGroup},
        loader::Source,
    },
    playback::{PlayerController, PlayerState, PlaylistStore},
    settings::{Settings, SettingsStore},
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
    /// Где плейлист хранится между запусками
    playlist_store: PlaylistStore,
    /// Ревизия плейлиста, которая уже сохранена на диск
    saved_playlist_revision: u64,
    /// Настройки и их файл. Громкость живёт в контроллере, в настройки она копируется
    /// перед сохранением — см. `save_settings_if_changed`
    settings: Settings,
    settings_store: SettingsStore,
    /// Открыто ли окно настроек
    settings_open: bool,
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
    /// Сетки библиотеки — у каждой своё выделение и своя прокрутка
    artists_grid: TileGridView,
    albums_grid: TileGridView,
    artist_albums_grid: TileGridView,
    /// Поиск в открытом виде библиотеки
    search: library_views::LibrarySearch,
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
    /// Название на языке интерфейса
    fn title(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::NowPlaying => t.now_playing,
            Self::Artists => t.artists,
            Self::Albums => t.albums,
            Self::Songs => t.songs,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ListFocus {
    Library,
    Playlist,
    /// Сетка исполнителей или альбомов
    Grid,
}

impl PlayerApp {
    /// Из `cc` берётся контекст egui — чтобы аудиопоток мог разбудить приложение
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let data_dir = eframe::storage_dir(crate::APP_NAME);
        let (library, mut error) = Library::load(data_dir.clone());
        let (settings_store, settings, settings_error) = SettingsStore::load(data_dir.clone());
        if let Some(e) = settings_error {
            error.get_or_insert(e);
        }
        let playlist_store = PlaylistStore::new(data_dir);
        let saved_playlist = playlist_store.load().unwrap_or_else(|e| {
            error.get_or_insert(e);
            None
        });
        let outdated = library.outdated_paths();
        let mut app = Self {
            controller: PlayerController::new({
                // Трек доиграл — будим приложение: eframe вызовет `logic` и при скрытом окне
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            }),
            error,
            drag_secs: None,
            loaders: VecDeque::new(),
            loader_shown_at: None,
            library,
            playlist_store,
            saved_playlist_revision: 0,
            settings,
            settings_store,
            settings_open: false,
            library_view: TrackListView::new("library"),
            playlist_view: TrackListView::new("playlist"),
            focus: ListFocus::Playlist,
            view: CentralView::NowPlaying,
            selected_artist: None,
            selected_album: None,
            album_tracks_view: TrackListView::new("album_tracks"),
            artists_grid: TileGridView::new("artists"),
            albums_grid: TileGridView::new("albums"),
            artist_albums_grid: TileGridView::new("artist_albums"),
            search: library_views::LibrarySearch::default(),
            pending_removal: None,
            groups: LibraryGroups::default(),
            covers: CoverCache::new(),
        };
        if let Some(list) = saved_playlist {
            app.controller.restore_track_list(list);
        }
        // Восстановленный плейлист и так лежит на диске
        app.saved_playlist_revision = app.controller.playlist_revision();
        app.controller.set_volume(app.settings.volume());
        i18n::set_language(app.settings.language());
        // Библиотека сохранена старым форматом — один раз перечитываем её теги в фоне
        if let Some(paths) = outdated {
            app.start_loading(Source::Files(paths), Destination::LibraryRefresh);
        }
        app
    }

    /// Переключает центральную область на вид `view`. Поиск относится к виду, который
    /// был открыт, — при смене вида он очищается
    fn set_view(&mut self, view: CentralView) {
        if self.view == view {
            return;
        }
        self.view = view;
        if !self.search.is_empty() {
            self.search = library_views::LibrarySearch::default();
            // Строки Songs во время поиска — найденные треки: выделение относилось к ним
            self.library_view.clear_selection();
        }
    }

    /// Сохраняет плейлист, если он изменился. Вызывается в конце `logic` (изменения без окна:
    /// следующий трек) и в конце `ui` (изменения из интерфейса). Сколько бы изменений ни было
    /// за кадр (например, «Play all» и меняет треки, и выбирает текущий), файл пишется один раз
    fn save_playlist_if_changed(&mut self) {
        let revision = self.controller.playlist_revision();
        if revision == self.saved_playlist_revision {
            return;
        }
        // Ревизия считается сохранённой и при ошибке: иначе запись повторялась бы каждый кадр.
        // Следующее изменение попробует снова
        self.saved_playlist_revision = revision;
        let result = self
            .playlist_store
            .save(&self.controller.player().track_list);
        self.set_result(result);
    }

    /// Сохраняет настройки, если они изменились. Вызывается там же, где
    /// `save_playlist_if_changed`. Пока зажата кнопка мыши, жест не закончен (например,
    /// тянут ползунок громкости) — файл пишется один раз, когда кнопку отпустят
    fn save_settings_if_changed(&mut self, ctx: &egui::Context) {
        self.settings.update_volume(self.controller.volume());
        if ctx.input(|input| input.pointer.any_down()) {
            return;
        }
        let result = self.settings_store.save_if_changed(&self.settings);
        self.set_result(result);
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

    /// Всё, что должно работать и без интерфейса: eframe вызывает это перед каждым `ui`,
    /// а когда окно скрыто (другой рабочий стол, свёрнуто, закрыто другими окнами) и `ui`
    /// не вызывается совсем — по `request_repaint`. Так следующий трек включается, даже
    /// когда окно не видно: аудиопоток будит приложение, когда трек доиграл
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Трек доиграл — переходим к следующему в очереди
        if self.controller.poll_events()
            && let Err(e) = self.controller.play_next()
        {
            self.error = Some(e);
        }
        // Язык интерфейса — из настроек; сменили настройку — со следующего кадра новый язык
        i18n::set_language(self.settings.language());
        self.poll_loaders(ctx);
        self.save_playlist_if_changed();
        self.save_settings_if_changed(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.covers.update(ui.ctx());

        // Пока трек играет, обновляем время и ползунок. Это перерисовка видимого окна;
        // переход к следующему треку от неё не зависит — см. `logic`
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
                egui::Panel::bottom("library_sidebar_bottom")
                    .frame(egui::Frame::new().inner_margin(16.0))
                    .resizable(false)
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        self.render_settings_button(ui);
                    });
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
                            self.set_view(CentralView::NowPlaying);
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
        self.render_settings_dialog(ui.ctx());

        // Плашки поверх всего окна, по центру сверху: прогресс добавления (только когда
        // poll_loaders решил его показать) и под ним ошибка — пока не нажмут «OK»
        let mut banner_top = widgets::BANNER_TOP;
        if self.loader_shown_at.is_some()
            && let Some(pending) = self.loaders.front()
        {
            let (cancel, bottom) = widgets::loading_banner(
                ui.ctx(),
                pending.progress_template(),
                pending.loader.progress(),
            );
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

        self.save_playlist_if_changed();
        self.save_settings_if_changed(ui.ctx());
    }
}
