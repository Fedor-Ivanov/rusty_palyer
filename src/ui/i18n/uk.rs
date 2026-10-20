//! Тексты интерфейса на украинском.

use super::{Plural, Texts, slavic_plural_form};

pub static TEXTS: Texts = Texts {
    plural_form: slavic_plural_form,

    library: "Медіатека",
    now_playing: "Зараз грає",
    artists: "Артисти",
    albums: "Альбоми",
    songs: "Пісні",
    playlist: "Плейлист",
    settings: "Налаштування",

    track_count: Plural::three("трек", "треки", "треків"),
    song_count: Plural::three("пісня", "пісні", "пісень"),
    album_count: Plural::three("альбом", "альбоми", "альбомів"),
    artist_count: Plural::three("артист", "артисти", "артистів"),
    more_file_count: Plural::three("файл", "файли", "файлів"),

    no_tracks: "Немає треків",
    library_empty: "Медіатека порожня",
    no_results: "Нічого не знайдено",
    no_albums: "Немає альбомів",

    search: "Пошук",
    found_of: "{found} з {tracks}",

    open_files: "Відкрити файли",
    add_files: "Додати файли",
    add_folder: "Додати папку",
    audio_files: "Аудіо",
    adding_progress: "Додавання: {done} з {total}",
    updating_library_progress: "Оновлення медіатеки: {done} з {total}",
    scanning_folder: "Сканування папки…",
    no_audio_found: "Не знайдено підтримуваних аудіофайлів",
    more_files_failed: "…і ще {more} не вдалося додати",

    show_artist: "Показати артиста",
    show_album: "Показати альбом",
    show_current_track: "Показати поточний трек",
    file_missing: "Файл не знайдено: його переміщено або видалено",
    shuffle: "Перемішати",
    repeat_all: "Повторювати всі",
    repeat_one: "Повторювати один",

    play: "Відтворити",
    play_all: "Відтворити всі",
    add_to_playlist: "Додати до плейлиста",
    remove_from_library: "Видалити з медіатеки",
    remove_from_playlist: "Видалити з плейлиста",
    play_items: "Відтворити {items}",
    add_items_to_playlist: "Додати {items} до плейлиста",
    remove_items_from_library: "Видалити {items} з медіатеки",
    remove_items_from_playlist: "Видалити {items} з плейлиста",
    remove: "Видалити",
    cancel: "Скасувати",
    ok: "OK",
    done: "Готово",

    language: "Мова",

    remove_album_title: "Видалити альбом з медіатеки?",
    remove_album_message: "Альбом «{album}» буде видалено з медіатеки. \
                           Файли залишаться на диску.",
    remove_albums_title: "Видалити {albums} з медіатеки?",
    remove_albums_message: "Вибрані альбоми ({albums}, {tracks}) буде видалено з медіатеки. \
                            Файли залишаться на диску.",
    remove_artist_title: "Видалити артиста з медіатеки?",
    remove_artist_message: "Усі треки артиста «{artist}» буде видалено з медіатеки. \
                            Файли залишаться на диску.",
    remove_artists_title: "Видалити {artists} з медіатеки?",
    remove_artists_message: "Усі треки вибраних артистів ({artists}, {tracks}) буде видалено \
                             з медіатеки. Файли залишаться на диску.",
    remove_song_title: "Видалити пісню з медіатеки?",
    remove_song_message: "Пісню «{song}» буде видалено з медіатеки. Файл залишиться на диску.",
    remove_songs_title: "Видалити {songs} з медіатеки?",
    remove_songs_message: "Вибрані пісні ({songs}) буде видалено з медіатеки. \
                           Файли залишаться на диску.",
};
