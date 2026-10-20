//! Тексты интерфейса на английском.

use super::{Plural, PluralForm, Texts};

fn plural_form(count: usize) -> PluralForm {
    if count == 1 {
        PluralForm::One
    } else {
        PluralForm::Many
    }
}

pub static TEXTS: Texts = Texts {
    plural_form,

    library: "Library",
    now_playing: "Now Playing",
    artists: "Artists",
    albums: "Albums",
    songs: "Songs",
    playlist: "Playlist",
    settings: "Settings",

    track_count: Plural::two("track", "tracks"),
    song_count: Plural::two("song", "songs"),
    album_count: Plural::two("album", "albums"),
    artist_count: Plural::two("artist", "artists"),
    more_file_count: Plural::two("more file", "more files"),

    no_tracks: "No tracks",
    library_empty: "Library is empty",
    no_results: "No results",
    no_albums: "No albums",

    search: "Search",
    found_of: "{found} of {tracks}",

    open_files: "Open files",
    add_files: "Add files",
    add_folder: "Add folder",
    audio_files: "Audio",
    adding_progress: "Adding {done} of {total}",
    updating_library_progress: "Updating library {done} of {total}",
    scanning_folder: "Scanning folder…",
    no_audio_found: "No supported audio files found",
    more_files_failed: "…and {more} could not be added",

    show_artist: "Show artist",
    show_album: "Show album",
    show_current_track: "Show the current track",
    file_missing: "File not found: it was moved or deleted",
    shuffle: "Shuffle",
    repeat_all: "Repeat all",
    repeat_one: "Repeat one",

    play: "Play",
    play_all: "Play all",
    add_to_playlist: "Add to playlist",
    remove_from_library: "Remove from library",
    remove_from_playlist: "Remove from playlist",
    play_items: "Play {items}",
    add_items_to_playlist: "Add {items} to playlist",
    remove_items_from_library: "Remove {items} from library",
    remove_items_from_playlist: "Remove {items} from playlist",
    remove: "Remove",
    cancel: "Cancel",
    ok: "OK",
    done: "Done",

    language: "Language",

    remove_album_title: "Remove album from library?",
    remove_album_message: "\u{201C}{album}\u{201D} will be removed from your library. \
                           The files will stay on your disk.",
    remove_albums_title: "Remove {albums} from library?",
    remove_albums_message: "{albums} ({tracks}) will be removed from your library. \
                            The files will stay on your disk.",
    remove_artist_title: "Remove artist from library?",
    remove_artist_message: "All tracks by \u{201C}{artist}\u{201D} will be removed from your \
                            library. The files will stay on your disk.",
    remove_artists_title: "Remove {artists} from library?",
    remove_artists_message: "All tracks by {artists} ({tracks}) will be removed from your \
                             library. The files will stay on your disk.",
    remove_song_title: "Remove song from library?",
    remove_song_message: "\u{201C}{song}\u{201D} will be removed from your library. \
                          The file will stay on your disk.",
    remove_songs_title: "Remove {songs} from library?",
    remove_songs_message: "{songs} will be removed from your library. \
                           The files will stay on your disk.",
};
