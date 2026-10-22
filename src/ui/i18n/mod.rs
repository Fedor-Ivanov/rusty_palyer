//! Локализация интерфейса: все тексты на английском, французском, русском и украинском.
//!
//! Тексты одного языка — значение `Texts` (см. `en.rs`, `fr.rs`, `ru.rs`, `uk.rs`). Забытый в переводе текст
//! не даст собрать проект: у структуры нет значений по умолчанию.
//! Тексты с подстановками — шаблоны с `{имя}`, их заполняет `fill`. Числа со словом —
//! `Texts::count`: в русском и украинском у слова три формы (1 трек, 2 трека, 5 треков).
//!
//! Новый язык: вариант в `settings::Language` (и в `Language::ALL`), файл с его `Texts`
//! и строки в `t()` и `language_name` — компилятор подскажет, где не хватает.
//!
//! Язык один на всё приложение: виджетам он нужен без лишних параметров, поэтому текущий
//! язык — глобальный (`set_language`), а тексты берутся через `t()`.

mod en;
mod fr;
mod ru;
mod uk;

use std::sync::atomic::{AtomicU8, Ordering};

use crate::core::settings::{GlassTint, Language};

/// Правило форм числа в русском и украинском: 1, 21, 101 — «трек»; 2–4, 22–24 — «трека»;
/// 0, 5–20, 25–30, 111–114 — «треков»
fn slavic_plural_form(count: usize) -> PluralForm {
    match (count % 10, count % 100) {
        (1, rest) if rest != 11 => PluralForm::One,
        (2..=4, rest) if !(12..=14).contains(&rest) => PluralForm::Few,
        _ => PluralForm::Many,
    }
}

/// Слово в формах для разных чисел. В английском `few` совпадает с `many`
pub struct Plural {
    one: &'static str,
    few: &'static str,
    many: &'static str,
}

impl Plural {
    /// Две формы — как в английском: 1 track, 2 tracks
    const fn two(one: &'static str, many: &'static str) -> Self {
        Self {
            one,
            few: many,
            many,
        }
    }

    /// Три формы — как в русском: 1 трек, 2 трека, 5 треков
    const fn three(one: &'static str, few: &'static str, many: &'static str) -> Self {
        Self { one, few, many }
    }
}

/// Какая форма слова нужна для числа
pub enum PluralForm {
    One,
    Few,
    Many,
}

pub struct Texts {
    /// Какую форму слова брать для числа — правило языка
    plural_form: fn(usize) -> PluralForm,

    // ---- разделы
    pub library: &'static str,
    pub now_playing: &'static str,
    pub artists: &'static str,
    pub albums: &'static str,
    pub songs: &'static str,
    /// Очередь воспроизведения — правая панель
    pub queue: &'static str,
    /// Сохранённые плейлисты — раздел медиатеки
    pub playlists: &'static str,
    pub settings: &'static str,

    // ---- слова для чисел
    pub track_count: Plural,
    pub song_count: Plural,
    pub album_count: Plural,
    pub playlist_count: Plural,
    pub artist_count: Plural,
    /// «…и ещё N файлов не удалось добавить»
    pub more_file_count: Plural,

    // ---- заглушки
    pub no_tracks: &'static str,
    pub library_empty: &'static str,
    pub no_results: &'static str,
    pub no_albums: &'static str,
    pub no_playlists: &'static str,

    // ---- поиск и счётчики в заголовках
    pub search: &'static str,
    /// Сколько найдено в Songs: `{found}`, `{tracks}` — всего треков с числом
    pub found_of: &'static str,
    /// Битрейт в деталях трека: «320 kbps»
    pub bitrate: &'static str,

    // ---- добавление файлов
    pub open_files: &'static str,
    pub add_files: &'static str,
    pub add_folder: &'static str,
    /// Фильтр в окне выбора файлов
    pub audio_files: &'static str,
    /// Плашка прогресса: `{done}`, `{total}`
    pub adding_progress: &'static str,
    pub updating_library_progress: &'static str,
    pub scanning_folder: &'static str,
    pub no_audio_found: &'static str,
    /// Хвост списка ошибок добавления: `{more}` — число со словом из `more_file_count`
    pub more_files_failed: &'static str,

    // ---- переходы и подсказки
    pub show_artist: &'static str,
    /// Кнопка текста песни и заглушка, когда текста нет
    pub lyrics: &'static str,
    pub no_lyrics: &'static str,
    pub searching_lyrics: &'static str,
    /// LRCLIB не ответил — текста пока нет
    pub lyrics_unavailable: &'static str,
    /// Текста нет, а поиск в интернете выключен
    pub lyrics_offline: &'static str,
    /// Трек без слов — так о нём говорит LRCLIB
    pub instrumental: &'static str,
    pub show_album: &'static str,
    pub show_current_track: &'static str,
    pub file_missing: &'static str,
    pub shuffle: &'static str,
    pub repeat_all: &'static str,
    pub repeat_one: &'static str,

