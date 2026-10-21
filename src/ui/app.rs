//! Окно плеера: состояние приложения (`PlayerApp`), кадр и раскладка панелей.
//! Сами панели, страницы и диалоги — в `panels`, `pages`, `dialogs`: каждый файл там —
//! `impl PlayerApp` со своей частью интерфейса.

use eframe::egui::{self, Ui};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use super::{
    action::Action,
    components::{self, CoverFade, TileGridView, TrackListView},
    covers::CoverCache,
    dialogs::remove::LibraryRemoval,
    i18n,
    loading::{Destination, PendingLoad},
    navigation::Route,
    pages::LibrarySearch,
    style::theme::WINDOW_TINT,
};
use crate::core::{Core, library::loader::Source, playback::PlayerState};

// Кнопки-иконки в заголовке библиотеки и в нижнем блоке списка
pub(crate) const ADD_BUTTON_SIZE: f32 = 24.0;
// Ширина боковых панелей (библиотека и список треков): пределы ресайза и стартовая ширина
pub(crate) const SIDE_PANEL_MIN_WIDTH: f32 = 180.0;
pub(crate) const SIDE_PANEL_MAX_WIDTH: f32 = 420.0;
pub(crate) const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
pub(crate) const TRACK_LIST_DEFAULT_WIDTH: f32 = 300.0;
// Минимальная ширина центральной области (CentralPanel): её обеспечивают максимумы боковых панелей
pub(crate) const CONTENT_MIN_WIDTH: f32 = 260.0;
// Зазор над плашкой текущего трека
pub(crate) const NOW_PLAYING_BAR_GAP: f32 = 8.0;

pub struct PlayerApp {
    /// Данные приложения: медиатека, плеер с плейлистом, настройки. Меняются только
    /// в `action::apply`
    pub(crate) core: Core,
    /// Действия, отправленные за кадр (`dispatch`): выполняются в его конце — см. `action`
    pub(crate) actions: Vec<Action>,
    pub(crate) error: Option<String>,
    /// Позиция ползунка (в секундах), пока пользователь его тянет
    pub(crate) drag_secs: Option<f32>,
    /// Фоновые добавления треков. Обрабатываются по очереди: треки второго появятся
    /// в списке после треков первого, даже если он закончит раньше
    pub(crate) loaders: VecDeque<PendingLoad>,
    /// С какого момента виден прогресс текущего добавления; `None` — не виден
    pub(crate) loader_shown_at: Option<Instant>,
    /// Открыто ли окно настроек
    pub(crate) settings_open: bool,
    pub(crate) library_view: TrackListView,
    pub(crate) playlist_view: TrackListView,
    /// Какой список получает клавиши (⌘A, Esc, Delete) — тот, с которым работали последним
    pub(crate) focus: ListFocus,
    /// Что открыто в центральной области. Меняется только через `navigate`
    pub(crate) route: Route,
    /// Список треков открытого альбома
    pub(crate) album_tracks_view: TrackListView,
    /// Сетки библиотеки — у каждой своё выделение и своя прокрутка
    pub(crate) artists_grid: TileGridView,
    pub(crate) albums_grid: TileGridView,
    pub(crate) artist_albums_grid: TileGridView,
    /// Поиск в открытом виде библиотеки
    pub(crate) search: LibrarySearch,
    /// Альбом или исполнитель, удаление которых из библиотеки ждёт подтверждения в диалоге
    pub(crate) pending_removal: Option<LibraryRemoval>,
    /// Миниатюры обложек для сетки альбомов
    pub(crate) covers: CoverCache,
    /// Обложка и фон текущего трека: плавная смена при переключении трека
    pub(crate) cover_fade: CoverFade,
    pub(crate) backdrop_fade: CoverFade,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ListFocus {
    Library,
    Playlist,
    /// Сетка исполнителей или альбомов
    Grid,
}

impl PlayerApp {
    /// Из `cc` берётся контекст egui — чтобы аудиопоток мог разбудить приложение
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let (core, error) = Core::load(eframe::storage_dir(crate::APP_NAME), {
            // Трек доиграл — будим приложение: eframe вызовет `logic` и при скрытом окне
            let ctx = cc.egui_ctx.clone();
            move || ctx.request_repaint()
        });
        i18n::set_language(core.settings.language());
        let outdated = core.library.outdated_paths();
        // Последнего трека нет (первый запуск или его не было при выходе) — в Now Playing
        // показать нечего, открываем альбомы
        let route = if core.controller.player().current_track.is_none() {
            Route::Albums
        } else {
            Route::NowPlaying
        };
        let mut app = Self {
            core,
            actions: Vec::new(),
            error,
            drag_secs: None,
            loaders: VecDeque::new(),
            loader_shown_at: None,
            settings_open: false,
            library_view: TrackListView::new("library"),
            playlist_view: TrackListView::new("playlist"),
            focus: ListFocus::Playlist,
            route,
            album_tracks_view: TrackListView::new("album_tracks"),
            artists_grid: TileGridView::new("artists"),
            albums_grid: TileGridView::new("albums"),
            artist_albums_grid: TileGridView::new("artist_albums"),
            search: LibrarySearch::default(),
            pending_removal: None,
            covers: CoverCache::new(),
            cover_fade: CoverFade::default(),
            backdrop_fade: CoverFade::default(),
        };
        // Библиотека сохранена старым форматом — один раз перечитываем её теги в фоне
        if let Some(paths) = outdated {
            app.start_loading(Source::Files(paths), Destination::LibraryRefresh);
        }
        app
    }

