//! Добавление треков: кнопка «+» с меню, выбор файлов и папки, очередь фоновых
//! добавлений, прогресс. Каждое добавление знает, куда идут треки: в библиотеку или плейлист.

use eframe::egui::{self, Rect, Ui, pos2, vec2};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use super::app::{ADD_BUTTON_SIZE, PlayerApp};
use crate::{
    core::library::{
        loader::{Finished, Loader, Source},
        scan::SUPPORTED_EXTENSIONS,
    },
    ui::{
        action::Action,
        components,
        i18n::{fill, t},
        style::icons::paint_add,
    },
};

// Фоновое добавление: как часто проверять готовность, через сколько показывать прогресс
// (быстрое добавление обходится без него) и сколько минимум его держать, чтобы не мелькал
const LOADER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOADER_SHOW_DELAY: Duration = Duration::from_millis(400);
const LOADER_MIN_VISIBLE: Duration = Duration::from_millis(500);

/// Куда идут добавленные треки
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Destination {
    /// Сохраняются между запусками
    Library,
    /// Только послушать сейчас, в библиотеку не попадают
    Playlist,
    /// Перечитать теги треков, уже лежащих в библиотеке (после смены формата)
    LibraryRefresh,
}

/// Фоновое добавление в очереди
pub(crate) struct PendingLoad {
    pub(crate) loader: Loader,
    destination: Destination,
}

impl PendingLoad {
    /// Текст плашки прогресса с `{done}` и `{total}` — зависит от того, что делаем
    pub(crate) fn progress_template(&self) -> &'static str {
        match self.destination {
            Destination::Library | Destination::Playlist => t().adding_progress,
            Destination::LibraryRefresh => t().updating_library_progress,
        }
    }
}

impl PlayerApp {
    /// Кнопка «+» у правого края строки заголовка `row` с меню «Add files» / «Add folder»
    pub(crate) fn render_add_button(&mut self, ui: &mut Ui, row: Rect, destination: Destination) {
        let add_rect = Rect::from_center_size(
            pos2(row.right() - ADD_BUTTON_SIZE / 2.0, row.center().y),
            vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
        );
        let id_salt = match destination {
            Destination::Library => "add_to_library",
            Destination::Playlist => "add_to_playlist",
            Destination::LibraryRefresh => "refresh_library",
        };
        let add =
            components::icon_button(ui, add_rect, components::HitShape::Circle, id_salt, true);
        paint_add(ui, add_rect, add.hover);

        // Меню под «+», выровнено по его правому краю. Открывается и закрывается
        // по нашему `clicked` — он учитывает только клики внутри круга
        egui::Popup::menu(&add.response)
            .open_memory(add.clicked.then_some(egui::SetOpenCommand::Toggle))
            .align(egui::RectAlign::BOTTOM_END)
            .gap(4.0)
            .show(|ui| {
                components::menu_style(ui);
                if components::menu_item(ui, t().add_files).clicked() {
                    self.dispatch(Action::AddFiles(destination));
                }
                if components::menu_item(ui, t().add_folder).clicked() {
                    self.dispatch(Action::AddFolder(destination));
                }
            });
    }

    /// Выбор отдельных файлов
    pub(crate) fn add_files(&mut self, destination: Destination) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter(t().audio_files, SUPPORTED_EXTENSIONS)
            .pick_files()
        else {
            // Пользователь закрыл диалог: оставляем текущий трек
            return;
        };
        self.start_loading(Source::Files(paths), destination);
    }

    /// Выбор папки: добавляются все аудиофайлы в ней, включая вложенные папки
    pub(crate) fn add_folder(&mut self, destination: Destination) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.start_loading(Source::Folder(dir), destination);
    }

    pub(crate) fn start_loading(&mut self, source: Source, destination: Destination) {
        // Что уже есть в библиотеке, второй раз не читаем: повторное добавление
        // той же папки добавит только новые файлы
        let skip = match destination {
            Destination::Library => self.core.library.paths().clone(),
            Destination::Playlist | Destination::LibraryRefresh => HashSet::new(),
        };
        self.loaders.push_back(PendingLoad {
            loader: Loader::start(source, skip),
            destination,
        });
    }

    /// Забирает готовые треки у фонового добавления. Вызывается каждый кадр
    pub(crate) fn poll_loaders(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.loaders.front_mut() else {
            return;
        };
        pending.loader.update();

        // Прогресс показываем, только если добавление затянулось: несколько файлов
        // добавляются быстрее, и тогда прогресс не мелькает вовсе
        if self.loader_shown_at.is_none() && pending.loader.elapsed() >= LOADER_SHOW_DELAY {
            self.loader_shown_at = Some(Instant::now());
        }
        // Уже показанный прогресс держим не меньше LOADER_MIN_VISIBLE — иначе он мог бы
        // появиться и сразу исчезнуть, это выглядит хуже, чем не показывать его вовсе
        let can_hide = self
            .loader_shown_at
            .is_none_or(|shown_at| shown_at.elapsed() >= LOADER_MIN_VISIBLE);

        if pending.loader.is_finished() && can_hide {
            let pending = self.loaders.pop_front();
            self.loader_shown_at = None;
            if let Some(pending) = pending {
                self.dispatch(Action::TracksLoaded {
                    finished: pending.loader.finish(),
                    destination: pending.destination,
                });
            }
        }

        // Пока идёт добавление, кадры нужны и без действий пользователя:
        // иначе прогресс обновлялся бы только при движении мыши
        if !self.loaders.is_empty() {
            ctx.request_repaint_after(LOADER_POLL_INTERVAL);
        }
    }

    /// Кладёт добавленные треки по назначению — все разом
    pub(crate) fn apply_loaded(&mut self, finished: Finished, destination: Destination) {
        // Обновление библиотеки: файлы, которые не удалось прочитать, просто остаются
        // как были (пропавшие и так помечены) — ошибки не показываем
        if destination == Destination::LibraryRefresh {
            let result = self.core.library.refresh(finished.tracks);
            self.report_error(result);
            return;
        }

        if !finished.tracks.is_empty() {
            match destination {
                Destination::Library => {
                    let result = self.core.library.add(finished.tracks);
                    self.report_error(result);
                }
                Destination::Playlist => {
                    let result = self.core.controller.add_tracks(finished.tracks);
                    self.report_error(result);
                }
                Destination::LibraryRefresh => unreachable!("обработано выше"),
            }
        }

        // Всё найденное уже было в библиотеке — это не ошибка, просто нечего добавлять
        if finished.total == Some(0) && finished.skipped == 0 {
            self.error = Some(t().no_audio_found.into());
        } else if !finished.errors.is_empty() {
            self.error = Some(summarize_errors(&finished.errors));
        }
    }

    /// Отменяет текущее добавление: уже прочитанные треки не добавляются
    pub(crate) fn cancel_loading(&mut self) {
        self.loaders.pop_front();
        self.loader_shown_at = None;
    }
}

/// Текст ошибки добавления: первые несколько файлов и сколько ещё не добавилось —
/// при тысячах файлов полный список не поместился бы на экран
fn summarize_errors(errors: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = errors[..errors.len().min(SHOWN)].join("\n");
    if errors.len() > SHOWN {
        let more = t().count(errors.len() - SHOWN, &t().more_file_count);
        text += "\n";
        text += &fill(t().more_files_failed, &[("more", &more)]);
    }
    text
}