    // ---- действия
    pub play: &'static str,
    pub play_all: &'static str,
    pub add_to_queue: &'static str,
    pub remove_from_library: &'static str,
    pub remove_from_queue: &'static str,
    /// Действия с несколькими выделенными: `{items}` — число со словом
    pub play_items: &'static str,
    pub add_items_to_queue: &'static str,
    pub remove_items_from_library: &'static str,
    pub remove_items_from_queue: &'static str,
    pub remove: &'static str,
    pub cancel: &'static str,
    pub ok: &'static str,
    pub done: &'static str,

    // ---- плейлисты
    /// Подсказки кнопок очереди: сохранить её как плейлист и очистить
    pub save_queue: &'static str,
    /// Сохранить очередь в связанный с ней плейлист: `{playlist}`
    pub save_queue_to: &'static str,
    pub save_as_new_playlist: &'static str,
    pub clear_queue: &'static str,
    /// Подменю «добавить в сохранённый плейлист» и его первый пункт
    pub add_to_playlist: &'static str,
    pub new_playlist: &'static str,
    /// Убрать треки со страницы плейлиста: `{items}` — число со словом
    pub remove_from_this_playlist: &'static str,
    /// Трек плейлиста не из медиатеки — в медиатеку: `{items}` — число со словом
    pub add_to_library: &'static str,
    pub add_items_to_library: &'static str,
    pub remove_items_from_this_playlist: &'static str,
    pub rename: &'static str,
    pub delete: &'static str,
    pub save: &'static str,
    /// Заголовки окна названия плейлиста и подсказка в поле
    pub save_playlist_title: &'static str,
    pub new_playlist_title: &'static str,
    pub rename_playlist_title: &'static str,
    pub playlist_name_hint: &'static str,
    pub delete_playlist_title: &'static str,
    /// `{playlist}`
    pub delete_playlist_message: &'static str,
    pub delete_playlists_title: &'static str,
    pub delete_playlists_message: &'static str,
    /// «Удалить 3 плейлиста» — пункт меню нескольких выделенных
    pub delete_items: &'static str,

    // ---- настройки
    pub language: &'static str,
    pub version: &'static str,
    pub online_search: &'static str,
    /// Под переключателем: что и куда отправляется
    pub online_search_note: &'static str,
    pub glass_tint: &'static str,
    pub transparency: &'static str,
    pub tint_graphite: &'static str,
    pub tint_blue: &'static str,
    pub tint_violet: &'static str,
    pub tint_green: &'static str,
    pub tint_rose: &'static str,
    pub tint_amber: &'static str,

    // ---- диалог удаления из медиатеки
    pub remove_album_title: &'static str,
    /// `{album}`
    pub remove_album_message: &'static str,
    /// `{albums}`
    pub remove_albums_title: &'static str,
    /// `{albums}`, `{tracks}`
    pub remove_albums_message: &'static str,
    pub remove_artist_title: &'static str,
    /// `{artist}`
    pub remove_artist_message: &'static str,
    /// `{artists}`
    pub remove_artists_title: &'static str,
    /// `{artists}`, `{tracks}`
    pub remove_artists_message: &'static str,
    pub remove_song_title: &'static str,
    /// `{song}`
    pub remove_song_message: &'static str,
    /// `{songs}`
    pub remove_songs_title: &'static str,
    /// `{songs}`
    pub remove_songs_message: &'static str,
}

impl Texts {
    /// Подпись действия над треками: для одного — `one` («Add to library»), для нескольких —
    /// шаблон `many` с их числом в `{items}` («Add 3 tracks to library»)
    pub fn for_tracks(&self, one: &str, many: &str, count: usize) -> String {
        match count {
            0 | 1 => one.to_owned(),
            count => fill(many, &[("items", &self.count(count, &self.track_count))]),
        }
    }

    /// Число со словом в нужной форме: «3 tracks», «3 трека»
    pub fn count(&self, count: usize, word: &Plural) -> String {
        let form = match (self.plural_form)(count) {
            PluralForm::One => word.one,
            PluralForm::Few => word.few,
            PluralForm::Many => word.many,
        };
        format!("{count} {form}")
    }
}

/// Шаблон с подставленными значениями: `{name}` в `template` заменяется на значение
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

/// Название оттенка стекла на текущем языке
pub fn tint_name(tint: GlassTint) -> &'static str {
    match tint {
        GlassTint::Graphite => t().tint_graphite,
        GlassTint::Blue => t().tint_blue,
        GlassTint::Violet => t().tint_violet,
        GlassTint::Green => t().tint_green,
        GlassTint::Rose => t().tint_rose,
        GlassTint::Amber => t().tint_amber,
    }
}

