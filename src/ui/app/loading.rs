//! Добавление треков: выбор файлов и папки, очередь фоновых добавлений, прогресс.

use eframe::egui;
use std::time::{Duration, Instant};

use super::PlayerApp;
use crate::library::{
    loader::{Finished, Loader, Source},
    scan::SUPPORTED_EXTENSIONS,
};

// Фоновое добавление: как часто проверять готовность, через сколько показывать прогресс
// (быстрое добавление обходится без него) и сколько минимум его держать, чтобы не мелькал
const LOADER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOADER_SHOW_DELAY: Duration = Duration::from_millis(400);
const LOADER_MIN_VISIBLE: Duration = Duration::from_millis(500);

impl PlayerApp {
    /// Выбор отдельных файлов
    pub(super) fn add_files(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio", SUPPORTED_EXTENSIONS)
            .pick_files()
        else {
            // Пользователь закрыл диалог: оставляем текущий трек
            return;
        };
        self.loaders.push_back(Loader::start(Source::Files(paths)));
    }

    /// Выбор папки: добавляются все аудиофайлы в ней, включая вложенные папки
    pub(super) fn add_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.loaders.push_back(Loader::start(Source::Folder(dir)));
    }

    /// Забирает готовые треки у фонового добавления и кладёт их в конец списка.
    /// Вызывается каждый кадр
    pub(super) fn poll_loaders(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.loaders.front_mut() else {
            return;
        };
        loader.update();

        // Прогресс показываем, только если добавление затянулось: несколько файлов
        // добавляются быстрее, и тогда прогресс не мелькает вовсе
        if self.loader_shown_at.is_none() && loader.elapsed() >= LOADER_SHOW_DELAY {
            self.loader_shown_at = Some(Instant::now());
        }
        // Уже показанный прогресс держим не меньше LOADER_MIN_VISIBLE — иначе он мог бы
        // появиться и сразу исчезнуть, это выглядит хуже, чем не показывать его вовсе
        let can_hide = self
            .loader_shown_at
            .is_none_or(|shown_at| shown_at.elapsed() >= LOADER_MIN_VISIBLE);

        if loader.is_finished() && can_hide {
            let finished = self.loaders.pop_front().map(Loader::finish);
            self.loader_shown_at = None;
            if let Some(finished) = finished {
                self.apply_loaded(finished);
            }
        }

        // Пока идёт добавление, кадры нужны и без действий пользователя:
        // иначе прогресс обновлялся бы только при движении мыши
        if !self.loaders.is_empty() {
            ctx.request_repaint_after(LOADER_POLL_INTERVAL);
        }
    }

    /// Кладёт добавленные треки в конец списка — все разом
    pub(super) fn apply_loaded(&mut self, finished: Finished) {
        if !finished.tracks.is_empty() {
            // Если ничего ещё не выбрано, текущим становится первый из добавленных;
            // иначе текущий трек не трогаем
            let first_new = self.controller.player().track_list.tracks.len();
            let nothing_selected = self.controller.player().track_list.current_index.is_none();
            self.controller.append_tracks(finished.tracks);
            if nothing_selected && let Err(e) = self.controller.select(first_new) {
                self.error = Some(e);
            }
        }

        if finished.total == Some(0) {
            self.error = Some("No supported audio files found".into());
        } else if !finished.errors.is_empty() {
            self.error = Some(summarize_errors(&finished.errors));
        }
    }

    /// Отменяет текущее добавление: уже прочитанные треки не добавляются
    pub(super) fn cancel_loading(&mut self) {
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
        text += &format!(
            "\n…and {} more files could not be added",
            errors.len() - SHOWN
        );
    }
    text
}