    /// Сохраняет плейлист и настройки, если они изменились. Вызывается в конце `logic`
    /// (изменения без окна: следующий трек) и в конце `ui` (изменения из интерфейса).
    /// Пока зажата кнопка мыши, жест не закончен (например, тянут ползунок громкости) —
    /// настройки пишутся один раз, когда кнопку отпустят
    pub(crate) fn save_if_changed(&mut self, ctx: &egui::Context) {
        let result = self.core.save_playlist_if_changed();
        self.report_error(result);
        if !ctx.input(|input| input.pointer.any_down()) {
            let result = self.core.save_settings_if_changed();
            self.report_error(result);
        }
    }

    /// Сообщает об ошибке, если `result` — ошибка: она появится на плашке. `Ok` ничего
    /// не делает и уже показанную ошибку не сбрасывает: плашка закрывается только кнопкой
    /// «OK», иначе ошибка исчезала бы сама при следующем удачном действии
    pub(crate) fn report_error(&mut self, result: Result<(), String>) {
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
        if self.core.controller.poll_events() {
            self.dispatch(Action::TrackEnded);
        }
        self.poll_loaders(ctx);
        self.apply_actions(ctx);
        self.save_if_changed(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.covers.update(ui.ctx());

        // Пока трек играет, обновляем время и ползунок. Это перерисовка видимого окна;
        // переход к следующему треку от неё не зависит — см. `logic`
        if self.core.controller.player().state == PlayerState::Playing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // Тонировка системного размытия — первой, под всеми панелями. Других заливок у панелей
        // нет: стекло по всему окну одного цвета
        ui.painter().rect_filled(ui.max_rect(), 0, WINDOW_TINT);
        // Место под фон Now Playing — сразу над тонировкой, под всеми панелями. Сам фон
        // известен, только когда дойдём до центральной области, а заходит он и под панель
        // управления, нарисованную раньше, — поэтому место занимаем заранее
        let root_painter = ui.painter().clone();
        let backdrop_slot = root_painter.add(egui::Shape::Noop);

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
            .show(ui, |ui| self.render_sidebar(ui));

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
                let mut show_bar = self.route != Route::NowPlaying
                    && self.core.controller.player().current_track.is_some();
                egui::Panel::bottom("now_playing_bar")
                    .frame(egui::Frame::new().inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: NOW_PLAYING_BAR_GAP as i8,
                        bottom: 0,
                    }))
                    .exact_size(components::NOW_PLAYING_BAR_HEIGHT + NOW_PLAYING_BAR_GAP)
                    .resizable(false)
                    .drag_to_open(false)
                    .show_separator_line(false)
                    .show_collapsible(ui, &mut show_bar, |ui| {
                        if self.render_now_playing_bar(ui) {
                            self.dispatch(Action::Navigate(Route::NowPlaying));
                        }
                    });

                match self.route {
                    Route::NowPlaying => {
                        if let Some(backdrop) = self.track_backdrop(ui) {
                            root_painter.set(backdrop_slot, backdrop);
                        }
                        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                            ui.vertical_centered_justified(|ui| {
                                if self.core.controller.player().track_list.tracks.is_empty() {
                                    self.render_add_tracks_block(ui);
                                } else {
                                    self.render_current_track_info(ui);
                                }
                            })
                        });
                    }
                    _ => self.render_library_view(ui),
                }
            });

        self.render_removal_dialog(ui.ctx());
        self.render_settings_dialog(ui.ctx());

        // Плашки поверх всего окна, по центру сверху: прогресс добавления (только когда
        // poll_loaders решил его показать) и под ним ошибка — пока не нажмут «OK»
        let mut banner_top = components::BANNER_TOP;
        if self.loader_shown_at.is_some()
            && let Some(pending) = self.loaders.front()
        {
            let (cancel, bottom) = components::loading_banner(
                ui.ctx(),
                pending.progress_template(),
                pending.loader.progress(),
            );
            banner_top = bottom - ui.ctx().content_rect().top() + components::BANNER_GAP;
            if cancel {
                self.dispatch(Action::CancelLoading);
            }
        }
        if let Some(error) = &self.error
            && components::error_banner(ui.ctx(), error, banner_top)
        {
            self.error = None;
        }

        // Линии ресайза — последними, поверх всего, на границе панелей — там, где egui даёт тянуть край
        let sidebar_rect = sidebar.response.rect;
        components::paint_resize_line(ui, sidebar_rect.right(), sidebar_rect.y_range());
        let track_list_rect = track_list.response.rect;
        components::paint_resize_line(ui, track_list_rect.left(), track_list_rect.y_range());

        // Всё, что пользователь сделал за кадр, — после отрисовки, до сохранения
        self.apply_actions(ui.ctx());
        self.save_if_changed(ui.ctx());
    }
}