/// Название языка на нём самом — так языки показывают в списке выбора: кто не понимает
/// текущий язык интерфейса, всё равно найдёт свой
pub fn language_name(language: Language) -> &'static str {
    match language {
        Language::English => "English",
        Language::French => "Français",
        Language::Russian => "Русский",
        Language::Ukrainian => "Українська",
    }
}

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Делает `language` языком интерфейса — со следующего кадра им пишутся все тексты
pub fn set_language(language: Language) {
    LANGUAGE.store(language as u8, Ordering::Relaxed);
}

/// Тексты на текущем языке
pub fn t() -> &'static Texts {
    let value = LANGUAGE.load(Ordering::Relaxed);
    let language = Language::ALL
        .into_iter()
        .find(|&language| language as u8 == value)
        .unwrap_or_default();
    match language {
        Language::English => &en::TEXTS,
        Language::French => &fr::TEXTS,
        Language::Russian => &ru::TEXTS,
        Language::Ukrainian => &uk::TEXTS,
    }
}

#[cfg(test)]
mod tests {
    use super::{Texts, en, fill, fr, ru, uk};
    use std::collections::BTreeSet;

    /// Шаблоны с подстановками — чтобы сравнить их наборы `{…}` в языках
    fn templates(t: &Texts) -> [(&'static str, &'static str); 25] {
        [
            ("found_of", t.found_of),
            ("bitrate", t.bitrate),
            ("adding_progress", t.adding_progress),
            ("updating_library_progress", t.updating_library_progress),
            ("more_files_failed", t.more_files_failed),
            ("play_items", t.play_items),
            ("add_items_to_queue", t.add_items_to_queue),
            ("remove_items_from_library", t.remove_items_from_library),
            ("remove_items_from_queue", t.remove_items_from_queue),
            ("remove_album_message", t.remove_album_message),
            ("remove_albums_title", t.remove_albums_title),
            ("remove_albums_message", t.remove_albums_message),
            ("remove_artist_message", t.remove_artist_message),
            ("remove_artists_title", t.remove_artists_title),
            ("remove_artists_message", t.remove_artists_message),
            ("remove_song_message", t.remove_song_message),
            ("remove_songs_title", t.remove_songs_title),
            ("remove_songs_message", t.remove_songs_message),
            (
                "remove_items_from_this_playlist",
                t.remove_items_from_this_playlist,
            ),
            ("delete_playlist_message", t.delete_playlist_message),
            ("delete_playlists_title", t.delete_playlists_title),
            ("delete_playlists_message", t.delete_playlists_message),
            ("delete_items", t.delete_items),
            ("save_queue_to", t.save_queue_to),
            ("add_items_to_library", t.add_items_to_library),
        ]
    }

    fn placeholders(template: &str) -> BTreeSet<&str> {
        template
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn translations_have_same_placeholders() {
        for (language, texts) in [("fr", &fr::TEXTS), ("ru", &ru::TEXTS), ("uk", &uk::TEXTS)] {
            for ((name, english), (_, translated)) in
                templates(&en::TEXTS).into_iter().zip(templates(texts))
            {
                assert_eq!(
                    placeholders(english),
                    placeholders(translated),
                    "{language}: {name}"
                );
            }
        }
    }

    #[test]
    fn ukrainian_and_french_plural_forms() {
        let t = &uk::TEXTS;
        let tracks = |n| t.count(n, &t.track_count);
        assert_eq!(tracks(1), "1 трек");
        assert_eq!(tracks(3), "3 треки");
        assert_eq!(tracks(5), "5 треків");
        assert_eq!(tracks(21), "21 трек");

        // Во французском 0 — тоже единственное число
        let t = &fr::TEXTS;
        assert_eq!(t.count(0, &t.track_count), "0 piste");
        assert_eq!(t.count(1, &t.track_count), "1 piste");
        assert_eq!(t.count(2, &t.track_count), "2 pistes");
    }

    #[test]
    fn russian_plural_forms() {
        let t = &ru::TEXTS;
        let tracks = |n| t.count(n, &t.track_count);
        assert_eq!(tracks(1), "1 трек");
        assert_eq!(tracks(3), "3 трека");
        assert_eq!(tracks(5), "5 треков");
        assert_eq!(tracks(11), "11 треков");
        assert_eq!(tracks(12), "12 треков");
        assert_eq!(tracks(21), "21 трек");
        assert_eq!(tracks(22), "22 трека");
        assert_eq!(tracks(111), "111 треков");
        assert_eq!(tracks(0), "0 треков");
    }

    #[test]
    fn english_plural_forms_and_fill() {
        let t = &en::TEXTS;
        assert_eq!(t.count(1, &t.track_count), "1 track");
        assert_eq!(t.count(2, &t.track_count), "2 tracks");
        assert_eq!(
            fill(t.adding_progress, &[("done", "3"), ("total", "10")]),
            "Adding 3 of 10"
        );
    }
}
